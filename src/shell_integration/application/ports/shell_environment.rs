use crate::shell_integration::domain::shell_integration_error::ShellIntegrationError;
use crate::shell_integration::domain::value_objects::launch_facts::LaunchFacts;
#[cfg(test)]
use std::cell::RefCell;
#[cfg(test)]
use std::collections::HashMap;
use std::path::PathBuf;

/// The facts of this process and its shell that the integration commands ask for, each found out when asked.
pub(crate) trait ShellEnvironment {
    /// Absolute path of the running tabsmith binary.
    fn current_executable(&self) -> Result<String, ShellIntegrationError>;
    /// Executable path of the parent process, when it can be found.
    fn parent_shell(&self) -> Option<String>;
    /// What the launch rules (SPEC §5.2) look at.
    fn launch_facts(&self) -> LaunchFacts;
    /// pwsh's `$PROFILE` (CurrentUserCurrentHost).
    fn pwsh_profile(&self) -> Result<PathBuf, ShellIntegrationError>;
}

/// Answers with the facts it was given and records which were asked, in order, for tests. A failing answer is an
/// `Environment` error with the given message, the only failure the shell environment reports.
#[cfg(test)]
pub(crate) struct RecordingShellEnvironment {
    current_executable: Result<String, String>,
    parent_shell: Option<String>,
    launch_env: HashMap<String, String>,
    stdin_is_terminal: bool,
    pwsh_profile: Result<PathBuf, String>,
    asked: RefCell<Vec<&'static str>>,
}

#[cfg(test)]
impl Default for RecordingShellEnvironment {
    fn default() -> Self {
        RecordingShellEnvironment {
            current_executable: Ok("/usr/local/bin/tabsmith".into()),
            parent_shell: None,
            launch_env: HashMap::new(),
            stdin_is_terminal: false,
            pwsh_profile: Ok(PathBuf::from("/home/u/.config/powershell/Microsoft.PowerShell_profile.ps1")),
            asked: RefCell::default(),
        }
    }
}

#[cfg(test)]
impl RecordingShellEnvironment {
    pub(crate) fn with_current_executable(self, current_executable: Result<String, String>) -> Self {
        RecordingShellEnvironment {
            current_executable,
            ..self
        }
    }

    pub(crate) fn with_parent_shell(self, parent_shell: Option<String>) -> Self {
        RecordingShellEnvironment {
            parent_shell,
            ..self
        }
    }

    pub(crate) fn with_launch_facts(self, env: HashMap<String, String>, stdin_is_terminal: bool) -> Self {
        RecordingShellEnvironment {
            launch_env: env,
            stdin_is_terminal,
            ..self
        }
    }

    pub(crate) fn with_pwsh_profile(self, pwsh_profile: Result<PathBuf, String>) -> Self {
        RecordingShellEnvironment {
            pwsh_profile,
            ..self
        }
    }

    pub(crate) fn asked(&self) -> Vec<&'static str> {
        self.asked.borrow().clone()
    }

    fn record(&self, fact: &'static str) {
        self.asked.borrow_mut().push(fact);
    }
}

#[cfg(test)]
impl ShellEnvironment for RecordingShellEnvironment {
    fn current_executable(&self) -> Result<String, ShellIntegrationError> {
        self.record("current_executable");
        self.current_executable
            .clone()
            .map_err(ShellIntegrationError::Environment)
    }

    fn parent_shell(&self) -> Option<String> {
        self.record("parent_shell");
        self.parent_shell.clone()
    }

    fn launch_facts(&self) -> LaunchFacts {
        self.record("launch_facts");
        LaunchFacts::new(self.launch_env.clone(), self.stdin_is_terminal)
    }

    fn pwsh_profile(&self) -> Result<PathBuf, ShellIntegrationError> {
        self.record("pwsh_profile");
        self.pwsh_profile
            .clone()
            .map_err(ShellIntegrationError::Environment)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn should_record_which_facts_were_asked_in_order() {
        let environment = RecordingShellEnvironment::default();

        let _ = environment.parent_shell();
        let _ = environment.current_executable();
        let _ = environment.pwsh_profile();
        let _ = environment.launch_facts();

        assert_eq!(
            environment.asked(),
            ["parent_shell", "current_executable", "pwsh_profile", "launch_facts"]
        );
    }

    #[test]
    fn should_answer_with_the_facts_it_was_given() {
        let environment = RecordingShellEnvironment::default()
            .with_current_executable(Ok("/usr/local/bin/tabsmith".into()))
            .with_parent_shell(Some("/bin/zsh".into()))
            .with_launch_facts(HashMap::from([("CI".into(), "true".into())]), true)
            .with_pwsh_profile(Ok(PathBuf::from("/home/u/profile.ps1")));

        let launch_facts = environment.launch_facts();
        let answers = (
            environment.current_executable().unwrap(),
            environment.parent_shell(),
            (launch_facts.is("CI", "true"), launch_facts.stdin_is_terminal()),
            environment.pwsh_profile().unwrap(),
        );

        assert_eq!(
            answers,
            (
                "/usr/local/bin/tabsmith".to_string(),
                Some("/bin/zsh".to_string()),
                (true, true),
                PathBuf::from("/home/u/profile.ps1"),
            )
        );
    }
}
