//! QuickJS runtime for Fig spec code (SPEC §9.2). Generator processes run in `process_executor` (SPEC §9.3); the
//! prelude and the module wrapper live in `quickjs_module_wrapper`.
//! Lives on the engine thread only (QuickJS is !Send). Every async JS call is resolved by a manual event pump
//! that polls a `RuntimeEventSource` (the engine's inbox): a newer completion request aborts the current wait.
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
use crate::specs::application::ports::runtime_event::RuntimeEvent;
use crate::specs::application::ports::runtime_event_poll::RuntimeEventPoll;
use crate::specs::application::ports::runtime_event_sink::RuntimeEventSink;
use crate::specs::application::ports::runtime_event_source::RuntimeEventSource;
use crate::specs::application::ports::interrupted::Interrupted;
use crate::specs::application::ports::spec_runtime::SpecRuntime;
use crate::specs::domain::spec_error::SpecError;
use crate::specs::domain::value_objects::exec_request::ExecRequest;
use crate::specs::domain::value_objects::generator_context::GeneratorContext;
use crate::specs::domain::value_objects::spec_function_call::SpecFunctionCall;
use crate::specs::domain::value_objects::spec_function_kind::SpecFunctionKind;
use crate::specs::domain::value_objects::spec_function_ref::SpecFunctionRef;
use crate::specs::domain::value_objects::spec_function_result::SpecFunctionResult;
use crate::specs::domain::value_objects::spec_value::SpecValue;
use crate::specs::domain::value_objects::task_id::TaskId;
use crate::specs::domain::value_objects::task_outcome::TaskOutcome;
use crate::specs::domain::value_objects::template_suggestions::TemplateSuggestions;
use crate::specs::infrastructure::adapters::exec_input::{process_output_json, ExecInput};
use crate::specs::infrastructure::adapters::fig_spec_mapper::{
    function_result, generator_context_json, spec_value_from_json, spec_value_to_json, template_json,
};
use crate::specs::infrastructure::adapters::process_executor::exec;
use crate::specs::infrastructure::adapters::quickjs_module_wrapper::{wrap_module, PRELUDE};
use rquickjs::context::EvalOptions;
use rquickjs::{CatchResultExt, Context, Function, Object, Runtime};
use serde_json::{json, Map, Value};
use std::cell::{Cell, RefCell};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::time::{Duration, Instant, SystemTime};

/// One argument to a JS callback.
#[derive(Clone, Debug, PartialEq)]
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
    sink: RuntimeEventSink,
    /// `autocomplete.scriptTimeout`.
    timeout_ms: u64,
}

impl TaskRegistry {
    fn id(&mut self) -> u64 {
        self.next_id += 1;
        self.next_id
    }

    /// Run `exec` on a worker thread; the result comes back through the sink as a `RuntimeEvent { id, .. }`.
    fn spawn_exec(&self, id: u64, input: ExecRequest, ctx: GeneratorContext) {
        let (sink, timeout) = (self.sink.clone(), self.timeout_ms);
        let spawned = std::thread::Builder::new()
            .name("tabsmith-exec".into())
            .spawn({
                let sink = sink.clone();
                move || {
                    let output = exec(&input, &ctx, timeout);
                    sink(RuntimeEvent { id, output });
                }
            });
        if let Err(e) = spawned {
            sink(RuntimeEvent {
                id,
                output: Err(format!("spawning exec thread: {e}")),
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
    events: Rc<dyn RuntimeEventSource>,
    /// Serialized namespaces by path, with the file's (mtime, len) when evaluated: an edited file (developer mode)
    /// is evaluated again. Old fn ids stay valid (`__ts.fns` only grows).
    modules: RefCell<ModuleCache>,
}

/// A file's (mtime, len); `None` if it cannot be stat'ed.
type Stamp = Option<(Option<SystemTime>, u64)>;

type ModuleCache = HashMap<PathBuf, (Stamp, Map<String, Value>)>;

impl QuickJsSpecRuntime {
    /// Create the runtime (memory limit, interrupt handler), evaluate the prelude.
    /// `events` is the engine's inbox: exec workers post into its sink, and waits poll it.
    /// Note: the task registry keeps that sink for its whole life, so the inbox never disconnects; the engine stops
    /// on its shutdown message.
    pub(crate) fn new(events: Rc<dyn RuntimeEventSource>, settings: &Settings) -> Result<Self, SpecError> {
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
            sink: events.sink(),
            timeout_ms: script_timeout(settings),
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
                                (h.sink)(RuntimeEvent {
                                    id,
                                    output: Err(format!("bad exec input: {e}")),
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
            events,
            modules: RefCell::new(HashMap::new()),
        })
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

    /// Pump until every task finished or `deadline`. Returns one entry per task (`None` = still pending).
    /// Pending tasks stay alive and can be waited on again; finished results are delivered once (then dropped,
    /// like `forget`; an unknown/forgotten task yields `Some(Err)`). `Err(Aborted)` when the event source
    /// reports a newer request (`has_newer_request`); no task is dropped then.
    pub(crate) fn wait_json(
        &self,
        tasks: &[TaskId],
        deadline: Instant,
    ) -> Result<Vec<Option<TaskResult>>, SpecError> {
        loop {
            if self.events.has_newer_request() {
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
            match self.events.poll(next.saturating_duration_since(now)) {
                RuntimeEventPoll::Event(e) => self.handle_event(e),
                RuntimeEventPoll::MessageKept | RuntimeEventPoll::TimedOut => {}
                RuntimeEventPoll::Disconnected => return Ok(self.collect(tasks)),
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

    fn arm(&self, until: Instant) {
        self.deadline.set(Some(until));
    }

    fn arm_off(&self) {
        self.deadline.set(None);
    }

    /// Deliver a runtime event: the exec result `{stdout, stderr, status}` (or its error) of host op `id`.
    pub(crate) fn handle_event(&self, event: RuntimeEvent) {
        let RuntimeEvent { id, output } = event;
        self.finish(id, output.map(|o| process_output_json(&o)));
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

impl SpecRuntime for QuickJsSpecRuntime {
    fn start(&self, call: SpecFunctionCall, context: Option<&GeneratorContext>) -> TaskId {
        let default = GeneratorContext::default();
        let (function, args) = call_arguments(&call, context.unwrap_or(&default));
        self.start_call(function, args, context)
    }

    /// Start a native process (SPEC §9.3) on a worker thread; result `{stdout, stderr, status}`.
    fn start_exec(&self, input: ExecRequest, ctx: &GeneratorContext) -> TaskId {
        let mut h = self.tasks.borrow_mut();
        let id = h.id();
        h.live.insert(id);
        h.spawn_exec(id, input, ctx.clone());
        id
    }

    /// Start a timer task that completes (with `null`) after `ms` (generator debounce).
    fn start_sleep(&self, ms: u64) -> TaskId {
        let mut h = self.tasks.borrow_mut();
        let id = h.id();
        h.live.insert(id);
        h.timers
            .push((Instant::now() + Duration::from_millis(ms), id));
        id
    }

    fn wait(
        &self,
        tasks: &[TaskId],
        deadline: Instant,
    ) -> Result<Vec<Option<TaskOutcome>>, Interrupted> {
        match self.wait_json(tasks, deadline) {
            Ok(results) => Ok(results.into_iter().map(|r| r.map(task_outcome)).collect()),
            Err(SpecError::Aborted) => Err(Interrupted),
            // wait_json only fails with Aborted; any other error fails every task.
            Err(
                e @ (SpecError::Timeout | SpecError::Invalid(_) | SpecError::BundleUnavailable(_)),
            ) => Ok(vec![Some(Err(e.to_string())); tasks.len()]),
        }
    }

    /// Drop the result/bookkeeping of tasks the caller no longer cares about.
    fn forget(&self, tasks: &[TaskId]) {
        let mut h = self.tasks.borrow_mut();
        for t in tasks {
            h.live.remove(t);
            h.done.remove(t);
            h.ctxs.remove(t);
            h.timers.retain(|x| x.1 != *t);
        }
    }

    fn call(
        &self,
        call: SpecFunctionCall,
        context: Option<&GeneratorContext>,
        timeout: Duration,
    ) -> Result<SpecFunctionResult, SpecError> {
        let default = GeneratorContext::default();
        let generator = context.unwrap_or(&default);
        let (function, args) = call_arguments(&call, generator);
        let value = self.call_json(function, args, context, timeout)?;
        Ok(function_result(call.kind(), &value, generator))
    }

    fn read(
        &self,
        value: &SpecValue,
        kind: SpecFunctionKind,
        context: &GeneratorContext,
    ) -> SpecFunctionResult {
        function_result(kind, &spec_value_to_json(value), context)
    }

    fn read_exec_stdout(&self, value: &SpecValue) -> String {
        spec_value_to_json(value)
            .get("stdout")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string()
    }

    fn apply_settings(&self, settings: &Settings) {
        self.tasks.borrow_mut().timeout_ms = script_timeout(settings);
    }
}

/// A finished task's JSON result as its outcome.
fn task_outcome(result: TaskResult) -> TaskOutcome {
    result.map(|v| spec_value_from_json(&v))
}

/// The function a spec function call runs, and the JS arguments its kind passes.
pub(crate) fn call_arguments(
    call: &SpecFunctionCall,
    context: &GeneratorContext,
) -> (SpecFunctionRef, Vec<JsArg>) {
    let arguments = match call {
        SpecFunctionCall::QueryTerm { search_term, .. } => vec![JsArg::Json(json!(search_term))],
        SpecFunctionCall::Trigger {
            new_token,
            old_token,
            ..
        } => vec![JsArg::Json(json!(new_token)), JsArg::Json(json!(old_token))],
        SpecFunctionCall::Alias { token, .. } | SpecFunctionCall::LoadSpec { token, .. } => {
            vec![JsArg::Json(json!(token)), JsArg::Exec]
        }
        SpecFunctionCall::GenerateSpecCacheKey { tokens, .. } => {
            vec![JsArg::Json(json!({ "tokens": tokens }))]
        }
        SpecFunctionCall::GenerateSpec { tokens, .. } => {
            vec![JsArg::Json(json!(tokens)), JsArg::Exec]
        }
        SpecFunctionCall::Script { tokens, .. } => vec![JsArg::Json(json!(tokens))],
        SpecFunctionCall::Custom { tokens, .. } => vec![
            JsArg::Json(json!(tokens)),
            JsArg::Exec,
            JsArg::Json(generator_context_json(context)),
        ],
        SpecFunctionCall::PostProcess { stdout, tokens, .. } => {
            vec![JsArg::Json(json!(stdout)), JsArg::Json(json!(tokens))]
        }
        SpecFunctionCall::FilterTemplateSuggestions {
            suggestions: TemplateSuggestions::Listed(listed),
            ..
        } => vec![JsArg::Json(template_json(listed))],
        SpecFunctionCall::FilterTemplateSuggestions {
            suggestions: TemplateSuggestions::Returned(returned),
            ..
        } => vec![JsArg::Json(spec_value_to_json(returned))],
    };
    (call.function(), arguments)
}

#[cfg(test)]
mod tests;
