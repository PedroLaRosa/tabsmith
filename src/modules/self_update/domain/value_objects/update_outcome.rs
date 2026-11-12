use crate::modules::self_update::domain::value_objects::skip_reason::SkipReason;

/// How an update ended when nothing went wrong.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum UpdateOutcome {
    /// A background check that did not look for a release.
    Skipped(SkipReason),
    /// The running version is the latest release.
    UpToDate(String),
    /// The executable now holds the release of version `to`; the running process is still `from`.
    Updated { from: String, to: String },
}
