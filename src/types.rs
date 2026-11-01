//! Types shared between the wrapper (main thread), the UI and the completion engine thread (SPEC §4.3).
use crate::shared::application::ports::clock::Clock;
use crate::shared::infrastructure::system_clock::SystemClock;

/// Delegates to [`SystemClock`] until every caller takes the `Clock` port (deleted in Phase 11).
pub fn now_ms() -> u64 {
    SystemClock.now_ms()
}

/// Delegates to [`SystemClock`] until every caller takes the `Clock` port (deleted in Phase 11).
pub fn now_secs() -> u64 {
    SystemClock.now_secs()
}
