#[cfg(test)]
use std::cell::Cell;

/// The reminder that terminals opened before an install need a restart (`doctor.prompt-restart-terminal` in
/// `state.json`, which doctor reads and clears).
pub(crate) trait RestartReminderRepository {
    /// Best effort: a reminder that cannot be saved is dropped.
    fn save_pending(&self);

    /// Whether a reminder was saved and not cleared since.
    fn is_pending(&self) -> bool;

    /// Best effort: a reminder that cannot be cleared stays as it is.
    fn clear_pending(&self);
}

/// Remembers whether it was asked to save the reminder, for tests.
#[cfg(test)]
#[derive(Default)]
pub(crate) struct InMemoryRestartReminderRepository {
    pending: Cell<bool>,
}

#[cfg(test)]
impl RestartReminderRepository for InMemoryRestartReminderRepository {
    fn save_pending(&self) {
        self.pending.set(true);
    }

    fn is_pending(&self) -> bool {
        self.pending.get()
    }

    fn clear_pending(&self) {
        self.pending.set(false);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn should_remember_it_was_asked_to_save_the_reminder() {
        let repository = InMemoryRestartReminderRepository::default();

        repository.save_pending();

        assert!(repository.is_pending());
    }

    #[test]
    fn should_have_no_reminder_pending_until_asked() {
        let repository = InMemoryRestartReminderRepository::default();

        let pending = repository.is_pending();

        assert!(!pending);
    }

    #[test]
    fn should_have_no_reminder_pending_after_it_is_cleared() {
        let repository = InMemoryRestartReminderRepository::default();
        repository.save_pending();

        repository.clear_pending();

        assert!(!repository.is_pending());
    }
}
