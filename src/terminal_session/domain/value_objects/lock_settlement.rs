/// What a look at the shell's buffer decided about the insertion lock.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum LockSettlement {
    /// Still waiting for the echo (or re-armed because nothing of it showed yet): no request yet.
    StillHeld,
    /// The echo came, or the shell did something else, or the cap ran out: the lock is over.
    Released,
}
