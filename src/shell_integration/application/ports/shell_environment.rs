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

/// Records which facts were asked, in order, for tests.
#[cfg(test)]
#[derive(Default)]
pub(crate) struct RecordingShellEnvironment {
    asked: RefCell<Vec<&'static str>>,
}

#[cfg(test)]
impl RecordingShellEnvironment {
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
        Ok(String::new())
    }

    fn parent_shell(&self) -> Option<String> {
        self.record("parent_shell");
        None
    }

    fn launch_facts(&self) -> LaunchFacts {
        self.record("launch_facts");
        LaunchFacts::new(HashMap::new(), false)
    }

    fn pwsh_profile(&self) -> Result<PathBuf, ShellIntegrationError> {
        self.record("pwsh_profile");
        Ok(PathBuf::new())
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
}
