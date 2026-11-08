#[cfg(test)]
use crate::modules::diagnostics::application::test_fixtures::EventLog;
use crate::modules::diagnostics::domain::value_objects::check_fix::CheckFix;
use crate::modules::shell_integration::domain::value_objects::integration_paths::IntegrationPaths;
#[cfg(test)]
use std::cell::RefCell;

/// Repairs what a failed check found.
pub(crate) trait CheckFixer {
    /// The failure is the text printed after `Failed to fix: `, already rendered with its causes.
    fn fix(&self, fix: &CheckFix, paths: &IntegrationPaths) -> Result<(), String>;
}

/// Records every fix it is asked for and answers with the result it was given; after a successful fix it runs an
/// effect, so a test can flip the fact the next probe reads.
#[cfg(test)]
pub(crate) struct RecordingCheckFixer {
    result: Result<(), String>,
    effect: Option<Box<dyn Fn()>>,
    fixes: RefCell<Vec<CheckFix>>,
    paths: RefCell<Vec<IntegrationPaths>>,
    log: EventLog,
}

#[cfg(test)]
impl RecordingCheckFixer {
    pub(crate) fn succeeding() -> Self {
        RecordingCheckFixer {
            result: Ok(()),
            effect: None,
            fixes: RefCell::new(Vec::new()),
            paths: RefCell::new(Vec::new()),
            log: EventLog::default(),
        }
    }

    pub(crate) fn failing(text: &str) -> Self {
        RecordingCheckFixer {
            result: Err(text.to_string()),
            ..Self::succeeding()
        }
    }

    pub(crate) fn with_effect(mut self, effect: impl Fn() + 'static) -> Self {
        self.effect = Some(Box::new(effect));
        self
    }

    pub(crate) fn logging_to(mut self, log: EventLog) -> Self {
        self.log = log;
        self
    }

    pub(crate) fn fixes(&self) -> Vec<CheckFix> {
        self.fixes.borrow().clone()
    }

    pub(crate) fn paths(&self) -> Vec<IntegrationPaths> {
        self.paths.borrow().clone()
    }
}

#[cfg(test)]
impl CheckFixer for RecordingCheckFixer {
    fn fix(&self, fix: &CheckFix, paths: &IntegrationPaths) -> Result<(), String> {
        self.log.push(format!("fixer.fix({fix:?})"));
        self.fixes.borrow_mut().push(fix.clone());
        self.paths.borrow_mut().push(paths.clone());
        if self.result.is_ok() {
            if let Some(effect) = &self.effect {
                effect();
            }
        }
        self.result.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::diagnostics::application::test_fixtures::{sample_paths, EventLog};
    use std::cell::Cell;
    use std::path::PathBuf;
    use std::rc::Rc;

    #[test]
    fn should_record_each_fix_in_the_order_it_was_asked() {
        let fixer = RecordingCheckFixer::succeeding();

        let _ = fixer.fix(&CheckFix::InstallSpecBundle, &sample_paths());
        let _ = fixer.fix(
            &CheckFix::ResetSettings(PathBuf::from("/s.json")),
            &sample_paths(),
        );

        assert_eq!(
            fixer.fixes(),
            vec![
                CheckFix::InstallSpecBundle,
                CheckFix::ResetSettings(PathBuf::from("/s.json"))
            ]
        );
    }

    #[test]
    fn should_record_the_paths_each_fix_was_asked_with() {
        let fixer = RecordingCheckFixer::succeeding();

        let _ = fixer.fix(&CheckFix::InstallSpecBundle, &sample_paths());

        assert_eq!(fixer.paths().len(), 1);
    }

    #[test]
    fn should_succeed_when_it_is_scripted_to_succeed() {
        let fixer = RecordingCheckFixer::succeeding();

        let result = fixer.fix(&CheckFix::InstallSpecBundle, &sample_paths());

        assert_eq!(result, Ok(()));
    }

    #[test]
    fn should_fail_with_the_text_it_was_given() {
        let fixer = RecordingCheckFixer::failing("disk full");

        let result = fixer.fix(&CheckFix::InstallSpecBundle, &sample_paths());

        assert_eq!(result, Err("disk full".to_string()));
    }

    #[test]
    fn should_run_its_effect_after_a_successful_fix() {
        let repaired = Rc::new(Cell::new(false));
        let flag = repaired.clone();
        let fixer = RecordingCheckFixer::succeeding().with_effect(move || flag.set(true));

        let _ = fixer.fix(&CheckFix::InstallSpecBundle, &sample_paths());

        assert!(repaired.get());
    }

    #[test]
    fn should_not_run_its_effect_after_a_failed_fix() {
        let repaired = Rc::new(Cell::new(false));
        let flag = repaired.clone();
        let fixer = RecordingCheckFixer::failing("no").with_effect(move || flag.set(true));

        let _ = fixer.fix(&CheckFix::InstallSpecBundle, &sample_paths());

        assert!(!repaired.get());
    }

    #[test]
    fn should_log_each_fix() {
        let log = EventLog::default();
        let fixer = RecordingCheckFixer::succeeding().logging_to(log.clone());

        let _ = fixer.fix(&CheckFix::InstallSpecBundle, &sample_paths());

        assert_eq!(
            log.entries(),
            vec!["fixer.fix(InstallSpecBundle)".to_string()]
        );
    }
}
