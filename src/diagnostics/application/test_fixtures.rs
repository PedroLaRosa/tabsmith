#[cfg(test)]
use crate::shell_integration::domain::repositories::restart_reminder_repository::RestartReminderRepository;
#[cfg(test)]
use crate::shell_integration::domain::value_objects::integration_paths::IntegrationPaths;
#[cfg(test)]
use std::cell::RefCell;
#[cfg(test)]
use std::path::PathBuf;
#[cfg(test)]
use std::rc::Rc;

/// What the doubles of one test did, in order, so the test can assert the ORDER of probe calls and reporter events.
/// Clones share the same entries.
#[cfg(test)]
#[derive(Clone, Default)]
pub(crate) struct EventLog {
    entries: Rc<RefCell<Vec<String>>>,
}

#[cfg(test)]
impl EventLog {
    pub(crate) fn push(&self, entry: impl Into<String>) {
        self.entries.borrow_mut().push(entry.into());
    }

    pub(crate) fn entries(&self) -> Vec<String> {
        self.entries.borrow().clone()
    }
}

/// The paths of a user `u` whose pwsh `$PROFILE` is not known yet.
#[cfg(test)]
pub(crate) fn sample_paths() -> IntegrationPaths {
    IntegrationPaths::new(
        PathBuf::from("/home/u"),
        PathBuf::from("/home/u/.tabsmith"),
        "/usr/local/bin/tabsmith".to_string(),
        None,
        PathBuf::from("/home/u/.config/fish"),
        None,
    )
}

/// A restart reminder repository that writes what it is asked into the log and then asks the one it wraps.
#[cfg(test)]
pub(crate) struct LoggingRestartReminder {
    inner: Rc<dyn RestartReminderRepository>,
    log: EventLog,
}

#[cfg(test)]
impl LoggingRestartReminder {
    pub(crate) fn new(inner: Rc<dyn RestartReminderRepository>, log: EventLog) -> Self {
        LoggingRestartReminder { inner, log }
    }
}

#[cfg(test)]
impl RestartReminderRepository for LoggingRestartReminder {
    fn save_pending(&self) {
        self.log.push("reminder.save_pending");
        self.inner.save_pending();
    }

    fn is_pending(&self) -> bool {
        self.log.push("reminder.is_pending");
        self.inner.is_pending()
    }

    fn clear_pending(&self) {
        self.log.push("reminder.clear_pending");
        self.inner.clear_pending();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shell_integration::domain::repositories::restart_reminder_repository::InMemoryRestartReminderRepository;

    #[test]
    fn should_keep_the_entries_in_the_order_they_were_pushed() {
        let log = EventLog::default();

        log.push("first");
        log.push("second".to_string());

        assert_eq!(log.entries(), vec!["first", "second"]);
    }

    #[test]
    fn should_share_its_entries_with_its_clones() {
        let log = EventLog::default();
        let clone = log.clone();

        clone.push("from the clone");

        assert_eq!(log.entries(), vec!["from the clone"]);
    }

    #[test]
    fn should_describe_a_user_whose_pwsh_profile_is_not_known() {
        let paths = sample_paths();

        assert_eq!(
            (
                paths.data_root().to_path_buf(),
                paths.pwsh_profile().is_none()
            ),
            (PathBuf::from("/home/u/.tabsmith"), true)
        );
    }

    #[test]
    fn should_log_and_delegate_the_question_whether_a_reminder_is_pending() {
        let log = EventLog::default();
        let inner = Rc::new(InMemoryRestartReminderRepository::default());
        inner.save_pending();
        let reminder = LoggingRestartReminder::new(inner, log.clone());

        let pending = reminder.is_pending();

        assert_eq!(
            (pending, log.entries()),
            (true, vec!["reminder.is_pending".to_string()])
        );
    }

    #[test]
    fn should_log_and_delegate_clearing_the_reminder() {
        let log = EventLog::default();
        let inner = Rc::new(InMemoryRestartReminderRepository::default());
        inner.save_pending();
        let reminder = LoggingRestartReminder::new(inner.clone(), log.clone());

        reminder.clear_pending();

        assert_eq!(
            (inner.is_pending(), log.entries()),
            (false, vec!["reminder.clear_pending".to_string()])
        );
    }

    #[test]
    fn should_log_and_delegate_saving_the_reminder() {
        let log = EventLog::default();
        let inner = Rc::new(InMemoryRestartReminderRepository::default());
        let reminder = LoggingRestartReminder::new(inner.clone(), log.clone());

        reminder.save_pending();

        assert_eq!(
            (inner.is_pending(), log.entries()),
            (true, vec!["reminder.save_pending".to_string()])
        );
    }
}
