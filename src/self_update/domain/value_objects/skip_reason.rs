/// Why a background update check did nothing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SkipReason {
    /// `app.disableAutoupdates` is set.
    Disabled,
    /// The last check was less than a day ago.
    NotDue,
}
