use crate::modules::terminal_session::application::launch_request::LaunchRequest;
use crate::modules::terminal_session::domain::value_objects::wrapped_shell_command::WrappedShellCommand;

/// Decides which shell, or which command, the wrapper launches.
pub(crate) struct PrepareSessionLaunchUseCase;

impl PrepareSessionLaunchUseCase {
    /// The shell is the explicit one, else `TABSMITH_SHELL`, else `pwsh` on Windows or `SHELL` elsewhere; an empty
    /// value of the environment counts as unset. A first extra argument that is no option replaces the shell.
    pub(crate) fn execute(&self, request: LaunchRequest) -> WrappedShellCommand {
        let environment = request.environment;
        let shell = request
            .shell
            .or_else(|| non_empty(environment.tabsmith_shell))
            .or_else(|| {
                if cfg!(windows) {
                    Some("pwsh".into())
                } else {
                    non_empty(environment.system_shell)
                }
            });
        WrappedShellCommand::new(shell.as_deref(), &request.command, environment.login_shell)
    }
}

fn non_empty(value: Option<String>) -> Option<String> {
    value.filter(|text| !text.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::terminal_session::application::launch_environment::LaunchEnvironment;

    fn environment(tabsmith_shell: Option<&str>, system_shell: Option<&str>, login_shell: bool) -> LaunchEnvironment {
        LaunchEnvironment {
            tabsmith_shell: tabsmith_shell.map(String::from),
            system_shell: system_shell.map(String::from),
            login_shell,
        }
    }

    fn launch(shell: Option<&str>, command: &[&str], environment: LaunchEnvironment) -> Vec<String> {
        let request = LaunchRequest {
            shell: shell.map(String::from),
            command: command.iter().map(|argument| argument.to_string()).collect(),
            environment,
        };
        PrepareSessionLaunchUseCase.execute(request).argv().to_vec()
    }

    #[test]
    fn should_prefer_the_explicit_shell_over_the_environment() {
        let environment = environment(Some("/bin/zsh"), Some("/bin/fish"), false);

        let argv = launch(Some("/bin/bash"), &[], environment);

        assert_eq!(argv, ["/bin/bash"]);
    }

    #[cfg(not(windows))]
    #[test]
    fn should_use_tabsmith_shell_before_the_system_shell() {
        let environment = environment(Some("/bin/zsh"), Some("/bin/bash"), false);

        let argv = launch(None, &[], environment);

        assert_eq!(argv, ["/bin/zsh"]);
    }

    #[cfg(windows)]
    #[test]
    fn should_use_pwsh_on_windows_without_reading_the_system_shell() {
        let environment = environment(None, Some("/bin/bash"), false);

        let argv = launch(None, &[], environment);

        assert_eq!(argv, ["pwsh"]);
    }

    /// What is left once the empty values are out: the system shell, which Windows never reads.
    #[test]
    fn should_treat_an_empty_shell_as_unset() {
        let environment = environment(Some(""), Some("/bin/bash"), false);
        let fallback = if cfg!(windows) { "pwsh" } else { "/bin/bash" };

        let argv = launch(None, &[], environment);

        assert_eq!(argv, [fallback]);
    }

    #[test]
    fn should_add_login_only_for_the_shells_that_take_it() {
        let argvs: Vec<Vec<String>> = ["bash", "/usr/bin/zsh", "fish", "pwsh", "nu"]
            .iter()
            .map(|shell| launch(Some(shell), &[], environment(None, None, true)))
            .collect();

        assert_eq!(
            argvs,
            [
                vec!["bash", "--login"],
                vec!["/usr/bin/zsh", "--login"],
                vec!["fish", "--login"],
                vec!["pwsh"],
                vec!["nu"],
            ]
        );
    }

    #[test]
    fn should_replace_the_shell_with_a_first_extra_argument_that_is_not_an_option() {
        let environment = environment(None, Some("/bin/bash"), true);

        let argv = launch(None, &["htop", "-d", "5"], environment);

        assert_eq!(argv, ["htop", "-d", "5"]);
    }
}
