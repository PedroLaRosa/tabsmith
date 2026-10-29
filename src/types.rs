//! Types shared between the wrapper (main thread), the UI and the completion engine thread (SPEC §4.3).
use crate::completion::application::completion_request::CompletionRequest;
use crate::completion::domain::value_objects::completion_result::CompletionResult;
use crate::history::domain::entities::history_entry::HistoryEntry;
use crate::settings::domain::value_objects::settings::Settings;
use crate::shared::application::ports::clock::Clock;
use crate::shared::domain::value_objects::shell::Shell;
use crate::shared::infrastructure::system_clock::SystemClock;
use crate::specs::application::ports::runtime_event::RuntimeEvent;
use std::collections::HashMap;
use std::path::PathBuf;

/// Messages to the wrapper's main loop.
pub enum Msg {
    Stdin(Vec<u8>),
    Pty(Vec<u8>),
    PtyEof,
    Resize(u16, u16),
    ChildExit(i32),
    Term,
    Completion(Box<CompletionResult>),
    Ghost { req: u64, text: Option<String> },
}

/// Messages to the engine thread.
pub enum EngineMsg {
    Complete(CompletionRequest),
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
