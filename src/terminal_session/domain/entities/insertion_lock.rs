use crate::terminal_session::domain::value_objects::lock_settlement::LockSettlement;
use std::time::{Duration, Instant};

/// A shell that has seen an insertion gets this long to echo it (and keys held behind an unmodelled key wait this
/// long for their reply).
pub(crate) const INSERT_LOCK: Duration = Duration::from_millis(100);
/// A shell that has echoed nothing of an insertion keeps it in the basis this long at most.
const LOCK_CAP: Duration = Duration::from_secs(1);

/// An insertion the shell has not echoed yet (SPEC §7.3): until it shows the expected buffer the menu and the
/// completion requests wait, and the buffer the user will see is the expected one. Re-armed up to the cap while the
/// shell has echoed nothing of the insertion (a slow shell).
#[derive(Clone, Debug)]
pub(crate) struct InsertionLock {
    deadline: Instant,
    cap: Instant,
    expected: String,
    before: String,
}

impl InsertionLock {
    /// `expected` = the buffer once the shell applied the insertion; `before` = the buffer it had.
    pub(crate) fn new(now: Instant, expected: String, before: String) -> Self {
        InsertionLock {
            deadline: now + INSERT_LOCK,
            cap: now + LOCK_CAP,
            expected,
            before,
        }
    }

    pub(crate) fn expected(&self) -> &str {
        &self.expected
    }

    pub(crate) fn deadline(&self) -> Instant {
        self.deadline
    }

    pub(crate) fn is_due(&self, now: Instant) -> bool {
        self.deadline <= now
    }

    /// `shown` = the buffer the shell shows now.
    pub(crate) fn settle(&mut self, shown: Option<&str>, now: Instant) -> LockSettlement {
        let applied = shown == Some(self.expected.as_str());
        if !applied && now < self.deadline {
            return LockSettlement::StillHeld;
        }
        // Nothing of it echoed yet (a slow shell): it stays in the basis, or it would be written twice.
        if !applied && now < self.cap && shown == Some(self.before.as_str()) {
            self.deadline = now + INSERT_LOCK;
            return LockSettlement::StillHeld;
        }
        LockSettlement::Released
    }
}

#[cfg(test)]
impl InsertionLock {
    /// Test arrangement: only the deadline is due ("re-armed: nothing echoed").
    pub(crate) fn expire_deadline_at(&mut self, deadline: Instant) {
        self.deadline = deadline;
    }

    /// Test arrangement: the deadline and the cap both due ("Never past the cap").
    pub(crate) fn expire_at(&mut self, deadline: Instant, cap: Instant) {
        self.deadline = deadline;
        self.cap = cap;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lock(at: Instant) -> InsertionLock {
        InsertionLock::new(at, "git checkout".into(), "git chec".into())
    }

    #[test]
    fn should_hold_until_the_deadline_while_the_shell_shows_something_else() {
        let start = Instant::now();
        let mut lock = lock(start);

        let settlement = lock.settle(Some("git che"), start + Duration::from_millis(99));

        assert_eq!(settlement, LockSettlement::StillHeld);
    }

    #[test]
    fn should_release_once_the_shell_shows_the_expected_buffer() {
        let start = Instant::now();
        let mut lock = lock(start);

        let settlement = lock.settle(Some("git checkout"), start + Duration::from_millis(1));

        assert_eq!(settlement, LockSettlement::Released);
    }

    #[test]
    fn should_rearm_when_nothing_was_echoed_after_the_deadline_and_before_the_cap() {
        let start = Instant::now();
        let mut lock = lock(start);
        let late = start + Duration::from_millis(150);

        let settlement = lock.settle(Some("git chec"), late);

        assert_eq!((settlement, lock.deadline()), (LockSettlement::StillHeld, late + INSERT_LOCK));
    }

    #[test]
    fn should_release_past_the_cap_even_when_nothing_was_echoed() {
        let start = Instant::now();
        let mut lock = lock(start);

        let settlement = lock.settle(Some("git chec"), start + Duration::from_secs(1));

        assert_eq!(settlement, LockSettlement::Released);
    }

    #[test]
    fn should_release_after_the_deadline_when_the_shell_did_something_else() {
        let start = Instant::now();
        let mut lock = lock(start);

        let settlement = lock.settle(Some("ls"), start + Duration::from_millis(150));

        assert_eq!(settlement, LockSettlement::Released);
    }

    #[test]
    fn should_release_after_the_deadline_when_there_is_no_buffer() {
        let start = Instant::now();
        let mut lock = lock(start);

        let settlement = lock.settle(None, start + Duration::from_millis(150));

        assert_eq!(settlement, LockSettlement::Released);
    }
}
