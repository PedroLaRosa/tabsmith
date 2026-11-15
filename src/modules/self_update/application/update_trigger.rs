/// What asked for the update.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum UpdateTrigger {
    /// `tabsmith update`.
    Manual,
    /// The background check when a terminal tab opens.
    Scheduled,
}
