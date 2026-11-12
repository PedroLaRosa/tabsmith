/// A wait stopped because a newer completion request is waiting; no task was dropped.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Interrupted;
