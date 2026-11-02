use crate::diagnostics::domain::value_objects::check_outcome::CheckOutcome;
use crate::diagnostics::domain::value_objects::check_severity::CheckSeverity;

/// The policy (`--all`, `--strict`) and the running counts of one doctor run.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct DoctorRun {
    all: bool,
    strict: bool,
    errors: usize,
    warnings: usize,
}

impl DoctorRun {
    pub(crate) fn new(all: bool, strict: bool) -> Self {
        Self {
            all,
            strict,
            errors: 0,
            warnings: 0,
        }
    }

    pub(crate) fn all(&self) -> bool {
        self.all
    }

    pub(crate) fn strict(&self) -> bool {
        self.strict
    }

    pub(crate) fn errors(&self) -> usize {
        self.errors
    }

    pub(crate) fn warnings(&self) -> usize {
        self.warnings
    }

    /// A Soft error is only a warning unless `--strict`; the warning drops the error's info and fix.
    pub(crate) fn effective(
        &self,
        severity: CheckSeverity,
        outcome: CheckOutcome,
    ) -> CheckOutcome {
        match outcome {
            CheckOutcome::Err { reason, .. }
                if severity == CheckSeverity::Soft && !self.strict =>
            {
                CheckOutcome::Warn(reason)
            }
            outcome @ (CheckOutcome::Ok | CheckOutcome::Warn(_) | CheckOutcome::Err { .. }) => {
                outcome
            }
        }
    }

    pub(crate) fn count(&mut self, outcome: &CheckOutcome) {
        match outcome {
            CheckOutcome::Ok => {}
            CheckOutcome::Warn(_) => self.warnings += 1,
            CheckOutcome::Err { .. } => self.errors += 1,
        }
    }

    /// An error that could not be fixed ends the run; it is counted here, not through `count`.
    pub(crate) fn abort(&mut self) {
        self.errors += 1;
    }

    /// The (errors, warnings) so far.
    pub(crate) fn counts(&self) -> (usize, usize) {
        (self.errors, self.warnings)
    }

    pub(crate) fn restore(&mut self, counts: (usize, usize)) {
        (self.errors, self.warnings) = counts;
    }

    pub(crate) fn failed(&self) -> bool {
        self.errors > 0 || (self.strict && self.warnings > 0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diagnostics::domain::value_objects::check_fix::CheckFix;
    use crate::diagnostics::domain::value_objects::info_line::InfoLine;

    fn error_with_info_and_fix(reason: &str) -> CheckOutcome {
        CheckOutcome::Err {
            reason: reason.into(),
            info: vec![InfoLine::Text("see the docs".into())],
            fix: Some(CheckFix::InstallSpecBundle),
        }
    }

    #[test]
    fn should_keep_the_policy_it_was_created_with() {
        let run = DoctorRun::new(true, false);

        assert_eq!((run.all(), run.strict()), (true, false));
    }

    #[test]
    fn should_turn_a_soft_error_into_a_warning_with_its_reason_when_not_strict() {
        let run = DoctorRun::new(false, false);

        let outcome = run.effective(CheckSeverity::Soft, CheckOutcome::err("meh"));

        assert_eq!(outcome, CheckOutcome::Warn("meh".into()));
    }

    #[test]
    fn should_drop_the_info_and_the_fix_when_a_soft_error_becomes_a_warning() {
        let run = DoctorRun::new(false, false);

        let outcome = run.effective(CheckSeverity::Soft, error_with_info_and_fix("meh"));

        assert_eq!(outcome, CheckOutcome::Warn("meh".into()));
    }

    #[test]
    fn should_keep_a_soft_error_with_its_info_and_fix_when_strict() {
        let run = DoctorRun::new(false, true);

        let outcome = run.effective(CheckSeverity::Soft, error_with_info_and_fix("meh"));

        assert_eq!(outcome, error_with_info_and_fix("meh"));
    }

    #[test]
    fn should_never_rewrite_a_normal_error() {
        let run = DoctorRun::new(false, false);

        let outcome = run.effective(CheckSeverity::Normal, error_with_info_and_fix("nope"));

        assert_eq!(outcome, error_with_info_and_fix("nope"));
    }

    #[test]
    fn should_let_a_warning_pass_when_strict() {
        let run = DoctorRun::new(false, true);

        let outcome = run.effective(CheckSeverity::Soft, CheckOutcome::Warn("careful".into()));

        assert_eq!(outcome, CheckOutcome::Warn("careful".into()));
    }

    #[test]
    fn should_let_a_success_pass() {
        let run = DoctorRun::new(false, false);

        let outcome = run.effective(CheckSeverity::Soft, CheckOutcome::Ok);

        assert_eq!(outcome, CheckOutcome::Ok);
    }

    #[test]
    fn should_count_nothing_for_a_success() {
        let mut run = DoctorRun::new(false, false);

        run.count(&CheckOutcome::Ok);

        assert_eq!(run.counts(), (0, 0));
    }

    #[test]
    fn should_count_a_warning() {
        let mut run = DoctorRun::new(false, false);

        run.count(&CheckOutcome::Warn("careful".into()));

        assert_eq!(run.counts(), (0, 1));
    }

    #[test]
    fn should_count_an_error() {
        let mut run = DoctorRun::new(false, false);

        run.count(&CheckOutcome::err("nope"));

        assert_eq!(run.counts(), (1, 0));
    }

    #[test]
    fn should_count_an_aborted_run_as_an_error_without_counting_an_outcome() {
        let mut run = DoctorRun::new(false, false);

        run.abort();

        assert_eq!((run.errors(), run.warnings()), (1, 0));
    }

    #[test]
    fn should_restore_the_counts_it_had_before_a_group() {
        let mut run = DoctorRun::new(false, false);
        run.count(&CheckOutcome::err("nope"));
        let before_group = run.counts();
        run.count(&CheckOutcome::err("again"));
        run.count(&CheckOutcome::Warn("careful".into()));

        run.restore(before_group);

        assert_eq!(run.counts(), (1, 0));
    }

    #[test]
    fn should_fail_when_there_is_an_error() {
        let mut run = DoctorRun::new(false, false);
        run.count(&CheckOutcome::err("nope"));

        assert!(run.failed());
    }

    #[test]
    fn should_fail_on_a_warning_when_strict() {
        let mut run = DoctorRun::new(false, true);
        run.count(&CheckOutcome::Warn("careful".into()));

        assert!(run.failed());
    }

    #[test]
    fn should_not_fail_on_a_warning_when_not_strict() {
        let mut run = DoctorRun::new(false, false);
        run.count(&CheckOutcome::Warn("careful".into()));

        assert!(!run.failed());
    }

    #[test]
    fn should_not_fail_when_strict_and_nothing_was_counted() {
        let run = DoctorRun::new(false, true);

        assert!(!run.failed());
    }

    #[test]
    fn should_replay_a_soft_error_a_fixed_error_and_a_fatal_error_as_one_error_and_one_warning() {
        let mut run = DoctorRun::new(false, false);

        let soft = run.effective(CheckSeverity::Soft, CheckOutcome::err("meh"));
        run.count(&soft);
        let fixable = run.effective(CheckSeverity::Normal, error_with_info_and_fix("broken"));
        assert!(fixable.is_err());
        let after_the_fix = run.effective(CheckSeverity::Normal, CheckOutcome::Ok);
        run.count(&after_the_fix);
        let fatal = run.effective(CheckSeverity::Normal, CheckOutcome::err("nope"));
        assert!(fatal.is_err());
        run.abort();

        assert_eq!(run.counts(), (1, 1));
    }

    #[test]
    fn should_replay_a_strict_soft_error_that_aborts_as_one_error() {
        let mut run = DoctorRun::new(false, true);

        let soft = run.effective(CheckSeverity::Soft, CheckOutcome::err("meh"));
        assert!(soft.is_err());
        run.abort();

        assert_eq!(run.counts(), (1, 0));
    }

    #[test]
    fn should_replay_all_mode_counting_every_outcome_as_two_errors_and_one_warning() {
        let mut run = DoctorRun::new(true, false);

        for (severity, outcome) in [
            (CheckSeverity::Soft, CheckOutcome::err("meh")),
            (CheckSeverity::Normal, error_with_info_and_fix("broken")),
            (CheckSeverity::Normal, CheckOutcome::err("nope")),
        ] {
            let effective = run.effective(severity, outcome);
            run.count(&effective);
        }

        assert_eq!(run.counts(), (2, 1));
    }
}
