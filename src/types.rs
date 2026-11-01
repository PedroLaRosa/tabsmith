//! Types shared between the wrapper (main thread), the UI and the completion engine thread (SPEC §4.3).
use crate::completion::domain::value_objects::completion_result::CompletionResult;
use crate::shared::application::ports::clock::Clock;
use crate::shared::infrastructure::system_clock::SystemClock;

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

/// Delegates to [`SystemClock`] until every caller takes the `Clock` port (deleted in Phase 11).
pub fn now_ms() -> u64 {
    SystemClock.now_ms()
}

/// Delegates to [`SystemClock`] until every caller takes the `Clock` port (deleted in Phase 11).
pub fn now_secs() -> u64 {
    SystemClock.now_secs()
}
