//! QuickJS runtime for Fig spec code (SPEC §9.2). Generator processes run in `process_executor` (SPEC §9.3); the
//! prelude and the module wrapper live in `quickjs_module_wrapper`.
//! Lives on the engine thread only (QuickJS is !Send). Every async JS call is resolved by a manual event pump
//! that also receives the engine's messages: a newer `EngineMsg::Complete` aborts the current wait.
//! OWNER: host agent.
//!
//! Host ↔ JS protocol: Rust registers `__ts_log`, `__ts_exec(ctxId, inputJson) → opId`, `__ts_sleep(ms) → opId`
//! and `__ts_done(task, ok, payload)`. JS keeps the resolvers of its pending host ops in a JS-side table and Rust
//! settles them with `__ts.settle(opId, ok, payload)`, so no JS handle ever lives on the Rust side (ponytail: no
//! `Persistent`, nothing can outlive the runtime). Callbacks start with `__ts.start(task, fnId, args, ctx)` and
//! report back through `__ts_done`.
use crate::settings::domain::services::settings_catalog::AUTOCOMPLETE_SCRIPT_TIMEOUT;
use crate::settings::domain::value_objects::settings::Settings;
use crate::shared::infrastructure::logger;
use crate::specs::domain::spec_error::SpecError;
use crate::specs::domain::value_objects::exec_request::ExecRequest;
use crate::specs::domain::value_objects::generator_context::GeneratorContext;
use crate::specs::domain::value_objects::spec_function_ref::SpecFunctionRef;
use crate::specs::domain::value_objects::task_id::TaskId;
use crate::specs::infrastructure::adapters::exec_input::{process_output_json, ExecInput};
use crate::specs::infrastructure::adapters::process_executor::exec;
use crate::specs::infrastructure::adapters::quickjs_module_wrapper::{wrap_module, PRELUDE};
use crate::types::EngineMsg;
use rquickjs::context::EvalOptions;
use rquickjs::{CatchResultExt, Context, Function, Object, Runtime};
use serde_json::{json, Map, Value};
use std::cell::{Cell, RefCell};
use std::collections::{HashMap, HashSet, VecDeque};
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::mpsc::{Receiver, RecvTimeoutError, Sender};
use std::time::{Duration, Instant, SystemTime};

/// One argument to a JS callback.
#[derive(Clone, Debug)]
pub(crate) enum JsArg {
    Json(Value),
    /// The `executeShellCommand` function, bound to the call's `GeneratorContext`.
    Exec,
}

/// Result of a finished task: JSON (callbacks: the returned value; exec: `{stdout, stderr, status}`) or an error.
pub(crate) type TaskResult = Result<Value, String>;

/// Module evaluation deadline (SPEC §6.3).
const LOAD_TIMEOUT: Duration = Duration::from_secs(5);
/// Interrupt deadline for small host → JS calls (settle/start) and the minimum CPU limit of each pumped job.
const SLICE: Duration = Duration::from_millis(1000);
const MIN_PUMP: Duration = Duration::from_millis(50);
const MEMORY_LIMIT: usize = 512 << 20;

/// Rust-side state shared with the functions registered in JS.
struct TaskRegistry {
    next_id: u64,
    /// Tasks started by the engine and not yet delivered or forgotten.
    live: HashSet<TaskId>,
    done: HashMap<TaskId, TaskResult>,
    /// Exec context of each callback task (`__ts_exec` gets the task id; unknown/0 → default).
    ctxs: HashMap<u64, GeneratorContext>,
    /// Pending sleeps (`start_sleep` tasks and JS `setTimeout` ops): (due, id).
    timers: Vec<(Instant, u64)>,
    tx: Sender<EngineMsg>,
    /// `autocomplete.scriptTimeout`.
    timeout_ms: u64,
}

impl TaskRegistry {
    fn id(&mut self) -> u64 {
        self.next_id += 1;
        self.next_id
    }

    /// Run `exec` on a worker thread; the result comes back as `EngineMsg::ExecDone{id}`.
    fn spawn_exec(&self, id: u64, input: ExecRequest, ctx: GeneratorContext) {
        let (tx, timeout) = (self.tx.clone(), self.timeout_ms);
        let spawned = std::thread::Builder::new()
            .name("tabsmith-exec".into())
            .spawn({
                let tx = tx.clone();
                move || {
                    let out = exec(&input, &ctx, timeout);
                    let _ = tx.send(EngineMsg::ExecDone { id, out });
                }
            });
        if let Err(e) = spawned {
            let _ = tx.send(EngineMsg::ExecDone {
                id,
                out: Err(format!("spawning exec thread: {e}")),
            });
        }
    }
}

fn script_timeout(settings: &Settings) -> u64 {
    settings.i64(AUTOCOMPLETE_SCRIPT_TIMEOUT).max(0) as u64
}

pub(crate) struct QuickJsSpecRuntime {
    ctx: Context,
    _rt: Runtime,
    tasks: Rc<RefCell<TaskRegistry>>,
    /// Interrupt deadline checked by QuickJS while running JS (`None` = no limit).
    deadline: Rc<Cell<Option<Instant>>>,
    rx: Receiver<EngineMsg>,
    deferred: RefCell<VecDeque<EngineMsg>>,
    /// Serialized namespaces by path, with the file's (mtime, len) when evaluated: an edited file (developer mode)
    /// is evaluated again. Old fn ids stay valid (`__ts.fns` only grows).
    modules: RefCell<ModuleCache>,
}

/// A file's (mtime, len); `None` if it cannot be stat'ed.
type Stamp = Option<(Option<SystemTime>, u64)>;

type ModuleCache = HashMap<PathBuf, (Stamp, Map<String, Value>)>;

impl QuickJsSpecRuntime {
    /// Create the runtime (memory limit, interrupt handler), evaluate the prelude.
    /// `rx` is the engine's inbox; `tx` is a sender to that same inbox (for exec workers and timers).
    /// Note: `QuickJsSpecRuntime` keeps `tx`, so the inbox never disconnects; the engine stops on
    /// `EngineMsg::Shutdown`.
    pub(crate) fn new(
        rx: Receiver<EngineMsg>,
        tx: Sender<EngineMsg>,
        settings: Settings,
    ) -> Result<Self, SpecError> {
        let rt = Runtime::new().map_err(|e| SpecError::Invalid(e.to_string()))?;
        rt.set_memory_limit(MEMORY_LIMIT);
        let deadline: Rc<Cell<Option<Instant>>> = Rc::default();
        let d = deadline.clone();
        rt.set_interrupt_handler(Some(Box::new(move || {
            d.get().is_some_and(|d| Instant::now() > d)
        })));
        let ctx = Context::full(&rt).map_err(|e| SpecError::Invalid(e.to_string()))?;
        let tasks = Rc::new(RefCell::new(TaskRegistry {
            next_id: 0,
            live: HashSet::new(),
            done: HashMap::new(),
            ctxs: HashMap::new(),
            timers: Vec::new(),
            tx,
            timeout_ms: script_timeout(&settings),
        }));
        ctx.with(|ctx| {
            let g = ctx.globals();
            let (h1, h2, h3) = (tasks.clone(), tasks.clone(), tasks.clone());
            (|| -> rquickjs::Result<()> {
                g.set(
                    "__ts_log",
                    Function::new(ctx.clone(), |level: String, msg: String| {
                        logger::debug(&format!("js console.{level}: {msg}"))
                    })?,
                )?;
                g.set(
                    "__ts_exec",
                    Function::new(ctx.clone(), move |ctx_id: f64, input: String| -> f64 {
                        let mut h = h1.borrow_mut();
                        let id = h.id();
                        let generator = h.ctxs.get(&(ctx_id as u64)).cloned().unwrap_or_default();
                        match serde_json::from_str::<ExecInput>(&input) {
                            Ok(input) => h.spawn_exec(id, ExecRequest::from(input), generator),
                            Err(e) => {
                                let _ = h.tx.send(EngineMsg::ExecDone {
                                    id,
                                    out: Err(format!("bad exec input: {e}")),
                                });
                            }
                        }
                        id as f64
                    })?,
                )?;
                g.set(
                    "__ts_sleep",
                    Function::new(ctx.clone(), move |ms: f64| -> f64 {
                        let mut h = h2.borrow_mut();
                        let id = h.id();
                        h.timers.push((
                            Instant::now() + Duration::from_millis(ms.max(0.0) as u64),
                            id,
                        ));
                        id as f64
                    })?,
                )?;
                g.set(
                    "__ts_done",
                    Function::new(ctx.clone(), move |task: f64, ok: bool, payload: String| {
                        let mut h = h3.borrow_mut();
                        let task = task as TaskId;
                        if h.live.contains(&task) {
                            let r = if ok {
                                serde_json::from_str(&payload).map_err(|e| e.to_string())
                            } else {
                                Err(payload)
                            };
                            h.done.insert(task, r);
                        }
                    })?,
                )?;
                ctx.eval::<(), _>(PRELUDE)
            })()
            .catch(&ctx)
            .map_err(|e| SpecError::Invalid(format!("js prelude: {e}")))
        })?;
        Ok(QuickJsSpecRuntime {
            ctx,
            _rt: rt,
            tasks,
            deadline,
            rx,
            deferred: RefCell::new(VecDeque::new()),
            modules: RefCell::new(HashMap::new()),
        })
    }

    /// Next message for the engine loop: deferred messages first (incl. a request that aborted a wait), then a
    /// blocking receive. `ExecDone`/`Timer` are consumed internally. `None` when all senders are gone.
    pub(crate) fn next_msg(&self) -> Option<EngineMsg> {
        loop {
            if let Some(m) = self.deferred.borrow_mut().pop_front() {
                return Some(m);
            }
            let m = self.rx.recv().ok()?;
            if let Some(m) = self.internal(m) {
                return Some(m);
            }
        }
    }

    /// Like `next_msg` but non-blocking.
    pub(crate) fn try_next_msg(&self) -> Option<EngineMsg> {
        loop {
            if let Some(m) = self.deferred.borrow_mut().pop_front() {
                return Some(m);
            }
            let m = self.rx.try_recv().ok()?;
            if let Some(m) = self.internal(m) {
                return Some(m);
            }
        }
    }

    pub(crate) fn apply_settings(&self, settings: &Settings) {
        self.tasks.borrow_mut().timeout_ms = script_timeout(settings);
    }

    /// Evaluate a spec module once (cached by path until its mtime/size changes) and return its exports, serialized
    /// (functions → `{"$fn": id}`), e.g. `{"default": {...}, "versions": {...}, "getVersionCommand": {"$fn": 3}}`.
    /// Empty file → empty map. 5 s deadline.
    pub(crate) fn load_module(&self, path: &Path) -> Result<Map<String, Value>, SpecError> {
        let stamp = std::fs::metadata(path)
            .ok()
            .map(|m| (m.modified().ok(), m.len()));
        let cached = self
            .modules
            .borrow()
            .get(path)
            .filter(|(s, _)| stamp.is_some() && *s == stamp)
            .map(|(_, ns)| ns.clone()); // ponytail: clone per call; the parser caches normalised specs anyway
        if let Some(ns) = cached {
            return Ok(ns);
        }
        let src = std::fs::read_to_string(path)
            .map_err(|e| SpecError::Invalid(format!("{}: {e}", path.display())))?;
        let ns = if src.trim().is_empty() {
            Map::new()
        } else {
            let script = wrap_module(&src).ok_or_else(|| {
                SpecError::Invalid(format!("{}: no trailing export clause", path.display()))
            })?;
            let deadline = Instant::now() + LOAD_TIMEOUT;
            let mut opts = EvalOptions::default();
            opts.filename = Some(path.display().to_string());
            self.arm(deadline);
            let r = self.ctx.with(|ctx| {
                ctx.eval_with_options::<String, _>(script, opts)
                    .catch(&ctx)
                    .map_err(|e| e.to_string())
            });
            self.arm_off();
            let json = r.map_err(|e| {
                if Instant::now() >= deadline {
                    SpecError::Timeout
                } else {
                    SpecError::Invalid(format!("{}: {e}", path.display()))
                }
            })?;
            match serde_json::from_str(&json) {
                Ok(Value::Object(m)) => m,
                Ok(_) => Map::new(),
                Err(e) => return Err(SpecError::Invalid(format!("{}: {e}", path.display()))),
            }
        };
        self.modules.borrow_mut().insert(path.to_path_buf(), (stamp, ns.clone()));
        Ok(ns)
    }

    /// Start a JS callback `fns[f](...args)`; the result is the serialized return value (Promises awaited).
    pub(crate) fn start_call(
        &self,
        f: SpecFunctionRef,
        args: Vec<JsArg>,
        ctx: Option<&GeneratorContext>,
    ) -> TaskId {
        let generator = ctx.cloned().unwrap_or_default();
        let id = {
            let mut h = self.tasks.borrow_mut();
            let id = h.id();
            h.live.insert(id);
            h.ctxs.insert(id, generator.clone());
            id
        };
        let mut exec_at = Vec::new();
        let args: Vec<Value> = args
            .into_iter()
            .enumerate()
            .map(|(i, a)| match a {
                JsArg::Json(v) => v,
                JsArg::Exec => {
                    exec_at.push(i);
                    Value::Null
                }
            })
            .collect();
        let mut c = json!({ "id": id, "exec": exec_at });
        if !generator.cwd().is_empty() {
            c["cwd"] = generator.cwd().into();
        }
        let (args, c) = (Value::Array(args).to_string(), c.to_string());
        self.arm(Instant::now() + SLICE);
        let r = self.ctx.with(|ctx| {
            (|| -> rquickjs::Result<()> {
                let ts: Object = ctx.globals().get("__ts")?;
                ts.get::<_, Function>("start")?
                    .call((id as f64, f, args, c))
            })()
            .catch(&ctx)
            .map_err(|e| e.to_string())
        });
        self.arm_off();
        if let Err(e) = r {
            self.tasks.borrow_mut().done.insert(id, Err(e));
        }
        id
    }

    /// Start a native process (SPEC §9.3) on a worker thread; result `{stdout, stderr, status}`.
    pub(crate) fn start_exec(&self, input: ExecRequest, ctx: &GeneratorContext) -> TaskId {
        let mut h = self.tasks.borrow_mut();
        let id = h.id();
        h.live.insert(id);
        h.spawn_exec(id, input, ctx.clone());
        id
    }

    /// Pump until every task finished or `deadline`. Returns one entry per task (`None` = still pending).
    /// Pending tasks stay alive and can be waited on again; finished results are delivered once (then dropped,
    /// like `forget`; an unknown/forgotten task yields `Some(Err)`). `Err(Aborted)` when a newer `Complete`
    /// arrived (or is already queued); no task is dropped then.
    pub(crate) fn wait_json(
        &self,
        tasks: &[TaskId],
        deadline: Instant,
    ) -> Result<Vec<Option<TaskResult>>, SpecError> {
        loop {
            if self
                .deferred
                .borrow()
                .iter()
                .any(|m| matches!(m, EngineMsg::Complete(_)))
            {
                return Err(SpecError::Aborted);
            }
            self.pump(deadline);
            let now = Instant::now();
            let finished = {
                let h = self.tasks.borrow();
                tasks
                    .iter()
                    .all(|t| h.done.contains_key(t) || !h.live.contains(t))
            };
            if finished || now >= deadline {
                return Ok(self.collect(tasks));
            }
            let next = self
                .tasks
                .borrow()
                .timers
                .iter()
                .map(|t| t.0)
                .min()
                .map_or(deadline, |t| t.min(deadline));
            match self.rx.recv_timeout(next.saturating_duration_since(now)) {
                Ok(m) => {
                    if let Some(m) = self.internal(m) {
                        self.deferred.borrow_mut().push_back(m); // the Complete check at the top aborts
                    }
                }
                Err(RecvTimeoutError::Timeout) => {}
                Err(RecvTimeoutError::Disconnected) => return Ok(self.collect(tasks)),
            }
        }
    }

    /// Convenience: start + wait for one callback.
    pub(crate) fn call_json(
        &self,
        f: SpecFunctionRef,
        args: Vec<JsArg>,
        ctx: Option<&GeneratorContext>,
        timeout: Duration,
    ) -> Result<Value, SpecError> {
        let task = self.start_call(f, args, ctx);
        let r = self.wait_json(&[task], Instant::now() + timeout);
        match r.map(|mut v| v.pop().flatten()) {
            Ok(Some(Ok(v))) => Ok(v),
            Ok(Some(Err(e))) => Err(SpecError::Invalid(e)),
            Ok(None) => {
                self.forget(&[task]);
                Err(SpecError::Timeout)
            }
            Err(e) => {
                self.forget(&[task]);
                Err(e)
            }
        }
    }

    /// Start a timer task that completes (with `null`) after `ms` (generator debounce).
    pub(crate) fn start_sleep(&self, ms: u64) -> TaskId {
        let mut h = self.tasks.borrow_mut();
        let id = h.id();
        h.live.insert(id);
        h.timers
            .push((Instant::now() + Duration::from_millis(ms), id));
        id
    }

    /// Drop the result/bookkeeping of tasks the caller no longer cares about.
    pub(crate) fn forget(&self, tasks: &[TaskId]) {
        let mut h = self.tasks.borrow_mut();
        for t in tasks {
            h.live.remove(t);
            h.done.remove(t);
            h.ctxs.remove(t);
            h.timers.retain(|x| x.1 != *t);
        }
    }

    fn arm(&self, until: Instant) {
        self.deadline.set(Some(until));
    }

    fn arm_off(&self) {
        self.deadline.set(None);
    }

    /// Handle `ExecDone`/`Timer`; return any other message.
    fn internal(&self, m: EngineMsg) -> Option<EngineMsg> {
        match m {
            EngineMsg::ExecDone { id, out } => {
                self.finish(id, out.map(|o| process_output_json(&o)));
            }
            EngineMsg::Timer { id } => {
                // ponytail: timers are deadlines checked by the pump; nothing sends Timer, but honour it.
                self.tasks.borrow_mut().timers.retain(|t| t.1 != id);
                self.finish(id, Ok(Value::Null));
            }
            m => return Some(m),
        }
        None
    }

    /// Deliver a host-op result: to the task table if it is an engine task, else to the waiting JS promise.
    fn finish(&self, id: u64, r: TaskResult) {
        let r = {
            let mut h = self.tasks.borrow_mut();
            if h.live.contains(&id) {
                h.done.insert(id, r);
                return;
            }
            r
        };
        let (ok, payload) = match r {
            Ok(v) => (true, v.to_string()),
            Err(e) => (false, e),
        };
        self.arm(Instant::now() + SLICE);
        let res = self.ctx.with(|ctx| {
            (|| -> rquickjs::Result<()> {
                let ts: Object = ctx.globals().get("__ts")?;
                ts.get::<_, Function>("settle")?
                    .call((id as f64, ok, payload))
            })()
            .catch(&ctx)
            .map_err(|e| e.to_string())
        });
        self.arm_off();
        if let Err(e) = res {
            logger::warn(&format!("js settle {id}: {e}"));
        }
    }

    /// Fire due timers, then run queued Promise jobs until `deadline` (at least `MIN_PUMP` from now). The slice only
    /// stops starting jobs: each job gets its own CPU limit (>= `SLICE`), so a long postProcess is not killed.
    fn pump(&self, deadline: Instant) {
        let now = Instant::now();
        let due: Vec<u64> = {
            let mut h = self.tasks.borrow_mut();
            let (due, keep) = h.timers.drain(..).partition(|t| t.0 <= now);
            h.timers = keep;
            due.into_iter().map(|t: (Instant, u64)| t.1).collect()
        };
        for id in due {
            self.finish(id, Ok(Value::Null));
        }
        let stop = deadline.max(now + MIN_PUMP);
        let d = self.deadline.clone();
        self.ctx.with(|ctx| {
            while Instant::now() < stop {
                d.set(Some(stop.max(Instant::now() + SLICE)));
                if !ctx.execute_pending_job() {
                    break;
                }
            }
        });
        self.arm_off();
    }

    fn collect(&self, tasks: &[TaskId]) -> Vec<Option<TaskResult>> {
        let mut h = self.tasks.borrow_mut();
        tasks
            .iter()
            .map(|t| match h.done.remove(t) {
                Some(r) => {
                    h.live.remove(t);
                    h.ctxs.remove(t);
                    Some(r)
                }
                None if h.live.contains(t) => None,
                None => Some(Err(format!("unknown task {t}"))),
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::specs::infrastructure::adapters::fig_spec_mapper::fn_id;
    #[cfg(unix)]
    use crate::specs::infrastructure::adapters::process_executor::fake_bin;
    use crate::shared::infrastructure::testing::temp_dir::TempDir;
    use crate::shared::domain::value_objects::shell::Shell;
    use crate::types::Request;
    use std::sync::mpsc;

    fn js() -> (QuickJsSpecRuntime, Sender<EngineMsg>) {
        let (tx, rx) = mpsc::channel();
        (QuickJsSpecRuntime::new(rx, tx.clone(), Settings::default()).unwrap(), tx)
    }

    /// Write a module `name` with `src` into `dir` and load it.
    fn module(js: &QuickJsSpecRuntime, dir: &TempDir, name: &str, src: &str) -> Map<String, Value> {
        let p = dir.path().join(name);
        std::fs::write(&p, src).unwrap();
        js.load_module(&p).unwrap()
    }

    fn call_default(
        js: &QuickJsSpecRuntime,
        ns: &Map<String, Value>,
        args: Vec<JsArg>,
        ctx: Option<&GeneratorContext>,
    ) -> Result<Value, SpecError> {
        js.call_json(
            fn_id(&ns["default"]).unwrap(),
            args,
            ctx,
            Duration::from_secs(5),
        )
    }

    fn find_key<'a>(v: &'a Value, key: &str) -> Option<&'a Value> {
        match v {
            Value::Object(m) => m
                .get(key)
                .or_else(|| m.values().find_map(|x| find_key(x, key))),
            Value::Array(a) => a.iter().find_map(|x| find_key(x, key)),
            _ => None,
        }
    }

    /// The `generatorContext` object for `custom(tokens, exec, generatorContext)` (SPEC §6.5). An empty `env`
    /// means "session env unknown" → the wrapper's own environment. Test-only: production builds it with
    /// `fig_spec_mapper::generator_context_json`, which keeps an empty env as `{}` (D1, D7).
    fn context_json(context: &GeneratorContext) -> Value {
        let env: Map<String, Value> = if context.env().is_empty() {
            std::env::vars_os()
                .map(|(k, v)| {
                    (
                        k.to_string_lossy().into_owned(),
                        v.to_string_lossy().into_owned().into(),
                    )
                })
                .collect()
        } else {
            context
                .env()
                .iter()
                .map(|(k, v)| (k.clone(), v.clone().into()))
                .collect()
        };
        json!({
            "currentWorkingDirectory": context.cwd(),
            "currentProcess": context.shell(),
            "sshPrefix": "",
            "environmentVariables": env,
            "searchTerm": context.search_term(),
            "isDangerous": context.is_dangerous(),
        })
    }

    #[test]
    fn loads_hand_written_es_modules() {
        let (js, _tx) = js();
        let dir = TempDir::new("esm");
        let src = "export const sub = { name: \"ok\" };\nconst spec = { name: \"zz\", subcommands: [sub] };\nexport default spec;\n";
        let ns = module(&js, &dir, "zz.js", src);
        assert_eq!(
            ns["default"],
            json!({"name": "zz", "subcommands": [{"name": "ok"}]})
        );
        let ns = module(
            &js,
            &dir,
            "inline.js",
            "export default {\n  name: \"inline\",\n} // end",
        );
        assert_eq!(ns["default"], json!({"name": "inline"}));
        // The esbuild form still wins.
        assert_eq!(
            module(
                &js,
                &dir,
                "eb.js",
                "var s={name:\"eb\"};export{s as default};"
            )["default"],
            json!({"name": "eb"})
        );
    }

    #[test]
    fn serializes_functions_and_values() {
        let (js, _tx) = js();
        let dir = TempDir::new("ser");
        let ns = module(
            &js,
            &dir,
            "m.js",
            "var f=()=>1,c={};c.self=c;var s={a:f,b:[f,1,undefined],u:undefined,r:/x+/g,n:null,big:10n,c,sym:Symbol()};\
             function g(){}export{s as default,g as helper};",
        );
        let d = &ns["default"];
        assert_eq!(d["a"], d["b"][0], "same function → same id");
        let (fa, fg) = (fn_id(&d["a"]).unwrap(), fn_id(&ns["helper"]).unwrap());
        assert_ne!(fa, fg);
        assert_eq!(d["b"], json!([{"$fn": fa}, 1, null]));
        assert_eq!(d.get("u"), None);
        assert_eq!(d.get("sym"), None);
        assert_eq!(d["r"], "/x+/g");
        assert_eq!(d["n"], Value::Null);
        assert_eq!(d["big"], "10");
        assert_eq!(d["c"], json!({"self": null}), "cycles → null");
        // Cached by path: same ids, no re-evaluation.
        assert_eq!(js.load_module(&dir.path().join("m.js")).unwrap(), ns);
        assert_eq!(
            js.call_json(fa, vec![], None, Duration::from_secs(2)),
            Ok(json!(1))
        );
        // Empty file → empty namespace; syntax error → Other.
        assert!(module(&js, &dir, "empty.js", " \n").is_empty());
        std::fs::write(dir.path().join("bad.js"), "var = ;export{a as default};").unwrap();
        assert!(matches!(
            js.load_module(&dir.path().join("bad.js")),
            Err(SpecError::Invalid(_))
        ));
    }

    #[cfg(feature = "embed-specs")]
    #[test]
    fn loads_bundled_specs() {
        let b = crate::specs::infrastructure::adapters::embedded_spec_bundle::testing::bundle_subset(&[
            "ls.js",
            "git.js",
            "aws/s3.js",
            "heroku/index.js",
        ]);
        let (js, _tx) = js();
        let ls = js.load_module(&b.path().join("ls.js")).unwrap();
        assert_eq!(ls["default"]["name"], "ls");
        let s3 = js.load_module(&b.path().join("aws/s3.js")).unwrap();
        assert_eq!(s3["default"]["name"], "s3");
        assert!(s3["default"]["subcommands"]
            .as_array()
            .is_some_and(|a| !a.is_empty()));

        let git = js.load_module(&b.path().join("git.js")).unwrap();
        assert_eq!(git["default"]["name"], "git");
        assert!(fn_id(&git["gitGenerators"]["commits"]["postProcess"]).is_some());
        let pp = fn_id(&git["gitGenerators"]["remotes"]["postProcess"]).unwrap();
        let stdout = "origin\tgit@github.com:a/b.git (fetch)\norigin\tgit@github.com:a/b.git (push)\nup\thttps://gitlab.com/x (fetch)";
        let out = js.call_json(
            pp,
            vec![
                JsArg::Json(stdout.into()),
                JsArg::Json(json!(["git", "push", ""])),
            ],
            None,
            Duration::from_secs(2),
        );
        assert_eq!(
            out,
            Ok(json!([
                {"name": "origin", "icon": "fig://icon?type=github", "description": "Remote"},
                {"name": "up", "icon": "fig://icon?type=gitlab", "description": "Remote"},
            ]))
        );

        // Diff-versioned resolver: `default(version)` picks the newest file <= version.
        let heroku = js.load_module(&b.path().join("heroku/index.js")).unwrap();
        assert!(fn_id(&heroku["getVersionCommand"]).is_some());
        for (v, want) in [
            (json!("8.6.0"), "heroku/8.6.0"),
            (json!("8.1.0"), "heroku/8.0.0"),
            (Value::Null, "heroku/8.6.0"),
        ] {
            let r = call_default(&js, &heroku, vec![JsArg::Json(v.clone())], None).unwrap();
            assert_eq!(r["versionedSpecPath"], want, "version {v}");
        }
    }

    /// `parserDirectives.alias` of git.js run against a fake `git` on a private PATH.
    #[cfg(all(unix, feature = "embed-specs"))]
    #[test]
    fn git_alias_directive_with_fake_git() {
        let bin = fake_bin(&[("git", r#"[ "$3" = alias.co ] && echo checkout || exit 1"#)]);
        let b = crate::specs::infrastructure::adapters::embedded_spec_bundle::testing::bundle_subset(&["git.js"]);
        let (js, _tx) = js();
        let git = js.load_module(&b.path().join("git.js")).unwrap();
        let alias = fn_id(
            find_key(&git["default"], "parserDirectives")
                .unwrap()
                .get("alias")
                .unwrap(),
        )
        .unwrap();
        let ctx = GeneratorContext::new(
            String::new(),
            [("PATH".to_string(), bin.path().display().to_string())].into(),
            String::new(),
            String::new(),
            false,
        );
        let run = |js: &QuickJsSpecRuntime, tok: &str| {
            js.call_json(
                alias,
                vec![JsArg::Json(tok.into()), JsArg::Exec],
                Some(&ctx),
                Duration::from_secs(5),
            )
        };
        assert_eq!(run(&js, "co"), Ok(json!("checkout")));
        assert!(
            matches!(run(&js, "nope"), Err(SpecError::Invalid(e)) if e.contains("Failed parsing alias"))
        );
    }

    /// A custom generator calling `executeShellCommand` (object and legacy string forms) through a fake `echo`.
    #[cfg(unix)]
    #[test]
    fn custom_generator_executes_commands() {
        let bin = fake_bin(&[("echo", r#"printf 'fake:%s\n' "$*""#)]);
        let cwd = TempDir::new("cwd");
        let (js, _tx) = js();
        let ns = module(
            &js,
            &cwd,
            "gen.js",
            "var g=async(tokens,exec,ctx)=>{let r=await exec({command:\"echo\",args:[tokens[0],ctx.searchTerm]});\
             let l=await exec(\"echo legacy | tr a-z A-Z\");let p=await exec({command:\"sh\",args:[\"-c\",\"pwd -P; printf %s $NO_COLOR$X\"],env:{X:\"y\"}});\
             let e;try{await exec({command:\"no-such-cmd-tabsmith\"})}catch(x){e=String(x)}\
             return [r.stdout,r.status,l,p.stdout,e]};export{g as default};",
        );
        let path = format!("{}:/bin:/usr/bin", bin.path().display());
        let ctx = GeneratorContext::new(
            cwd.path().display().to_string(),
            [("PATH".to_string(), path)].into(),
            String::new(),
            "term".into(),
            false,
        );
        let args = vec![
            JsArg::Json(json!(["git"])),
            JsArg::Exec,
            JsArg::Json(context_json(&ctx)),
        ];
        let out = call_default(&js, &ns, args, Some(&ctx)).unwrap();
        let real_cwd = cwd.path().canonicalize().unwrap().display().to_string();
        assert_eq!(out[0], "fake:git term");
        assert_eq!(out[1], 0);
        assert_eq!(
            out[2], "LEGACY",
            "legacy string form → sh -c, resolves to stdout (echo is a builtin there)"
        );
        assert_eq!(out[3], format!("{real_cwd}\n1y"));
        assert!(
            out[4]
                .as_str()
                .unwrap()
                .contains("failed running executable"),
            "{out}"
        );
    }

    #[test]
    fn set_timeout_ordering_and_intl() {
        let (js, _tx) = js();
        let dir = TempDir::new("timers");
        let ns = module(
            &js,
            &dir,
            "t.js",
            "var f=()=>new Promise(res=>{const o=[];setTimeout(()=>o.push(\"b\"),30);setTimeout(x=>o.push(x),0,\"a\");\
             const c=setTimeout(()=>o.push(\"never\"),10);clearTimeout(c);setTimeout(()=>res(o),60)});export{f as default};",
        );
        assert_eq!(
            call_default(&js, &ns, vec![], None),
            Ok(json!(["a", "b"]))
        );
        let ns = module(
            &js,
            &dir,
            "i.js",
            "var f=n=>new Intl.NumberFormat(void 0,{notation:\"compact\",compactDisplay:\"short\",maximumSignificantDigits:3}).format(n);\
             export{f as default};",
        );
        for (n, want) in [
            (1234567, "1.23M"),
            (15300, "15.3K"),
            (999, "999"),
            (2_000_000_000, "2B"),
        ] {
            assert_eq!(
                call_default(&js, &ns, vec![JsArg::Json(json!(n))], None),
                Ok(json!(want))
            );
        }
    }

    #[test]
    fn interrupt_stops_infinite_loop() {
        let (js, _tx) = js();
        let dir = TempDir::new("loop");
        let ns = module(
            &js,
            &dir,
            "l.js",
            "var f=()=>{while(true){}};var g=x=>x*2;export{f as default,g};",
        );
        let t0 = Instant::now();
        let r = js.call_json(
            fn_id(&ns["default"]).unwrap(),
            vec![],
            None,
            Duration::from_millis(200),
        );
        assert!(
            matches!(r, Err(SpecError::Timeout) | Err(SpecError::Invalid(_))),
            "{r:?}"
        );
        assert!(t0.elapsed() < Duration::from_secs(3));
        // The runtime is still usable.
        assert_eq!(
            js.call_json(
                fn_id(&ns["g"]).unwrap(),
                vec![JsArg::Json(json!(21))],
                None,
                Duration::from_secs(2)
            ),
            Ok(json!(42))
        );
    }

    /// The engine waits in 50 ms slices; a callback needing more CPU than one slice must still finish.
    #[test]
    fn long_callback_survives_short_pump_slices() {
        let (js, _tx) = js();
        let dir = TempDir::new("slice");
        let ns = module(&js, &dir, "b.js", "var f=ms=>{const t=Date.now();while(Date.now()-t<ms){}return ms};export{f as default};");
        let f = fn_id(&ns["default"]).unwrap();
        let tasks = [200, 40].map(|ms| js.start_call(f, vec![JsArg::Json(json!(ms))], None));
        let t0 = Instant::now();
        let mut got = vec![None, None];
        while got.iter().any(Option::is_none) && t0.elapsed() < Duration::from_secs(3) {
            for (g, r) in got.iter_mut().zip(
                js.wait_json(&tasks, Instant::now() + Duration::from_millis(50))
                    .unwrap(),
            ) {
                if g.is_none() {
                    *g = r; // results are delivered once
                }
            }
        }
        assert_eq!(got, vec![Some(Ok(json!(200))), Some(Ok(json!(40)))]);
    }

    /// Developer mode: an edited spec file is re-evaluated, an unchanged one is not (see `serializes_…`).
    #[test]
    fn load_module_reloads_edited_file() {
        let (js, _tx) = js();
        let dir = TempDir::new("reload");
        assert_eq!(
            module(
                &js,
                &dir,
                "m.js",
                "var s={name:\"v1\"};export{s as default};"
            )["default"]["name"],
            "v1"
        );
        let ns = module(
            &js,
            &dir,
            "m.js",
            "var s={name:\"v2-edited\"};export{s as default};",
        );
        assert_eq!(ns["default"]["name"], "v2-edited");
    }

    #[test]
    fn wait_aborts_and_defers_on_complete() {
        let (js, tx) = js();
        let req = |id| Request {
            id,
            buffer: "git ".into(),
            cursor: 4,
            cwd: "/".into(),
            shell: Shell::Zsh,
            history_mode: false,
        };
        let slow = js.start_sleep(10_000);
        tx.send(EngineMsg::ToggleFuzzy).unwrap();
        tx.send(EngineMsg::Complete(req(7))).unwrap();
        tx.send(EngineMsg::Shutdown).unwrap();
        let t0 = Instant::now();
        assert_eq!(
            js.wait_json(&[slow], Instant::now() + Duration::from_secs(5)),
            Err(SpecError::Aborted)
        );
        assert!(t0.elapsed() < Duration::from_secs(1));
        // Still queued → a new wait aborts at once.
        assert_eq!(
            js.wait_json(&[slow], Instant::now() + Duration::from_secs(5)),
            Err(SpecError::Aborted)
        );
        assert!(matches!(js.next_msg(), Some(EngineMsg::ToggleFuzzy)));
        assert!(matches!(js.next_msg(), Some(EngineMsg::Complete(r)) if r.id == 7));
        // Non-Complete messages are deferred without aborting; the slow task is still pending.
        let quick = js.start_sleep(10);
        let r = js
            .wait_json(&[quick, slow], Instant::now() + Duration::from_millis(300))
            .unwrap();
        assert_eq!(r, vec![Some(Ok(Value::Null)), None]);
        assert!(matches!(js.try_next_msg(), Some(EngineMsg::Shutdown)));
        assert!(js.try_next_msg().is_none());
        js.forget(&[slow]);
        let unknown = |t| Some(Err(format!("unknown task {t}")));
        assert_eq!(
            js.wait_json(&[slow, quick], Instant::now()).unwrap(),
            vec![unknown(slow), unknown(quick)]
        );
        // Exec tasks deliver `{stdout, stderr, status}`; ExecDone is consumed internally.
        let e = js.start_exec(
            ExecRequest::new("fig".into(), vec![], None, HashMap::new(), None),
            &GeneratorContext::default(),
        );
        let r = js
            .wait_json(&[e], Instant::now() + Duration::from_secs(5))
            .unwrap();
        assert_eq!(
            r,
            vec![Some(Ok(json!({"stdout": "", "stderr": "", "status": 1})))]
        );
    }

    /// Evaluate every module of the bundle (SPEC §10 `js`):
    /// `cargo test all_bundle_modules -- --ignored --nocapture`.
    #[cfg(feature = "embed-specs")]
    #[test]
    #[ignore]
    fn all_bundle_modules() {
        let dir = crate::specs::infrastructure::adapters::embedded_spec_bundle::testing::full_bundle();
        let mut files = Vec::new();
        let mut stack = vec![dir.path().to_path_buf()];
        while let Some(d) = stack.pop() {
            for e in std::fs::read_dir(d).unwrap().flatten() {
                let p = e.path();
                if p.is_dir() {
                    stack.push(p);
                } else if p.extension().is_some_and(|x| x == "js") {
                    files.push(p);
                }
            }
        }
        files.sort();
        let t0 = Instant::now();
        let (mut ok, mut empty, mut no_default, mut failed) = (0, 0, 0, Vec::new());
        let mut host = js();
        let mut slowest = (Duration::ZERO, PathBuf::new());
        for (i, f) in files.iter().enumerate() {
            if i % 100 == 0 {
                host = js(); // bound the namespace cache
            }
            let t = Instant::now();
            let r = host.0.load_module(f);
            if t.elapsed() > slowest.0 {
                slowest = (t.elapsed(), f.clone());
            }
            match r {
                Ok(ns) if ns.is_empty() => empty += 1,
                Ok(ns) if !ns.contains_key("default") => no_default += 1,
                Ok(_) => ok += 1,
                Err(e) => failed.push(format!(
                    "{}: {e}",
                    f.strip_prefix(dir.path()).unwrap().display()
                )),
            }
        }
        println!(
            "modules={} ok={ok} empty={empty} no_default={no_default} failed={} in {:?} (slowest {:?}: {})",
            files.len(),
            failed.len(),
            t0.elapsed(),
            slowest.0,
            slowest.1.display()
        );
        for f in failed.iter().take(20) {
            println!("  {f}");
        }
        assert!(failed.is_empty());
    }
}
