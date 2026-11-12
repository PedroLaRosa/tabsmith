/// How an update ended when nothing went wrong.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum UpdateOutcome {
    /// The running version is the latest release.
    UpToDate(String),
}
