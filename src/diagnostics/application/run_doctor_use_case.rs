use crate::diagnostics::application::check_evaluator::CheckEvaluator;
use crate::diagnostics::application::check_group_builder::CheckGroupBuilder;
use crate::diagnostics::application::ports::check_fixer::CheckFixer;
use crate::diagnostics::application::ports::doctor_reporter::DoctorReporter;
use crate::diagnostics::application::ports::integration_probe::IntegrationProbe;
use crate::diagnostics::application::ports::system_facts_probe::SystemFactsProbe;
use crate::diagnostics::domain::entities::doctor_run::DoctorRun;
use crate::diagnostics::domain::value_objects::check::Check;
use crate::diagnostics::domain::value_objects::check_fix::CheckFix;
use crate::diagnostics::domain::value_objects::check_group::CheckGroup;
use crate::diagnostics::domain::value_objects::check_outcome::CheckOutcome;
use crate::diagnostics::domain::value_objects::check_severity::CheckSeverity;
use crate::shell_integration::domain::repositories::restart_reminder_repository::RestartReminderRepository;
use crate::shell_integration::domain::value_objects::integration_paths::IntegrationPaths;
use std::rc::Rc;

/// Runs the doctor checks group by group, repairing what it can.
pub(crate) struct RunDoctorUseCase {
    builder: CheckGroupBuilder,
    evaluator: CheckEvaluator,
    fixer: Rc<dyn CheckFixer>,
    reporter: Rc<dyn DoctorReporter>,
    system: Rc<dyn SystemFactsProbe>,
    integrations: Rc<dyn IntegrationProbe>,
    restart_reminder: Rc<dyn RestartReminderRepository>,
}

impl RunDoctorUseCase {
    pub(crate) fn new(
        builder: CheckGroupBuilder,
        evaluator: CheckEvaluator,
        fixer: Rc<dyn CheckFixer>,
        reporter: Rc<dyn DoctorReporter>,
        system: Rc<dyn SystemFactsProbe>,
        integrations: Rc<dyn IntegrationProbe>,
        restart_reminder: Rc<dyn RestartReminderRepository>,
    ) -> Self {
        RunDoctorUseCase { builder, evaluator, fixer, reporter, system, integrations, restart_reminder }
    }

    /// False when an error that could not be repaired ends the run (without `--all`); the remaining groups are not run.
    fn run_group(&self, run: &mut DoctorRun, group: &CheckGroup, paths: &IntegrationPaths) -> bool {
        if run.all() {
            self.reporter.group_header(group.header());
        }
        let checks = group.checks().iter().filter(|check| check.severity() != CheckSeverity::Skip);
        for check in checks {
            if !self.run_check(run, check, paths) {
                return false;
            }
        }
        if run.all() {
            self.reporter.blank_line();
        }
        true
    }

    /// False when the check failed for good.
    fn run_check(&self, run: &mut DoctorRun, check: &Check, paths: &IntegrationPaths) -> bool {
        let outcome = self.settled(run, check, paths);
        if run.all() || !outcome.is_ok() {
            self.reporter.stop_progress();
            self.reporter.outcome(check.name(), &outcome);
        }
        if run.all() {
            run.count(&outcome);
            return true;
        }
        let CheckOutcome::Err { fix, .. } = outcome else {
            run.count(&outcome);
            return true;
        };
        if let Some(repaired) = fix.and_then(|fix| self.fix_and_recheck(run, check, &fix, paths)) {
            run.count(&repaired);
            return true;
        }
        self.reporter.blank_line();
        run.abort();
        false
    }

    /// The outcome of the check once the new probe answers have been through the policy of the run.
    fn settled(&self, run: &DoctorRun, check: &Check, paths: &IntegrationPaths) -> CheckOutcome {
        run.effective(check.severity(), self.evaluator.evaluate(check, paths))
    }

    /// The outcome of the check evaluated again after the fix, unless the fix failed or the check still errs.
    fn fix_and_recheck(
        &self,
        run: &DoctorRun,
        check: &Check,
        fix: &CheckFix,
        paths: &IntegrationPaths,
    ) -> Option<CheckOutcome> {
        self.reporter.fixing();
        if let Err(reason) = self.fixer.fix(fix, paths) {
            self.reporter.fix_failed(&reason);
            return None;
        }
        self.reporter.rechecking();
        self.reporter.blank_line();
        let again = self.settled(run, check, paths);
        self.reporter.outcome(check.name(), &again);
        (!again.is_err()).then_some(again)
    }
}

#[cfg(test)]
mod tests;
