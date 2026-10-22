//! QuickJS host for Fig specs (SPEC §9.2) and the native `exec` used by generators (SPEC §9.3).
//! Lives on the engine thread only (QuickJS is !Send). Every async JS call is resolved by a manual event pump
//! that also receives the engine's messages: a newer `EngineMsg::Complete` aborts the current wait.
//! OWNER: host agent.
use crate::settings::Settings;
use crate::spec::FnId;
use crate::types::{EngineMsg, ExecOutput};
use serde::Deserialize;
use serde_json::{Map, Value};
use std::collections::HashMap;
use std::path::Path;
use std::sync::mpsc::{Receiver, Sender};
use std::time::{Duration, Instant};

#[derive(Debug, Clone, PartialEq)]
pub enum JsError {
    /// A newer completion request arrived; it is queued and returned by the next `next_msg()`.
    Aborted,
    Timeout,
    Other(String),
}

impl std::fmt::Display for JsError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            JsError::Aborted => write!(f, "aborted by a newer request"),
            JsError::Timeout => write!(f, "timed out"),
            JsError::Other(s) => write!(f, "{s}"),
        }
    }
}

impl std::error::Error for JsError {}

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

pub struct Js {
    // host agent: rquickjs Runtime/Context, function table, pending exec promises, timers,
    // deferred engine messages, settings snapshot, task table.
    _private: (),
}

impl Js {
    /// Create the runtime (memory limit, interrupt handler), evaluate the prelude.
    /// `rx` is the engine's inbox; `tx` is a sender to that same inbox (for exec workers and timers).
    pub fn new(rx: Receiver<EngineMsg>, tx: Sender<EngineMsg>, settings: Settings) -> anyhow::Result<Js> {
        let _ = (rx, tx, settings);
        todo!("host agent")
    }

    /// Next message for the engine loop: deferred messages first (incl. a request that aborted a wait), then a
    /// blocking receive. `ExecDone`/`Timer` are consumed internally. `None` when all senders are gone.
    pub fn next_msg(&mut self) -> Option<EngineMsg> {
        todo!("host agent")
    }

    /// Like `next_msg` but non-blocking.
    pub fn try_next_msg(&mut self) -> Option<EngineMsg> {
        todo!("host agent")
    }

    pub fn set_settings(&mut self, settings: Settings) {
        let _ = settings;
        todo!("host agent")
    }

    /// Evaluate a spec module once (cached by path) and return its exports, serialized
    /// (functions → `{"$fn": id}`), e.g. `{"default": {...}, "versions": {...}, "getVersionCommand": {"$fn": 3}}`.
    /// Empty file → empty map. 5 s deadline.
    pub fn load_module(&mut self, path: &Path) -> Result<Map<String, Value>, JsError> {
        let _ = path;
        todo!("host agent")
    }

    /// Start a JS callback `fns[f](...args)`; the result is the serialized return value (Promises awaited).
    pub fn start_call(&mut self, f: FnId, args: Vec<JsArg>, ctx: Option<&GenCtx>) -> TaskId {
        let _ = (f, args, ctx);
        todo!("host agent")
    }

    /// Start a native process (SPEC §9.3) on a worker thread; result `{stdout, stderr, status}`.
    pub fn start_exec(&mut self, input: ExecInput, ctx: &GenCtx) -> TaskId {
        let _ = (input, ctx);
        todo!("host agent")
    }

    /// Pump until every task finished or `deadline`. Returns one entry per task (`None` = still pending).
    /// Pending tasks stay alive and can be waited on again. `Err(Aborted)` when a newer `Complete` arrived.
    pub fn wait(&mut self, tasks: &[TaskId], deadline: Instant) -> Result<Vec<Option<TaskResult>>, JsError> {
        let _ = (tasks, deadline);
        todo!("host agent")
    }

    /// Convenience: start + wait for one callback.
    pub fn call(&mut self, f: FnId, args: Vec<JsArg>, ctx: Option<&GenCtx>, timeout: Duration) -> Result<Value, JsError> {
        let _ = (f, args, ctx, timeout);
        todo!("host agent")
    }

    /// Start a timer task that completes (with `null`) after `ms` (generator debounce).
    pub fn start_sleep(&mut self, ms: u64) -> TaskId {
        let _ = ms;
        todo!("host agent")
    }

    /// Drop the result/bookkeeping of tasks the caller no longer cares about.
    pub fn forget(&mut self, tasks: &[TaskId]) {
        let _ = tasks;
        todo!("host agent")
    }
}

/// Run a process with RunProcess semantics (SPEC §9.3), blocking. Used by exec workers.
/// `default_timeout_ms` = `autocomplete.scriptTimeout` (5000; Windows 20000).
pub fn exec(input: &ExecInput, ctx: &GenCtx, default_timeout_ms: u64) -> Result<ExecOutput, String> {
    let _ = (input, ctx, default_timeout_ms);
    todo!("host agent")
}

/// Native answer to `ls -1ApL` in `dir` (SPEC §6.6.4, §9.3).
pub fn native_ls(dir: &Path) -> ExecOutput {
    let _ = dir;
    todo!("host agent")
}
