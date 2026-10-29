#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum CompletionPhase {
    /// First reply: static suggestions (+ stale generator results).
    Parse,
    /// Follow-up reply after triggered generators finished.
    Generators,
}
