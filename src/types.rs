//! Types shared between the wrapper (main thread), the UI and the completion engine thread (SPEC §4.3).
use crate::completion::domain::value_objects::argument_hint::ArgumentHint;
use crate::completion::domain::value_objects::completion_phase::CompletionPhase;
use crate::completion::domain::value_objects::generator_key::GeneratorKey;
use crate::completion::domain::value_objects::suggestion::Suggestion;
use crate::history::domain::entities::history_entry::HistoryEntry;
use crate::settings::domain::value_objects::settings::Settings;
use crate::shared::application::ports::clock::Clock;
use crate::shared::domain::value_objects::shell::Shell;
use crate::shared::infrastructure::system_clock::SystemClock;
use crate::specs::application::ports::runtime_event::RuntimeEvent;
use std::collections::HashMap;
use std::path::PathBuf;

/// Engine → wrapper reply (SPEC §4.3).
#[derive(Clone, Debug)]
pub struct Completion {
    pub req: u64,
    pub phase: CompletionPhase,
    pub error: bool,
    /// The buffer/cursor the reply was computed from (for insertion reconciliation, SPEC §7.3).
    pub buffer: String,
    pub cursor: usize,
    /// Token texts of the current command (after alias expansion).
    pub tokens: Vec<String>,
    pub command_text: String,
    pub command_index: usize,
    /// Identity of (current arg, current subcommand) — changes when a new argument starts.
    pub arg_key: u64,
    pub current_arg: Option<ArgumentHint>,
    pub search_term: String,
    pub triggered_generators: Vec<GeneratorKey>,
    pub items: Vec<Suggestion>,
    /// Some generator is still running.
    pub loading: bool,
    /// Local inline ghost text for this buffer (SPEC §6.11).
    pub ghost: Option<String>,
    /// Effective fuzzy mode for this reply.
    pub fuzzy: bool,
}

impl Completion {
    /// Error reply: no suggestions, menu error-reset.
    pub fn error(req: u64, buffer: String, cursor: usize) -> Completion {
        Completion {
            req,
            phase: CompletionPhase::Parse,
            error: true,
            buffer,
            cursor,
            tokens: Vec::new(),
            command_text: String::new(),
            command_index: 0,
            arg_key: 0,
            current_arg: None,
            search_term: String::new(),
            triggered_generators: Vec::new(),
            items: Vec::new(),
            loading: false,
            ghost: None,
            fuzzy: false,
        }
    }
}

/// Wrapper → engine completion request.
#[derive(Clone, Debug)]
pub struct Request {
    pub id: u64,
    pub buffer: String,
    /// Byte offset into `buffer`.
    pub cursor: usize,
    pub cwd: PathBuf,
    pub shell: Shell,
    pub history_mode: bool,
}

/// Messages to the wrapper's main loop.
pub enum Msg {
    Stdin(Vec<u8>),
    Pty(Vec<u8>),
    PtyEof,
    Resize(u16, u16),
    ChildExit(i32),
    Term,
    Completion(Box<Completion>),
    Ghost { req: u64, text: Option<String> },
}

/// Messages to the engine thread.
pub enum EngineMsg {
    Complete(Request),
    CommandFinished(HistoryEntry),
    /// A command finished but must not be recorded (NoHistory).
    CommandEnded,
    RecordInsert {
        cmd: String,
        name: String,
    },
    Llm {
        req: u64,
        buffer: String,
        cwd: PathBuf,
        shell: Shell,
    },
    Settings(Settings),
    Session {
        env: HashMap<String, String>,
        alias: Option<String>,
    },
    ToggleFuzzy,
    /// Internal (js host): an event for the spec runtime, e.g. a spawned process finished.
    Runtime(RuntimeEvent),
    Shutdown,
}

/// Delegates to [`SystemClock`] until every caller takes the `Clock` port (deleted in Phase 11).
pub fn now_ms() -> u64 {
    SystemClock.now_ms()
}

/// Delegates to [`SystemClock`] until every caller takes the `Clock` port (deleted in Phase 11).
pub fn now_secs() -> u64 {
    SystemClock.now_secs()
}

/// The headless terminal model the wrapper feeds with PTY output (alacritty_terminal, SPEC D4).
pub type Model = alacritty_terminal::Term<alacritty_terminal::event::VoidListener>;
