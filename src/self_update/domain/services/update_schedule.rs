/// How long a background update check waits after the previous one.
// ponytail: a fixed day, as Claude Code's startup check; make it a setting if anyone needs another interval.
const CHECK_INTERVAL_SECS: u64 = 24 * 60 * 60;

/// When a background update check is due.
pub(crate) struct UpdateSchedule;

impl UpdateSchedule {
    pub(crate) fn is_due(last_check_secs: Option<u64>, now_secs: u64) -> bool {
        match last_check_secs {
            None => true,
            Some(last_check_secs) => now_secs.saturating_sub(last_check_secs) >= CHECK_INTERVAL_SECS,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const NOW_SECS: u64 = 1_700_000_000;

    #[test]
    fn should_be_due_when_tabsmith_never_checked() {
        let last_check_secs = None;

        let due = UpdateSchedule::is_due(last_check_secs, NOW_SECS);

        assert!(due);
    }

    #[test]
    fn should_not_be_due_less_than_a_day_after_the_last_check() {
        let last_check_secs = Some(NOW_SECS - 23 * 60 * 60);

        let due = UpdateSchedule::is_due(last_check_secs, NOW_SECS);

        assert!(!due);
    }

    #[test]
    fn should_be_due_a_whole_day_after_the_last_check() {
        let last_check_secs = Some(NOW_SECS - 24 * 60 * 60);

        let due = UpdateSchedule::is_due(last_check_secs, NOW_SECS);

        assert!(due);
    }
}
