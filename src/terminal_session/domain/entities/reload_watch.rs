use crate::terminal_session::domain::services::session_deadlines::FILE_POLL;
use std::time::{Instant, SystemTime};

/// When the session looks at the files it follows (D17): the settings, reloaded live, and the session context
/// `_ pre-cmd` writes. A file counts as changed when its modification time differs from the one seen last.
#[derive(Clone, Debug)]
pub(crate) struct ReloadWatch {
    poll_at: Instant,
    settings_modified: Option<SystemTime>,
    context_modified: Option<SystemTime>,
}

impl ReloadWatch {
    /// `settings_modified` = when the settings file last changed, as the session starts.
    pub(crate) fn new(now: Instant, settings_modified: Option<SystemTime>) -> Self {
        ReloadWatch {
            poll_at: now + FILE_POLL,
            settings_modified,
            context_modified: None,
        }
    }

    /// True when the files are to be looked at; the next look is then armed a poll interval later (P9-T52).
    pub(crate) fn poll_due(&mut self, now: Instant) -> bool {
        if now < self.poll_at {
            return false;
        }
        self.poll_at = now + FILE_POLL;
        true
    }

    pub(crate) fn deadline(&self) -> Instant {
        self.poll_at
    }

    /// The context file is read again only when it exists and its time differs: deleting it is no change. The new
    /// time is kept before the file is read, so an unreadable file is not read again until it changes (P9-T44).
    pub(crate) fn context_changed(&mut self, modified: Option<SystemTime>) -> bool {
        if modified.is_some() && modified != self.context_modified {
            self.context_modified = modified;
            return true;
        }
        false
    }

    /// The settings are reloaded whenever the time differs, also when the file was deleted (back to the defaults).
    pub(crate) fn settings_changed(&mut self, modified: Option<SystemTime>) -> bool {
        if modified == self.settings_modified {
            return false;
        }
        self.settings_modified = modified;
        true
    }

    /// Test arrangement: the next look at the files is due at `at`.
    #[cfg(test)]
    pub(crate) fn expire_poll_at(&mut self, at: Instant) {
        self.poll_at = at;
    }

    #[cfg(test)]
    pub(crate) fn context_modified(&self) -> Option<SystemTime> {
        self.context_modified
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, UNIX_EPOCH};

    fn at(secs: u64) -> Option<SystemTime> {
        Some(UNIX_EPOCH + Duration::from_secs(secs))
    }

    #[test]
    fn should_report_a_context_file_only_when_its_time_changes() {
        let mut watch = ReloadWatch::new(Instant::now(), None);

        let first = watch.context_changed(at(1000));
        let same = watch.context_changed(at(1000));
        let rewritten = watch.context_changed(at(2000));

        assert_eq!((first, same, rewritten), (true, false, true));
    }

    #[test]
    fn should_not_report_a_deleted_context_file_and_keep_its_last_time() {
        let mut watch = ReloadWatch::new(Instant::now(), None);
        watch.context_changed(at(1000));

        let deleted = watch.context_changed(None);

        assert_eq!((deleted, watch.context_modified()), (false, at(1000)));
    }

    #[test]
    fn should_report_deleted_settings_as_a_change() {
        let mut watch = ReloadWatch::new(Instant::now(), at(5));

        let deleted = watch.settings_changed(None);
        let again = watch.settings_changed(None);

        assert_eq!((deleted, again), (true, false));
    }

    #[test]
    fn should_poll_again_after_the_interval() {
        let now = Instant::now();
        let mut watch = ReloadWatch::new(now, None);

        let early = watch.poll_due(now + FILE_POLL - Duration::from_millis(1));
        let due = watch.poll_due(now + FILE_POLL);

        assert_eq!((early, due, watch.deadline()), (false, true, now + FILE_POLL + FILE_POLL));
    }
}
