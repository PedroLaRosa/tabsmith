/// What asked for the update.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum UpdateTrigger {
    /// `tabsmith update`.
    Manual,
}
