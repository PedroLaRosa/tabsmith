use crate::modules::diagnostics::application::check_evaluator::CheckEvaluator;
use crate::modules::diagnostics::application::check_group_builder::CheckGroupBuilder;
use crate::modules::diagnostics::application::doctor_request::DoctorRequest;
use crate::modules::diagnostics::application::doctor_verdict::DoctorVerdict;
use crate::modules::diagnostics::application::ports::check_fixer::CheckFixer;
use crate::modules::diagnostics::application::ports::doctor_reporter::DoctorReporter;
use crate::modules::diagnostics::application::ports::integration_probe::IntegrationProbe;
use crate::modules::diagnostics::application::ports::system_facts_probe::SystemFactsProbe;
use crate::modules::diagnostics::domain::diagnostics_error::DiagnosticsError;
use crate::modules::diagnostics::domain::entities::doctor_run::DoctorRun;
use crate::modules::diagnostics::domain::value_objects::check::Check;
use crate::modules::diagnostics::domain::value_objects::check_fix::CheckFix;
use crate::modules::diagnostics::domain::value_objects::check_group::CheckGroup;
use crate::modules::diagnostics::domain::value_objects::check_outcome::CheckOutcome;
use crate::modules::diagnostics::domain::value_objects::check_severity::CheckSeverity;
use crate::modules::shared::domain::value_objects::shell::Shell;
use crate::modules::shell_integration::domain::repositories::restart_reminder_repository::RestartReminderRepository;
use crate::modules::shell_integration::domain::value_objects::integration_paths::IntegrationPaths;
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

    /// Runs the checks group by group, then tells the user how it went and whether a terminal restart is due.
    pub(crate) fn execute(&self, request: DoctorRequest) -> Result<DoctorVerdict, DiagnosticsError> {
        let paths = self.integrations.locate_paths().map_err(DiagnosticsError::IntegrationPaths)?;
        let parent = self.system.parent_executable();
        let current = parent.as_deref().and_then(Shell::from_name);
        let mut run = DoctorRun::new(request.all(), request.strict());
        if !run.all() {
            self.reporter.start_progress();
        }
        let ok = self.run_group(&mut run, &self.builder.dotfiles(&paths, current), &paths)
            && self.run_group(&mut run, &self.builder.setup(&paths), &paths)
            && self.run_group(&mut run, &self.builder.environment(parent.as_deref()), &paths);
        if ok {
            self.run_system_group(&mut run, current, parent.as_deref(), &paths);
        }
        self.reporter.stop_progress();
        self.summarize(&run);
        self.remind_to_restart();
        Ok(if run.failed() { DoctorVerdict::Failed } else { DoctorVerdict::Passed })
    }

    /// What the system group finds is shown but neither ends the run nor counts.
    fn run_system_group(&self, run: &mut DoctorRun, current: Option<Shell>, parent: Option<&str>, paths: &IntegrationPaths) {
        let counts = run.counts();
        self.run_group(run, &self.builder.system(current, parent), paths);
        run.restore(counts);
    }

    fn summarize(&self, run: &DoctorRun) {
        if run.failed() {
            self.reporter.blank_line();
            self.reporter.doctor_found_errors();
        } else {
            if !run.all() {
                self.reporter.everything_looks_good();
            }
            self.reporter.blank_line();
            self.reporter.support_hint();
        }
        self.reporter.blank_line();
    }

    fn remind_to_restart(&self) {
        if self.restart_reminder.is_pending() {
            self.reporter.restart_notice();
            self.restart_reminder.clear_pending();
        }
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
mod execute_tests;
#[cfg(test)]
mod tests;
