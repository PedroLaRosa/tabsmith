use crate::modules::history::domain::entities::history_entry::HistoryEntry;

/// What the wrapper must react to after a batch.
#[derive(Debug, PartialEq)]
pub(crate) enum ShellEvent {
    /// New prompt: menu error-reset, settings re-check; carries the finished command, if any (SPEC §3.5).
    NewCommand(Option<HistoryEntry>),
    /// New prompt after a command that must not be recorded (`NoHistory`): finished like `NewCommand(Some)`.
    CommandEnded,
    /// A command started: hide menu + ghost, cancel the insertion lock.
    PreExec,
}
