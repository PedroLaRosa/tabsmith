//! QuickJS host for Fig specs (SPEC §9.2) and the native `exec` used by generators (SPEC §9.3).
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
use crate::specs::domain::value_objects::spec_function_ref::SpecFunctionRef;
use crate::types::{EngineMsg, ExecOutput};
use rquickjs::context::EvalOptions;
use rquickjs::{CatchResultExt, Context, Function, Object, Runtime};
use serde::Deserialize;
use serde_json::{json, Map, Value};
use std::cell::{Cell, RefCell};
use std::collections::{HashMap, HashSet, VecDeque};
use std::ffi::OsString;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::rc::Rc;
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::sync::{Mutex, MutexGuard};
use std::time::{Duration, Instant, SystemTime};

/// `generatorContext` given to custom generators, plus what `exec` needs (SPEC §6.5, §9.3).
#[derive(Clone, Debug, Default)]
pub struct GenCtx {
    pub cwd: String,
    pub env: HashMap<String, String>,
    /// Shell name (`currentProcess`).
    pub shell: String,
    pub search_term: String,
    pub is_dangerous: bool,
}

impl GenCtx {
    /// The `generatorContext` object for `custom(tokens, exec, generatorContext)` (SPEC §6.5). An empty `env`
    /// means "session env unknown" → the wrapper's own environment.
    pub fn context_json(&self) -> Value {
        let env: Map<String, Value> = if self.env.is_empty() {
            std::env::vars_os()
                .map(|(k, v)| {
                    (
                        k.to_string_lossy().into_owned(),
                        v.to_string_lossy().into_owned().into(),
                    )
                })
                .collect()
        } else {
            self.env
                .iter()
                .map(|(k, v)| (k.clone(), v.clone().into()))
                .collect()
        };
        json!({
            "currentWorkingDirectory": self.cwd,
            "currentProcess": self.shell,
            "sshPrefix": "",
            "environmentVariables": env,
            "searchTerm": self.search_term,
            "isDangerous": self.is_dangerous,
        })
    }
}

/// One argument to a JS callback.
#[derive(Clone, Debug)]
pub enum JsArg {
    Json(Value),
    /// The `executeShellCommand` function, bound to the call's `GenCtx`.
    Exec,
}

/// `executeCommand` input `{command, args, cwd?, env?, timeout?}`.
#[derive(Clone, Debug, Default, Deserialize)]
pub struct ExecInput {
    pub command: String,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub cwd: Option<String>,
    /// `null` value → remove the variable.
    #[serde(default)]
    pub env: HashMap<String, Option<String>>,
    #[serde(default)]
    pub timeout: Option<u64>,
}

pub type TaskId = u64;

/// Result of a finished task: JSON (callbacks: the returned value; exec: `{stdout, stderr, status}`) or an error.
pub type TaskResult = Result<Value, String>;

/// Module evaluation deadline (SPEC §6.3).
const LOAD_TIMEOUT: Duration = Duration::from_secs(5);
/// Interrupt deadline for small host → JS calls (settle/start) and the minimum CPU limit of each pumped job.
const SLICE: Duration = Duration::from_millis(1000);
const MIN_PUMP: Duration = Duration::from_millis(50);
const MEMORY_LIMIT: usize = 512 << 20;

const PRELUDE: &str = r#"
globalThis.console = Object.fromEntries(["log","info","debug","warn","error"].map(l => [l, (...a) => __ts_log(l, a.map(String).join(" "))]));
let __tid = 0; const __live = new Set(); const __ops = new Map();
globalThis.setTimeout = (fn, ms = 0, ...args) => { const id = ++__tid; __live.add(id);
  __ts.op(__ts_sleep(+ms || 0)).then(() => { if (__live.delete(id)) fn(...args); }); return id; };
globalThis.clearTimeout = id => { __live.delete(id); };
globalThis.Intl ??= { NumberFormat: class { constructor(_l, o = {}) { this.o = o; }
  format(n) { if (this.o.notation !== "compact") return String(n);
    const u = [[1e12,"T"],[1e9,"B"],[1e6,"M"],[1e3,"K"]].find(([v]) => Math.abs(n) >= v);
    return u ? +(n / u[0]).toPrecision(this.o.maximumSignificantDigits ?? 3) + u[1] : String(n); } } };
globalThis.__ts = {
  fns: [], ids: new Map(),
  ser(v, seen = new WeakSet(), d = 0) {                 // functions → {"$fn": id}; drops undefined/symbol; cycle-safe
    if (typeof v === "function") { let id = this.ids.get(v);
      if (id === undefined) this.ids.set(v, id = this.fns.push(v) - 1); return { $fn: id }; }
    if (v === null || typeof v !== "object") return typeof v === "bigint" ? String(v) : v;
    if (v instanceof RegExp) return String(v);
    if (seen.has(v) || d > 200) return null; seen.add(v);
    const out = Array.isArray(v) ? v.map(x => this.ser(x, seen, d + 1))
      : Object.fromEntries(Object.entries(v).filter(([, x]) => x !== undefined && typeof x !== "symbol")
                                            .map(([k, x]) => [k, this.ser(x, seen, d + 1)]));
    seen.delete(v); return out;                         // shared (non-cyclic) refs are duplicated, cycles → null
  },
  json(v) { return JSON.stringify(this.ser(v) ?? null); },
  op(id) { return new Promise((res, rej) => __ops.set(id, [res, rej])); },
  settle(id, ok, v) { const p = __ops.get(id); if (p) { __ops.delete(id); ok ? p[0](v) : p[1](new Error(v)); } },
  mkExec(ctx) {
    const run = input => __ts.op(__ts_exec(ctx.id, JSON.stringify(input))).then(JSON.parse);
    return input => typeof input === "string"
      ? run({ command: "sh", args: ["-c", input], cwd: ctx.cwd }).then(r => r.stdout)
      : run({ cwd: ctx.cwd, ...input });
  },
  call(id, argsJson, ctxJson) {                          // every spec callback goes through here
    const ctx = JSON.parse(ctxJson), args = JSON.parse(argsJson);
    for (const i of ctx.exec) args[i] = this.mkExec(ctx);
    return Promise.resolve().then(() => this.fns[id](...args)).then(r => this.json(r));
  },
  start(task, id, argsJson, ctxJson) {
    __ts.call(id, argsJson, ctxJson).then(r => __ts_done(task, true, r), e => __ts_done(task, false, String(e)));
  },
};
"#;

/// Rust-side state shared with the functions registered in JS.
struct Host {
    next_id: u64,
    /// Tasks started by the engine and not yet delivered or forgotten.
    live: HashSet<TaskId>,
    done: HashMap<TaskId, TaskResult>,
    /// Exec context of each callback task (`__ts_exec` gets the task id; unknown/0 → default).
    ctxs: HashMap<u64, GenCtx>,
    /// Pending sleeps (`start_sleep` tasks and JS `setTimeout` ops): (due, id).
    timers: Vec<(Instant, u64)>,
    tx: Sender<EngineMsg>,
    /// `autocomplete.scriptTimeout`.
    timeout_ms: u64,
}

impl Host {
    fn id(&mut self) -> u64 {
        self.next_id += 1;
        self.next_id
    }

    /// Run `exec` on a worker thread; the result comes back as `EngineMsg::ExecDone{id}`.
    fn spawn_exec(&self, id: u64, input: ExecInput, ctx: GenCtx) {
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

pub struct Js {
    ctx: Context,
    _rt: Runtime,
    host: Rc<RefCell<Host>>,
    /// Interrupt deadline checked by QuickJS while running JS (`None` = no limit).
    deadline: Rc<Cell<Option<Instant>>>,
    rx: Receiver<EngineMsg>,
    deferred: VecDeque<EngineMsg>,
    /// Serialized namespaces by path, with the file's (mtime, len) when evaluated: an edited file (developer mode)
    /// is evaluated again. Old fn ids stay valid (`__ts.fns` only grows).
    modules: HashMap<PathBuf, (Stamp, Map<String, Value>)>,
}

/// A file's (mtime, len); `None` if it cannot be stat'ed.
type Stamp = Option<(Option<SystemTime>, u64)>;

impl Js {
    /// Create the runtime (memory limit, interrupt handler), evaluate the prelude.
    /// `rx` is the engine's inbox; `tx` is a sender to that same inbox (for exec workers and timers).
    /// Note: `Js` keeps `tx`, so the inbox never disconnects; the engine stops on `EngineMsg::Shutdown`.
    pub fn new(
        rx: Receiver<EngineMsg>,
        tx: Sender<EngineMsg>,
        settings: Settings,
    ) -> anyhow::Result<Js> {
        let rt = Runtime::new()?;
        rt.set_memory_limit(MEMORY_LIMIT);
        let deadline: Rc<Cell<Option<Instant>>> = Rc::default();
        let d = deadline.clone();
        rt.set_interrupt_handler(Some(Box::new(move || {
            d.get().is_some_and(|d| Instant::now() > d)
        })));
        let ctx = Context::full(&rt)?;
        let host = Rc::new(RefCell::new(Host {
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
            let (h1, h2, h3) = (host.clone(), host.clone(), host.clone());
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
                            Ok(input) => h.spawn_exec(id, input, generator),
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
            .map_err(|e| anyhow::anyhow!("js prelude: {e}"))
        })?;
        Ok(Js {
            ctx,
            _rt: rt,
            host,
            deadline,
            rx,
            deferred: VecDeque::new(),
            modules: HashMap::new(),
        })
    }

    /// Next message for the engine loop: deferred messages first (incl. a request that aborted a wait), then a
    /// blocking receive. `ExecDone`/`Timer` are consumed internally. `None` when all senders are gone.
    pub fn next_msg(&mut self) -> Option<EngineMsg> {
        loop {
            if let Some(m) = self.deferred.pop_front() {
                return Some(m);
            }
            let m = self.rx.recv().ok()?;
            if let Some(m) = self.internal(m) {
                return Some(m);
            }
        }
    }

    /// Like `next_msg` but non-blocking.
    pub fn try_next_msg(&mut self) -> Option<EngineMsg> {
        loop {
            if let Some(m) = self.deferred.pop_front() {
                return Some(m);
            }
            let m = self.rx.try_recv().ok()?;
            if let Some(m) = self.internal(m) {
                return Some(m);
            }
        }
    }

    pub fn set_settings(&mut self, settings: Settings) {
        self.host.borrow_mut().timeout_ms = script_timeout(&settings);
    }

    /// Evaluate a spec module once (cached by path until its mtime/size changes) and return its exports, serialized
    /// (functions → `{"$fn": id}`), e.g. `{"default": {...}, "versions": {...}, "getVersionCommand": {"$fn": 3}}`.
    /// Empty file → empty map. 5 s deadline.
    pub fn load_module(&mut self, path: &Path) -> Result<Map<String, Value>, SpecError> {
        let stamp = std::fs::metadata(path)
            .ok()
            .map(|m| (m.modified().ok(), m.len()));
        if let Some((_, ns)) = self
            .modules
            .get(path)
            .filter(|(s, _)| stamp.is_some() && *s == stamp)
        {
            return Ok(ns.clone()); // ponytail: clone per call; the parser caches normalised specs anyway
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
        self.modules.insert(path.to_path_buf(), (stamp, ns.clone()));
        Ok(ns)
    }

    /// Start a JS callback `fns[f](...args)`; the result is the serialized return value (Promises awaited).
    pub fn start_call(&mut self, f: SpecFunctionRef, args: Vec<JsArg>, ctx: Option<&GenCtx>) -> TaskId {
        let generator = ctx.cloned().unwrap_or_default();
        let id = {
            let mut h = self.host.borrow_mut();
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
        if !generator.cwd.is_empty() {
            c["cwd"] = generator.cwd.into();
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
            self.host.borrow_mut().done.insert(id, Err(e));
        }
        id
    }

    /// Start a native process (SPEC §9.3) on a worker thread; result `{stdout, stderr, status}`.
    pub fn start_exec(&mut self, input: ExecInput, ctx: &GenCtx) -> TaskId {
        let mut h = self.host.borrow_mut();
        let id = h.id();
        h.live.insert(id);
        h.spawn_exec(id, input, ctx.clone());
        id
    }

    /// Pump until every task finished or `deadline`. Returns one entry per task (`None` = still pending).
    /// Pending tasks stay alive and can be waited on again; finished results are delivered once (then dropped,
    /// like `forget`; an unknown/forgotten task yields `Some(Err)`). `Err(Aborted)` when a newer `Complete`
    /// arrived (or is already queued); no task is dropped then.
    pub fn wait(
        &mut self,
        tasks: &[TaskId],
        deadline: Instant,
    ) -> Result<Vec<Option<TaskResult>>, SpecError> {
        loop {
            if self
                .deferred
                .iter()
                .any(|m| matches!(m, EngineMsg::Complete(_)))
            {
                return Err(SpecError::Aborted);
            }
            self.pump(deadline);
            let now = Instant::now();
            let finished = {
                let h = self.host.borrow();
                tasks
                    .iter()
                    .all(|t| h.done.contains_key(t) || !h.live.contains(t))
            };
            if finished || now >= deadline {
                return Ok(self.collect(tasks));
            }
            let next = self
                .host
                .borrow()
                .timers
                .iter()
                .map(|t| t.0)
                .min()
                .map_or(deadline, |t| t.min(deadline));
            match self.rx.recv_timeout(next.saturating_duration_since(now)) {
                Ok(m) => {
                    if let Some(m) = self.internal(m) {
                        self.deferred.push_back(m); // the Complete check at the top aborts
                    }
                }
                Err(RecvTimeoutError::Timeout) => {}
                Err(RecvTimeoutError::Disconnected) => return Ok(self.collect(tasks)),
            }
        }
    }

    /// Convenience: start + wait for one callback.
    pub fn call(
        &mut self,
        f: SpecFunctionRef,
        args: Vec<JsArg>,
        ctx: Option<&GenCtx>,
        timeout: Duration,
    ) -> Result<Value, SpecError> {
        let task = self.start_call(f, args, ctx);
        let r = self.wait(&[task], Instant::now() + timeout);
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
    pub fn start_sleep(&mut self, ms: u64) -> TaskId {
        let mut h = self.host.borrow_mut();
        let id = h.id();
        h.live.insert(id);
        h.timers
            .push((Instant::now() + Duration::from_millis(ms), id));
        id
    }

    /// Drop the result/bookkeeping of tasks the caller no longer cares about.
    pub fn forget(&mut self, tasks: &[TaskId]) {
        let mut h = self.host.borrow_mut();
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
    fn internal(&mut self, m: EngineMsg) -> Option<EngineMsg> {
        match m {
            EngineMsg::ExecDone { id, out } => {
                self.finish(id, out.map(|o| serde_json::to_value(o).unwrap_or_default()));
            }
            EngineMsg::Timer { id } => {
                // ponytail: timers are deadlines checked by the pump; nothing sends Timer, but honour it.
                self.host.borrow_mut().timers.retain(|t| t.1 != id);
                self.finish(id, Ok(Value::Null));
            }
            m => return Some(m),
        }
        None
    }

    /// Deliver a host-op result: to the task table if it is an engine task, else to the waiting JS promise.
    fn finish(&mut self, id: u64, r: TaskResult) {
        let r = {
            let mut h = self.host.borrow_mut();
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
    fn pump(&mut self, deadline: Instant) {
        let now = Instant::now();
        let due: Vec<u64> = {
            let mut h = self.host.borrow_mut();
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

    fn collect(&mut self, tasks: &[TaskId]) -> Vec<Option<TaskResult>> {
        let mut h = self.host.borrow_mut();
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

/// Rewrite an esbuild ES module (`…;export{a as default,b as versions};`, no imports — verified for every file of
/// the bundle) into a script returning `__ts.json` of its exports. A hand-written module (`export default spec;`, the
/// Fig docs form, common in dev/user/local specs) is accepted too.
fn wrap_module(src: &str) -> Option<String> {
    let Some((at, rest)) = src
        .rmatch_indices("export")
        .map(|(i, _)| (i, src[i + 6..].trim_start()))
        .find(|(_, rest)| rest.starts_with('{'))
    else {
        // ponytail: only `default` is exported here; named exports (`versions`) come from esbuild bundles anyway.
        let i = src.rfind("export default")?;
        let body: Vec<&str> = src[..i]
            .lines()
            .map(|l| l.strip_prefix("export ").unwrap_or(l))
            .collect();
        return Some(format!(
            "__ts.json((function(){{{}\nvar __ts_default={}\n;return {{default:__ts_default}};}})())",
            body.join("\n"),
            &src[i + 14..]
        ));
    };
    let clause = rest[1..].split('}').next()?;
    let fields: Vec<String> = clause
        .split(',')
        .map(str::trim)
        .filter(|p| !p.is_empty())
        .map(|p| match p.split_once(" as ") {
            Some((local, exported)) => format!("{}:{}", exported.trim(), local.trim()),
            None => format!("{p}:{p}"),
        })
        .collect();
    Some(format!(
        "__ts.json((function(){{{}\n;return {{{}}};}})())",
        &src[..at],
        fields.join(",")
    ))
}

/// Run a process with RunProcess semantics (SPEC §9.3), blocking. Used by exec workers.
/// `default_timeout_ms` = `autocomplete.scriptTimeout` (5000; Windows 20000).
pub fn exec(
    input: &ExecInput,
    ctx: &GenCtx,
    default_timeout_ms: u64,
) -> Result<ExecOutput, String> {
    let cwd = [input.cwd.as_deref(), Some(ctx.cwd.as_str())]
        .into_iter()
        .flatten()
        .map(PathBuf::from)
        .find(|p| !p.as_os_str().is_empty() && p.is_dir())
        .or_else(|| std::env::current_dir().ok())
        .unwrap_or_else(|| PathBuf::from(if cfg!(windows) { "C:\\" } else { "/" }));
    if input.command == "ls" && input.args.iter().map(|a| a.trim()).eq(["-1ApL"]) {
        // The listed dir itself, never the fallback: `cd nonexist/` must list nothing, not the shell cwd.
        let dir = input
            .cwd
            .as_deref()
            .filter(|c| !c.is_empty())
            .map_or(cwd, |c| Path::new(&ctx.cwd).join(c));
        return Ok(native_ls(&dir));
    }
    if ["fig", "tabsmith"].contains(&input.command.as_str()) {
        return Ok(ExecOutput {
            status: 1,
            ..Default::default()
        }); // bundled ai() helper
    }
    // Effective value of a variable in the child's env (input.env > session env > wrapper env).
    let var = |key: &str| -> Option<OsString> {
        if let Some((_, v)) = input.env.iter().find(|(k, _)| key_eq(k, key)) {
            return v.as_ref().map(OsString::from);
        }
        if ctx.env.is_empty() {
            std::env::var_os(key)
        } else {
            ctx.env
                .iter()
                .find(|(k, _)| key_eq(k, key))
                .map(|(_, v)| v.into())
        }
    };
    let mut cmd = build_command(input, &cwd, &var)?;
    if !ctx.env.is_empty() {
        cmd.env_clear().envs(&ctx.env);
    }
    for k in ["LS_COLORS", "CLICOLOR_FORCE", "CLICOLOR", "COLORTERM"] {
        cmd.env_remove(k);
    }
    cmd.envs([
        ("PROCESS_LAUNCHED_BY_TABSMITH", "1"),
        ("HISTFILE", ""),
        ("HISTCONTROL", "ignoreboth"),
        ("TERM", "xterm-256color"),
        ("NO_COLOR", "1"),
    ]);
    for (k, v) in &input.env {
        match v {
            Some(v) => cmd.env(k, v),
            None => cmd.env_remove(k),
        };
    }
    // cmd.exe runs bare names from its cwd before PATH (a repo's `npm.cmd`); this makes it skip the cwd.
    if Path::new(cmd.get_program())
        .file_stem()
        .is_some_and(|s| s.eq_ignore_ascii_case("cmd"))
    {
        cmd.env("NoDefaultCurrentDirectoryInExePath", "1");
    }
    cmd.current_dir(&cwd)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        // SAFETY: runs in the forked child before exec; setsid is async-signal-safe.
        unsafe {
            cmd.pre_exec(|| {
                if libc::setsid() == -1 {
                    Err(std::io::Error::last_os_error())
                } else {
                    Ok(())
                }
            });
        }
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(windows_sys::Win32::System::Threading::CREATE_NO_WINDOW);
    }
    let timeout = default_timeout_ms.max(input.timeout.unwrap_or(0));
    let child = cmd
        .spawn()
        .map_err(|e| format!("failed running executable ({}): {e}", input.command))?;
    wait_output(child, timeout, &input.command)
}

fn key_eq(a: &str, b: &str) -> bool {
    if cfg!(windows) {
        a.eq_ignore_ascii_case(b)
    } else {
        a == b
    }
}

/// `Command` for `input` (no shell). Windows: the legacy `sh -c s` form runs through Git Bash, else `cmd /C`.
fn build_command(
    input: &ExecInput,
    cwd: &Path,
    var: &dyn Fn(&str) -> Option<OsString>,
) -> Result<Command, String> {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        if input.command == "sh"
            && input.args.len() == 2
            && input.args[0] == "-c"
            && find_exe("sh", cwd, var).is_none()
        {
            let mut c;
            match git_bash(cwd, var) {
                Some(bash) => {
                    c = Command::new(bash);
                    c.args(&input.args);
                }
                None => {
                    c = Command::new(var("ComSpec").unwrap_or_else(|| "cmd.exe".into()));
                    c.arg("/C").raw_arg(&input.args[1]); // raw: cmd does its own (non-MSVC) parsing
                }
            }
            return Ok(c);
        }
    }
    let exe = find_exe(&input.command, cwd, var)
        .ok_or_else(|| format!("failed running executable ({}): not found", input.command))?;
    let mut c = Command::new(exe);
    c.args(&input.args);
    Ok(c)
}

/// `which`: absolute → as is; with a separator → relative to `cwd`; else each `PATH` dir (Windows: `PATHEXT`).
fn find_exe(name: &str, cwd: &Path, var: &dyn Fn(&str) -> Option<OsString>) -> Option<PathBuf> {
    let p = Path::new(name);
    if p.is_absolute() {
        return Some(p.to_path_buf());
    }
    let dirs: Vec<PathBuf> = if p.components().count() > 1 {
        vec![cwd.to_path_buf()]
    } else {
        var("PATH")
            .map(|v| std::env::split_paths(&v).collect())
            .unwrap_or_default()
    };
    let exts: Vec<String> = if cfg!(windows) {
        let pathext = var("PATHEXT").map(|v| v.to_string_lossy().into_owned());
        let mut exts: Vec<String> = if p.extension().is_some() {
            vec![String::new()]
        } else {
            vec![]
        };
        exts.extend(
            pathext
                .as_deref()
                .unwrap_or(".COM;.EXE;.BAT;.CMD")
                .split(';')
                .filter(|e| !e.is_empty())
                .map(String::from),
        );
        exts
    } else {
        vec![String::new()]
    };
    dirs.iter()
        .flat_map(|d| exts.iter().map(move |e| d.join(format!("{name}{e}"))))
        .find(|c| is_executable(c))
}

fn is_executable(p: &Path) -> bool {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::metadata(p).is_ok_and(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
    }
    #[cfg(not(unix))]
    {
        p.is_file()
    }
}

/// Git for Windows' `bash.exe` (ish `gitBashPath`): next to `git.exe` on PATH, else the usual install dirs.
#[cfg(windows)]
fn git_bash(cwd: &Path, var: &dyn Fn(&str) -> Option<OsString>) -> Option<PathBuf> {
    let mut roots: Vec<PathBuf> = find_exe("git", cwd, var)
        .and_then(|g| Some(g.parent()?.parent()?.to_path_buf()))
        .into_iter()
        .collect();
    for v in ["ProgramW6432", "ProgramFiles", "ProgramFiles(x86)"] {
        roots.extend(var(v).map(|d| PathBuf::from(d).join("Git")));
    }
    roots.extend(var("LOCALAPPDATA").map(|d| PathBuf::from(d).join("Programs").join("Git")));
    roots
        .iter()
        .flat_map(|r| {
            [
                r.join("bin").join("bash.exe"),
                r.join("usr").join("bin").join("bash.exe"),
            ]
        })
        .find(|p| p.is_file())
}

/// Collect stdout/stderr (reader threads, so full pipes never block the child) while polling for exit every
/// 5 ms; on timeout kill the child (Unix: its whole session/process group; Windows: its job object) and fail.
fn wait_output(mut child: Child, timeout_ms: u64, label: &str) -> Result<ExecOutput, String> {
    #[cfg(unix)]
    let _live = Live::new(child.id() as isize);
    #[cfg(windows)]
    let job = job::Job::assign(&child);
    #[cfg(windows)]
    let _live = job.as_ref().map(|j| Live::new(j.handle() as isize)); // dropped before `job` closes the handle
    let (otx, orx) = mpsc::channel::<(usize, Vec<u8>)>();
    let pipes: [Option<Box<dyn Read + Send>>; 2] = [
        child
            .stdout
            .take()
            .map(|p| Box::new(p) as Box<dyn Read + Send>),
        child
            .stderr
            .take()
            .map(|p| Box::new(p) as Box<dyn Read + Send>),
    ];
    for (i, pipe) in pipes.into_iter().enumerate() {
        let otx = otx.clone();
        std::thread::spawn(move || {
            let mut buf = Vec::new();
            if let Some(mut p) = pipe {
                let _ = p.read_to_end(&mut buf);
            }
            let _ = otx.send((i, buf));
        });
    }
    let deadline = Instant::now() + Duration::from_millis(timeout_ms);
    let mut out: [Option<Vec<u8>>; 2] = [None, None];
    let mut status = None;
    loop {
        if status.is_none() {
            status = child
                .try_wait()
                .map_err(|e| format!("waiting for {label}: {e}"))?;
        }
        if let (Some(s), [Some(o), Some(e)]) = (status, &out) {
            return Ok(ExecOutput {
                stdout: clean(o),
                stderr: clean(e),
                status: s.code().unwrap_or(1),
            });
        }
        if Instant::now() >= deadline {
            #[cfg(unix)]
            // SAFETY: plain syscall; the child called setsid, so its pid is its process-group id.
            unsafe {
                libc::kill(-(child.id() as libc::pid_t), libc::SIGKILL);
            }
            #[cfg(windows)]
            if let Some(job) = &job {
                job.kill();
            }
            let _ = child.kill();
            let _ = child.wait();
            return Err(format!(
                "timed out after {timeout_ms} ms running executable ({label})"
            ));
        }
        if let Ok((i, b)) = orx.recv_timeout(Duration::from_millis(5)) {
            out[i] = Some(b);
        }
    }
}

/// Exec children still running: Unix process-group ids (each child called setsid), Windows job handles.
static LIVE: Mutex<Vec<isize>> = Mutex::new(Vec::new());

fn live() -> MutexGuard<'static, Vec<isize>> {
    LIVE.lock().unwrap_or_else(|e| e.into_inner())
}

/// Registers an exec child in `LIVE` while it runs.
struct Live(isize);

impl Live {
    fn new(id: isize) -> Live {
        #[cfg(unix)]
        {
            extern "C" fn at_exit() {
                kill_live_children();
            }
            static AT_EXIT: std::sync::Once = std::sync::Once::new();
            // SAFETY: registers a plain `extern "C" fn` that only takes a lock and sends signals.
            AT_EXIT.call_once(|| unsafe {
                libc::atexit(at_exit);
            });
        }
        live().push(id);
        Live(id)
    }
}

impl Drop for Live {
    fn drop(&mut self) {
        live().retain(|&g| g != self.0);
    }
}

/// Kill every exec child still running, with everything it started, so no generator outlives the session (a hanging
/// one would run forever, reparented to init). Unix: also runs at process exit (`atexit`, i.e. `std::process::exit`).
pub fn kill_live_children() {
    for &id in live().iter() {
        #[cfg(unix)]
        // SAFETY: plain syscall; `id` is the process-group id of a child that called setsid.
        unsafe {
            libc::kill(-(id as libc::pid_t), libc::SIGKILL);
        }
        #[cfg(windows)]
        job::terminate(id as windows_sys::Win32::Foundation::HANDLE);
    }
}

/// Windows job object holding an exec child and every process it starts, so a timeout kills the whole tree (std's
/// `Child::kill` only ends the direct child; grandchildren would keep the output pipes and reader threads alive).
#[cfg(windows)]
mod job {
    use std::ffi::c_void;
    use std::os::windows::io::AsRawHandle;
    use windows_sys::Win32::Foundation::{CloseHandle, HANDLE};

    // ponytail: declared here because windows-sys only has them behind its `Win32_System_JobObjects` feature.
    #[link(name = "kernel32")]
    extern "system" {
        fn CreateJobObjectW(attrs: *const c_void, name: *const u16) -> HANDLE;
        fn AssignProcessToJobObject(job: HANDLE, process: HANDLE) -> i32;
        fn TerminateJobObject(job: HANDLE, exit_code: u32) -> i32;
    }

    pub struct Job(HANDLE);

    impl Job {
        /// `None` if no job could be created or assigned (then a timeout kills only the child).
        /// ponytail: assigned right after spawn, so a process the child starts in that instant escapes; closing the
        /// gap needs CREATE_SUSPENDED + resuming the main thread, which std's `Command` does not expose.
        pub fn assign(child: &std::process::Child) -> Option<Job> {
            // SAFETY: plain Win32 calls; the child's process handle is valid while `child` lives.
            unsafe {
                let job = CreateJobObjectW(std::ptr::null(), std::ptr::null());
                if job.is_null() {
                    return None;
                }
                let job = Job(job);
                (AssignProcessToJobObject(job.0, child.as_raw_handle() as HANDLE) != 0)
                    .then_some(job)
            }
        }

        pub fn kill(&self) {
            terminate(self.0);
        }

        pub fn handle(&self) -> HANDLE {
            self.0
        }
    }

    /// `job` must be an open job handle (`kill_live_children` holds the `LIVE` lock, so it is not closed meanwhile).
    pub fn terminate(job: HANDLE) {
        // SAFETY: see above.
        unsafe { TerminateJobObject(job, 1) };
    }

    impl Drop for Job {
        fn drop(&mut self) {
            // SAFETY: closed exactly once; no kill-on-close limit, so processes still running are left alone.
            unsafe { CloseHandle(self.0) };
        }
    }
}

/// Cleans exec output: lossy UTF-8, `\r\n` → `\n`, drop `ESC[?25h`, strip leading/trailing newlines.
fn clean(b: &[u8]) -> String {
    String::from_utf8_lossy(b)
        .replace("\r\n", "\n")
        .replace("\x1b[?25h", "")
        .trim_matches('\n')
        .to_string()
}

/// Native answer to `ls -1ApL` in `dir` (SPEC §6.6.4, §9.3).
pub fn native_ls(dir: &Path) -> ExecOutput {
    match std::fs::read_dir(dir) {
        Ok(entries) => {
            let names: Vec<String> = entries
                .flatten()
                .map(|e| {
                    let mut name = e.file_name().to_string_lossy().into_owned();
                    if std::fs::metadata(e.path()).is_ok_and(|m| m.is_dir()) {
                        name.push('/'); // -p, following symlinks (-L)
                    }
                    name
                })
                .collect();
            ExecOutput {
                stdout: names.join("\n"),
                stderr: String::new(),
                status: 0,
            }
        }
        Err(e) => ExecOutput {
            stdout: String::new(),
            stderr: format!("ls: {}: {e}", dir.display()),
            status: 2,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::specs::infrastructure::adapters::fig_spec_mapper::fn_id;
    use crate::shared::infrastructure::testing::temp_dir::TempDir;
    use crate::shared::domain::value_objects::shell::Shell;
    use crate::types::Request;

    fn js() -> (Js, Sender<EngineMsg>) {
        let (tx, rx) = mpsc::channel();
        (Js::new(rx, tx.clone(), Settings::default()).unwrap(), tx)
    }

    /// Write a module `name` with `src` into `dir` and load it.
    fn module(js: &mut Js, dir: &TempDir, name: &str, src: &str) -> Map<String, Value> {
        let p = dir.path().join(name);
        std::fs::write(&p, src).unwrap();
        js.load_module(&p).unwrap()
    }

    fn call_default(
        js: &mut Js,
        ns: &Map<String, Value>,
        args: Vec<JsArg>,
        ctx: Option<&GenCtx>,
    ) -> Result<Value, SpecError> {
        js.call(
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

    /// A temp dir holding executable shell scripts, for hermetic "fake" commands.
    #[cfg(unix)]
    fn fake_bin(scripts: &[(&str, &str)]) -> TempDir {
        use std::os::unix::fs::PermissionsExt;
        let dir = TempDir::new("bin");
        for (name, body) in scripts {
            let p = dir.path().join(name);
            std::fs::write(&p, format!("#!/bin/sh\n{body}\n")).unwrap();
            std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o755)).unwrap();
        }
        dir
    }

    #[test]
    fn wrap_module_rewrites_export_clause() {
        let s = wrap_module("var t=1,a=2;export{t as default,a as versions, b};").unwrap();
        assert_eq!(
            s,
            "__ts.json((function(){var t=1,a=2;\n;return {default:t,versions:a,b:b};})())"
        );
        assert!(wrap_module("var x = 1;").is_none());
        // `export` inside the body is not mistaken for the clause.
        assert!(wrap_module("var e=\"export\";export {e as default};\n")
            .unwrap()
            .ends_with("return {default:e};})())"));
    }

    #[test]
    fn loads_hand_written_es_modules() {
        let (mut js, _tx) = js();
        let dir = TempDir::new("esm");
        let src = "export const sub = { name: \"ok\" };\nconst spec = { name: \"zz\", subcommands: [sub] };\nexport default spec;\n";
        let ns = module(&mut js, &dir, "zz.js", src);
        assert_eq!(
            ns["default"],
            json!({"name": "zz", "subcommands": [{"name": "ok"}]})
        );
        let ns = module(
            &mut js,
            &dir,
            "inline.js",
            "export default {\n  name: \"inline\",\n} // end",
        );
        assert_eq!(ns["default"], json!({"name": "inline"}));
        // The esbuild form still wins.
        assert_eq!(
            module(
                &mut js,
                &dir,
                "eb.js",
                "var s={name:\"eb\"};export{s as default};"
            )["default"],
            json!({"name": "eb"})
        );
    }

    #[test]
    fn serializes_functions_and_values() {
        let (mut js, _tx) = js();
        let dir = TempDir::new("ser");
        let ns = module(
            &mut js,
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
            js.call(fa, vec![], None, Duration::from_secs(2)),
            Ok(json!(1))
        );
        // Empty file → empty namespace; syntax error → Other.
        assert!(module(&mut js, &dir, "empty.js", " \n").is_empty());
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
        let (mut js, _tx) = js();
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
        let out = js.call(
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
            let r = call_default(&mut js, &heroku, vec![JsArg::Json(v.clone())], None).unwrap();
            assert_eq!(r["versionedSpecPath"], want, "version {v}");
        }
    }

    /// `parserDirectives.alias` of git.js run against a fake `git` on a private PATH.
    #[cfg(all(unix, feature = "embed-specs"))]
    #[test]
    fn git_alias_directive_with_fake_git() {
        let bin = fake_bin(&[("git", r#"[ "$3" = alias.co ] && echo checkout || exit 1"#)]);
        let b = crate::specs::infrastructure::adapters::embedded_spec_bundle::testing::bundle_subset(&["git.js"]);
        let (mut js, _tx) = js();
        let git = js.load_module(&b.path().join("git.js")).unwrap();
        let alias = fn_id(
            find_key(&git["default"], "parserDirectives")
                .unwrap()
                .get("alias")
                .unwrap(),
        )
        .unwrap();
        let ctx = GenCtx {
            env: [("PATH".to_string(), bin.path().display().to_string())].into(),
            ..Default::default()
        };
        let run = |js: &mut Js, tok: &str| {
            js.call(
                alias,
                vec![JsArg::Json(tok.into()), JsArg::Exec],
                Some(&ctx),
                Duration::from_secs(5),
            )
        };
        assert_eq!(run(&mut js, "co"), Ok(json!("checkout")));
        assert!(
            matches!(run(&mut js, "nope"), Err(SpecError::Invalid(e)) if e.contains("Failed parsing alias"))
        );
    }

    /// A custom generator calling `executeShellCommand` (object and legacy string forms) through a fake `echo`.
    #[cfg(unix)]
    #[test]
    fn custom_generator_executes_commands() {
        let bin = fake_bin(&[("echo", r#"printf 'fake:%s\n' "$*""#)]);
        let cwd = TempDir::new("cwd");
        let (mut js, _tx) = js();
        let ns = module(
            &mut js,
            &cwd,
            "gen.js",
            "var g=async(tokens,exec,ctx)=>{let r=await exec({command:\"echo\",args:[tokens[0],ctx.searchTerm]});\
             let l=await exec(\"echo legacy | tr a-z A-Z\");let p=await exec({command:\"sh\",args:[\"-c\",\"pwd -P; printf %s $NO_COLOR$X\"],env:{X:\"y\"}});\
             let e;try{await exec({command:\"no-such-cmd-tabsmith\"})}catch(x){e=String(x)}\
             return [r.stdout,r.status,l,p.stdout,e]};export{g as default};",
        );
        let path = format!("{}:/bin:/usr/bin", bin.path().display());
        let ctx = GenCtx {
            cwd: cwd.path().display().to_string(),
            env: [("PATH".to_string(), path)].into(),
            search_term: "term".into(),
            ..Default::default()
        };
        let args = vec![
            JsArg::Json(json!(["git"])),
            JsArg::Exec,
            JsArg::Json(ctx.context_json()),
        ];
        let out = call_default(&mut js, &ns, args, Some(&ctx)).unwrap();
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
    fn native_ls_and_short_circuits() {
        let dir = TempDir::new("ls");
        std::fs::write(dir.path().join("a.txt"), "").unwrap();
        std::fs::write(dir.path().join(".hidden"), "").unwrap();
        std::fs::create_dir(dir.path().join("sub")).unwrap();
        #[cfg(unix)]
        std::os::unix::fs::symlink(dir.path().join("sub"), dir.path().join("link")).unwrap();
        let mut got: Vec<String> = native_ls(dir.path())
            .stdout
            .lines()
            .map(String::from)
            .collect();
        got.sort();
        let mut want = vec![".hidden", "a.txt", "sub/"];
        if cfg!(unix) {
            want.insert(2, "link/");
        }
        assert_eq!(got, want);
        let missing = native_ls(&dir.path().join("nope"));
        assert_eq!((missing.status, missing.stdout.as_str()), (2, ""));

        // exec answers `ls -1ApL` natively (args trimmed; no input.cwd → ctx cwd; relative → under ctx cwd; missing
        // dir → error, not the ctx cwd's listing) and stubs fig/tabsmith.
        let ctx = GenCtx {
            cwd: dir.path().display().to_string(),
            ..Default::default()
        };
        let ls = |cwd: Option<&str>| {
            let input = ExecInput {
                command: "ls".into(),
                args: vec!["-1ApL ".into()],
                cwd: cwd.map(String::from),
                ..Default::default()
            };
            exec(&input, &ctx, 5000).unwrap()
        };
        assert_eq!(ls(None), native_ls(dir.path()));
        assert_eq!(ls(Some("sub/")), native_ls(&dir.path().join("sub")));
        let missing = ls(Some(&dir.path().join("nonexist/").display().to_string()));
        assert_eq!((missing.status, missing.stdout.as_str()), (2, ""));
        for cmd in ["fig", "tabsmith"] {
            let input = ExecInput {
                command: cmd.into(),
                ..Default::default()
            };
            assert_eq!(
                exec(&input, &ctx, 5000),
                Ok(ExecOutput {
                    status: 1,
                    ..Default::default()
                })
            );
        }
    }

    #[test]
    fn exec_reports_an_unknown_command_instead_of_stubbing_it() {
        let empty_path = TempDir::new("empty-path");
        let ctx = GenCtx {
            env: HashMap::from([("PATH".to_string(), empty_path.path().display().to_string())]),
            ..Default::default()
        };

        let input = ExecInput {
            command: "no-such-cli".into(),
            ..Default::default()
        };

        let result = exec(&input, &ctx, 5000);

        assert!(result.unwrap_err().contains("not found"));
    }

    #[cfg(unix)]
    #[test]
    fn exec_status_output_and_timeout() {
        let sh = |script: &str, timeout: Option<u64>| {
            let input = ExecInput {
                command: "sh".into(),
                args: vec!["-c".into(), script.into()],
                timeout,
                ..Default::default()
            };
            exec(&input, &GenCtx::default(), 100)
        };
        assert_eq!(
            sh(
                "printf '\\n\\nout\\r\\nx\\033[?25h\\n\\n'; echo err >&2; exit 3",
                None
            ),
            Ok(ExecOutput {
                stdout: "out\nx".into(),
                stderr: "err".into(),
                status: 3
            })
        );
        assert_eq!(
            sh("kill -9 $$", None).unwrap().status,
            1,
            "killed by a signal → 1"
        );
        assert_eq!(
            sh("echo $TERM $CLICOLOR", None).unwrap().stdout,
            "xterm-256color"
        );
        let t0 = Instant::now();
        let e = sh("sleep 5; echo late", None).unwrap_err();
        assert!(e.contains("timed out after 100 ms"), "{e}");
        assert!(t0.elapsed() < Duration::from_secs(2));
        assert_eq!(
            sh("sleep 0.2; echo ok", Some(3000)).unwrap().stdout,
            "ok",
            "input.timeout raises the floor"
        );
    }

    #[test]
    fn set_timeout_ordering_and_intl() {
        let (mut js, _tx) = js();
        let dir = TempDir::new("timers");
        let ns = module(
            &mut js,
            &dir,
            "t.js",
            "var f=()=>new Promise(res=>{const o=[];setTimeout(()=>o.push(\"b\"),30);setTimeout(x=>o.push(x),0,\"a\");\
             const c=setTimeout(()=>o.push(\"never\"),10);clearTimeout(c);setTimeout(()=>res(o),60)});export{f as default};",
        );
        assert_eq!(
            call_default(&mut js, &ns, vec![], None),
            Ok(json!(["a", "b"]))
        );
        let ns = module(
            &mut js,
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
                call_default(&mut js, &ns, vec![JsArg::Json(json!(n))], None),
                Ok(json!(want))
            );
        }
    }

    #[test]
    fn interrupt_stops_infinite_loop() {
        let (mut js, _tx) = js();
        let dir = TempDir::new("loop");
        let ns = module(
            &mut js,
            &dir,
            "l.js",
            "var f=()=>{while(true){}};var g=x=>x*2;export{f as default,g};",
        );
        let t0 = Instant::now();
        let r = js.call(
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
            js.call(
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
        let (mut js, _tx) = js();
        let dir = TempDir::new("slice");
        let ns = module(&mut js, &dir, "b.js", "var f=ms=>{const t=Date.now();while(Date.now()-t<ms){}return ms};export{f as default};");
        let f = fn_id(&ns["default"]).unwrap();
        let tasks = [200, 40].map(|ms| js.start_call(f, vec![JsArg::Json(json!(ms))], None));
        let t0 = Instant::now();
        let mut got = vec![None, None];
        while got.iter().any(Option::is_none) && t0.elapsed() < Duration::from_secs(3) {
            for (g, r) in got.iter_mut().zip(
                js.wait(&tasks, Instant::now() + Duration::from_millis(50))
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
        let (mut js, _tx) = js();
        let dir = TempDir::new("reload");
        assert_eq!(
            module(
                &mut js,
                &dir,
                "m.js",
                "var s={name:\"v1\"};export{s as default};"
            )["default"]["name"],
            "v1"
        );
        let ns = module(
            &mut js,
            &dir,
            "m.js",
            "var s={name:\"v2-edited\"};export{s as default};",
        );
        assert_eq!(ns["default"]["name"], "v2-edited");
    }

    /// cmd.exe looks for bare names in its cwd before PATH unless `NoDefaultCurrentDirectoryInExePath` is set.
    #[cfg(unix)]
    #[test]
    fn cmd_gets_no_cwd_search_env() {
        let bin = fake_bin(&[("cmd", r#"printf %s "$NoDefaultCurrentDirectoryInExePath""#)]);
        let ctx = GenCtx {
            env: [("PATH".to_string(), bin.path().display().to_string())].into(),
            ..Default::default()
        };
        let input = ExecInput {
            command: "cmd".into(),
            args: vec!["/C".into(), "git".into()],
            ..Default::default()
        };
        assert_eq!(exec(&input, &ctx, 5000).unwrap().stdout, "1");
    }

    /// A generator still running when the wrapper exits dies with it (re-runs this test in a child process that
    /// calls `std::process::exit` mid-exec, like the wrapper at the end of a session).
    #[cfg(unix)]
    #[test]
    fn exec_children_die_with_the_wrapper() {
        const NAME: &str = "js::tests::exec_children_die_with_the_wrapper";
        if std::env::var_os("TABSMITH_TEST_EXIT_MID_EXEC").is_some() {
            std::thread::spawn(|| {
                let args = vec!["-c".into(), "sleep 30; echo never".into()];
                exec(
                    &ExecInput {
                        command: "/bin/sh".into(),
                        args,
                        ..Default::default()
                    },
                    &GenCtx::default(),
                    60_000,
                )
            });
            let pgid = loop {
                if let Some(&g) = live().first() {
                    break g;
                }
                std::thread::sleep(Duration::from_millis(5));
            };
            std::thread::sleep(Duration::from_millis(100)); // let sh start `sleep`
            println!("pgid={pgid}");
            std::process::exit(0);
        }
        let out = Command::new(std::env::current_exe().unwrap())
            .args([NAME, "--exact", "--nocapture"])
            .env("TABSMITH_TEST_EXIT_MID_EXEC", "1")
            .output()
            .unwrap();
        let stdout = String::from_utf8_lossy(&out.stdout);
        let pgid: libc::pid_t = stdout
            .split("pgid=")
            .nth(1)
            .and_then(|r| r.split_whitespace().next())
            .unwrap()
            .parse()
            .unwrap();
        // SAFETY: plain syscalls on the child's process group.
        let gone = (0..300).any(|_| {
            std::thread::sleep(Duration::from_millis(10));
            (unsafe { libc::kill(-pgid, 0) }) == -1
        });
        if !gone {
            unsafe { libc::kill(-pgid, libc::SIGKILL) };
        }
        assert!(gone, "process group {pgid} outlived the wrapper");
    }

    /// A timeout kills the whole tree: the grandchild (inner cmd) never writes its marker.
    #[cfg(windows)]
    #[test]
    fn timeout_kills_grandchildren() {
        let dir = TempDir::new("tree");
        let args = [
            "/C",
            "cmd",
            "/C",
            "ping",
            "-n",
            "3",
            "127.0.0.1",
            "^>nul",
            "^&",
            "echo",
            "x^>marker",
        ];
        let input = ExecInput {
            command: "cmd".into(),
            args: args.map(String::from).to_vec(),
            cwd: Some(dir.path().display().to_string()),
            ..Default::default()
        };
        assert!(exec(&input, &GenCtx::default(), 300)
            .unwrap_err()
            .contains("timed out"));
        std::thread::sleep(Duration::from_secs(4));
        assert!(
            !dir.path().join("marker").exists(),
            "grandchild survived the timeout"
        );
    }

    #[test]
    fn wait_aborts_and_defers_on_complete() {
        let (mut js, tx) = js();
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
            js.wait(&[slow], Instant::now() + Duration::from_secs(5)),
            Err(SpecError::Aborted)
        );
        assert!(t0.elapsed() < Duration::from_secs(1));
        // Still queued → a new wait aborts at once.
        assert_eq!(
            js.wait(&[slow], Instant::now() + Duration::from_secs(5)),
            Err(SpecError::Aborted)
        );
        assert!(matches!(js.next_msg(), Some(EngineMsg::ToggleFuzzy)));
        assert!(matches!(js.next_msg(), Some(EngineMsg::Complete(r)) if r.id == 7));
        // Non-Complete messages are deferred without aborting; the slow task is still pending.
        let quick = js.start_sleep(10);
        let r = js
            .wait(&[quick, slow], Instant::now() + Duration::from_millis(300))
            .unwrap();
        assert_eq!(r, vec![Some(Ok(Value::Null)), None]);
        assert!(matches!(js.try_next_msg(), Some(EngineMsg::Shutdown)));
        assert!(js.try_next_msg().is_none());
        js.forget(&[slow]);
        let unknown = |t| Some(Err(format!("unknown task {t}")));
        assert_eq!(
            js.wait(&[slow, quick], Instant::now()).unwrap(),
            vec![unknown(slow), unknown(quick)]
        );
        // Exec tasks deliver `{stdout, stderr, status}`; ExecDone is consumed internally.
        let e = js.start_exec(
            ExecInput {
                command: "fig".into(),
                ..Default::default()
            },
            &GenCtx::default(),
        );
        let r = js
            .wait(&[e], Instant::now() + Duration::from_secs(5))
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
