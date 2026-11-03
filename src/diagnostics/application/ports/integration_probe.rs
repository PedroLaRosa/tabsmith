#[cfg(test)]
use crate::diagnostics::application::test_fixtures::EventLog;
use crate::shared::domain::value_objects::shell::Shell;
use crate::shell_integration::domain::entities::integration::Integration;
use crate::shell_integration::domain::shell_integration_error::ShellIntegrationError;
use crate::shell_integration::domain::value_objects::integration_paths::IntegrationPaths;
#[cfg(test)]
use std::collections::HashMap;

/// What doctor asks the shell integrations: where they live, which exist, whether each is installed properly.
pub(crate) trait IntegrationProbe {
    /// Where the integrations of this user live.
    fn locate_paths(&self) -> Result<IntegrationPaths, ShellIntegrationError>;

    /// The integrations of one shell. For pwsh with an unknown `$PROFILE` every call asks `pwsh`.
    fn list(
        &self,
        paths: &IntegrationPaths,
        shell: Shell,
    ) -> Result<Vec<Integration>, ShellIntegrationError>;

    fn check(
        &self,
        paths: &IntegrationPaths,
        integration: &Integration,
    ) -> Result<(), ShellIntegrationError>;

    /// The tabsmith binary each installed script file runs, by shell.
    fn script_binaries(&self, paths: &IntegrationPaths) -> Vec<(Shell, String)>;
}

/// A scripted answer; a closure because `ShellIntegrationError` cannot be cloned.
#[cfg(test)]
type Scripted<T> = Box<dyn Fn() -> Result<T, ShellIntegrationError>>;

/// A rule that checks any integration.
#[cfg(test)]
type CheckRule = Box<dyn Fn(&Integration) -> Result<(), ShellIntegrationError>>;

/// A scripted check result for one integration.
#[cfg(test)]
struct ScriptedCheck {
    integration: Integration,
    result: Scripted<()>,
}

/// Answers from what it was scripted with, logs each question, and panics on a question nobody scripted: that is
/// how a test asserts that doctor stays lazy.
#[cfg(test)]
#[derive(Default)]
pub(crate) struct InMemoryIntegrationProbe {
    paths: Option<Scripted<IntegrationPaths>>,
    lists: HashMap<Shell, Scripted<Vec<Integration>>>,
    checks: Vec<ScriptedCheck>,
    check_rule: Option<CheckRule>,
    script_binaries: Option<Vec<(Shell, String)>>,
    log: EventLog,
}

#[cfg(test)]
impl InMemoryIntegrationProbe {
    pub(crate) fn logging_to(mut self, log: EventLog) -> Self {
        self.log = log;
        self
    }

    pub(crate) fn locating(mut self, paths: IntegrationPaths) -> Self {
        self.paths = Some(Box::new(move || Ok(paths.clone())));
        self
    }

    pub(crate) fn failing_to_locate(mut self, message: &str) -> Self {
        let message = message.to_string();
        self.paths = Some(Box::new(move || {
            Err(ShellIntegrationError::Environment(message.clone()))
        }));
        self
    }

    pub(crate) fn listing(mut self, shell: Shell, integrations: Vec<Integration>) -> Self {
        self.lists
            .insert(shell, Box::new(move || Ok(integrations.clone())));
        self
    }

    pub(crate) fn failing_to_list(mut self, shell: Shell, message: &str) -> Self {
        let message = message.to_string();
        self.lists.insert(
            shell,
            Box::new(move || Err(ShellIntegrationError::Environment(message.clone()))),
        );
        self
    }

    pub(crate) fn checking(
        mut self,
        integration: Integration,
        result: impl Fn() -> Result<(), ShellIntegrationError> + 'static,
    ) -> Self {
        self.checks.push(ScriptedCheck {
            integration,
            result: Box::new(result),
        });
        self
    }

    /// Checks every integration that has no script of its own with this rule.
    pub(crate) fn checking_with(
        mut self,
        rule: impl Fn(&Integration) -> Result<(), ShellIntegrationError> + 'static,
    ) -> Self {
        self.check_rule = Some(Box::new(rule));
        self
    }

    pub(crate) fn with_script_binaries(mut self, binaries: Vec<(Shell, String)>) -> Self {
        self.script_binaries = Some(binaries);
        self
    }
}

#[cfg(test)]
impl IntegrationProbe for InMemoryIntegrationProbe {
    fn locate_paths(&self) -> Result<IntegrationPaths, ShellIntegrationError> {
        self.log.push("integrations.locate_paths");
        match &self.paths {
            Some(paths) => paths(),
            None => panic!("unscripted integrations.locate_paths"),
        }
    }

    fn list(
        &self,
        _paths: &IntegrationPaths,
        shell: Shell,
    ) -> Result<Vec<Integration>, ShellIntegrationError> {
        self.log
            .push(format!("integrations.list({})", shell.name()));
        match self.lists.get(&shell) {
            Some(listing) => listing(),
            None => panic!("unscripted integrations.list({})", shell.name()),
        }
    }

    fn check(
        &self,
        _paths: &IntegrationPaths,
        integration: &Integration,
    ) -> Result<(), ShellIntegrationError> {
        let question = format!("integrations.check({})", integration.describe());
        self.log.push(question.clone());
        if let Some(scripted) = self.checks.iter().find(|c| &c.integration == integration) {
            return (scripted.result)();
        }
        match &self.check_rule {
            Some(rule) => rule(integration),
            None => panic!("unscripted {question}"),
        }
    }

    fn script_binaries(&self, _paths: &IntegrationPaths) -> Vec<(Shell, String)> {
        self.log.push("integrations.script_binaries");
        match &self.script_binaries {
            Some(binaries) => binaries.clone(),
            None => panic!("unscripted integrations.script_binaries"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diagnostics::application::test_fixtures::{sample_paths, EventLog};
    use std::cell::Cell;
    use std::path::PathBuf;
    use std::rc::Rc;

    fn bashrc() -> Integration {
        Integration::dotfile(Shell::Bash, PathBuf::from("/home/u/.bashrc"))
    }

    fn zshrc() -> Integration {
        Integration::dotfile(Shell::Zsh, PathBuf::from("/home/u/.zshrc"))
    }

    #[test]
    fn should_locate_the_scripted_paths() {
        let probe = InMemoryIntegrationProbe::default().locating(sample_paths());

        let located = probe.locate_paths();

        assert_eq!(located.unwrap().data_root(), sample_paths().data_root());
    }

    #[test]
    fn should_fail_to_locate_with_the_scripted_message() {
        let probe = InMemoryIntegrationProbe::default().failing_to_locate("no home");

        let located = probe.locate_paths();

        assert_eq!(located.unwrap_err().to_string(), "no home");
    }

    #[test]
    #[should_panic(expected = "unscripted integrations.locate_paths")]
    fn should_panic_when_asked_for_paths_it_was_not_scripted_with() {
        let probe = InMemoryIntegrationProbe::default();

        let _ = probe.locate_paths();
    }

    #[test]
    fn should_list_the_integrations_scripted_for_the_shell() {
        let probe = InMemoryIntegrationProbe::default().listing(Shell::Bash, vec![bashrc()]);

        let listed = probe.list(&sample_paths(), Shell::Bash);

        assert_eq!(listed.unwrap(), vec![bashrc()]);
    }

    #[test]
    fn should_fail_to_list_with_the_scripted_message() {
        let probe =
            InMemoryIntegrationProbe::default().failing_to_list(Shell::Pwsh, "pwsh is not here");

        let listed = probe.list(&sample_paths(), Shell::Pwsh);

        assert_eq!(listed.unwrap_err().to_string(), "pwsh is not here");
    }

    #[test]
    #[should_panic(expected = "unscripted integrations.list(zsh)")]
    fn should_panic_when_asked_to_list_a_shell_it_was_not_scripted_with() {
        let probe = InMemoryIntegrationProbe::default().listing(Shell::Bash, vec![]);

        let _ = probe.list(&sample_paths(), Shell::Zsh);
    }

    #[test]
    fn should_check_the_integration_with_its_own_script() {
        let probe = InMemoryIntegrationProbe::default()
            .checking(bashrc(), || Ok(()))
            .checking(zshrc(), || {
                Err(ShellIntegrationError::NotInstalled("zsh".into()))
            });

        let checked = (
            probe.check(&sample_paths(), &bashrc()).is_ok(),
            probe
                .check(&sample_paths(), &zshrc())
                .unwrap_err()
                .to_string(),
        );

        assert_eq!(
            checked,
            (true, "Integration not installed: zsh".to_string())
        );
    }

    #[test]
    fn should_check_with_a_rule_a_test_can_flip() {
        let installed = Rc::new(Cell::new(false));
        let seen = installed.clone();
        let probe = InMemoryIntegrationProbe::default().checking_with(move |_| match seen.get() {
            true => Ok(()),
            false => Err(ShellIntegrationError::NotInstalled("bash".into())),
        });
        let before = probe.check(&sample_paths(), &bashrc()).is_ok();

        installed.set(true);

        assert_eq!(
            (before, probe.check(&sample_paths(), &bashrc()).is_ok()),
            (false, true)
        );
    }

    #[test]
    #[should_panic(expected = "unscripted integrations.check(bash pre post into .bashrc)")]
    fn should_panic_when_asked_to_check_an_integration_it_was_not_scripted_with() {
        let probe = InMemoryIntegrationProbe::default().checking(zshrc(), || Ok(()));

        let _ = probe.check(&sample_paths(), &bashrc());
    }

    #[test]
    fn should_give_the_scripted_script_binaries() {
        let probe = InMemoryIntegrationProbe::default()
            .with_script_binaries(vec![(Shell::Bash, "/old/tabsmith".to_string())]);

        let binaries = probe.script_binaries(&sample_paths());

        assert_eq!(binaries, vec![(Shell::Bash, "/old/tabsmith".to_string())]);
    }

    #[test]
    #[should_panic(expected = "unscripted integrations.script_binaries")]
    fn should_panic_when_asked_for_script_binaries_it_was_not_scripted_with() {
        let probe = InMemoryIntegrationProbe::default();

        let _ = probe.script_binaries(&sample_paths());
    }

    #[test]
    fn should_log_each_integration_question_in_order() {
        let log = EventLog::default();
        let probe = InMemoryIntegrationProbe::default()
            .logging_to(log.clone())
            .locating(sample_paths())
            .listing(Shell::Pwsh, vec![])
            .checking(bashrc(), || Ok(()))
            .with_script_binaries(vec![]);

        let paths = probe.locate_paths().unwrap();
        let _ = probe.list(&paths, Shell::Pwsh);
        let _ = probe.check(&paths, &bashrc());
        let _ = probe.script_binaries(&paths);

        assert_eq!(
            log.entries(),
            [
                "integrations.locate_paths",
                "integrations.list(pwsh)",
                "integrations.check(bash pre post into .bashrc)",
                "integrations.script_binaries",
            ]
        );
    }
}
