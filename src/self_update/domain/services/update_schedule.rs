/// When a background update check is due.
pub(crate) struct UpdateSchedule;

impl UpdateSchedule {
    pub(crate) fn is_due(_last_check_secs: Option<u64>, _now_secs: u64) -> bool {
        true
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
}
