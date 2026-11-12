/// How a doctor run ended; `Failed` makes the command exit with a failure.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum DoctorVerdict {
    Passed,
    Failed,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn should_tell_a_passed_run_from_a_failed_one() {
        let verdicts = (DoctorVerdict::Passed, DoctorVerdict::Failed);

        assert_ne!(verdicts.0, verdicts.1);
    }

    #[test]
    fn should_stay_usable_after_a_verdict_is_copied() {
        let verdict = DoctorVerdict::Failed;

        let copy = verdict;

        assert_eq!(format!("{copy:?} {verdict:?}"), "Failed Failed");
    }
}
