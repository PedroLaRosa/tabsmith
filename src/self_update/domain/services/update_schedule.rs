/// When a background update check is due.
pub(crate) struct UpdateSchedule;

impl UpdateSchedule {
    pub(crate) fn is_due(last_check_secs: Option<u64>, _now_secs: u64) -> bool {
        last_check_secs.is_none()
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
}
