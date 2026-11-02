#[cfg(test)]
use std::collections::HashMap;

/// What `tabsmith debug` asks the machine it runs on to describe the session in a bug report.
pub(crate) trait DiagnosticHost {
    /// The name of the user (`USERNAME` on Windows, `USER` elsewhere); empty when it is unset or not valid UTF-8.
    fn user(&self) -> String;

    /// The value of an environment variable; unset, empty and not valid UTF-8 are all `None`. This is not the
    /// presence rule doctor applies to the environment: the report treats an empty value as missing.
    fn env_text(&self, name: &str) -> Option<String>;

    /// Every environment variable, as the process sees them: not filtered, not sorted, names that are not valid
    /// UTF-8 already dropped and values converted lossily.
    fn environment_variables(&self) -> Vec<(String, String)>;

    /// The first line `<shell> --version` prints on stdout, trimmed; a blank first line is `Some("")`.
    /// `None` when the shell cannot be run or prints nothing. Waits for the shell as long as it takes.
    fn shell_version(&self, shell: &str) -> Option<String>;

    fn current_dir(&self) -> Option<String>;

    fn current_exe(&self) -> Option<String>;

    /// Whether this runs inside the Windows Subsystem for Linux; the whole test lives in the adapter.
    fn in_wsl(&self) -> bool;

    fn version(&self) -> String;

    /// The operating system and architecture, e.g. `linux x86_64`.
    fn platform(&self) -> String;
}

/// A described machine: every answer comes from what a test configured, with a plain default for the rest
/// (no user, an empty environment, nothing known about the shell or the directories, not WSL).
#[cfg(test)]
pub(crate) struct InMemoryDiagnosticHost {
    user: String,
    environment: Vec<(String, String)>,
    shell_versions: HashMap<String, Option<String>>,
    current_dir: Option<String>,
    current_exe: Option<String>,
    in_wsl: bool,
    version: String,
    platform: String,
}

#[cfg(test)]
impl Default for InMemoryDiagnosticHost {
    fn default() -> Self {
        InMemoryDiagnosticHost {
            user: String::new(),
            environment: Vec::new(),
            shell_versions: HashMap::new(),
            current_dir: None,
            current_exe: None,
            in_wsl: false,
            version: "0.0.0-test".to_string(),
            platform: "linux x86_64".to_string(),
        }
    }
}

#[cfg(test)]
impl InMemoryDiagnosticHost {
    pub(crate) fn with_user(mut self, user: &str) -> Self {
        self.user = user.to_string();
        self
    }

    /// Sets a variable; setting a name again replaces its value and keeps its place.
    pub(crate) fn with_env(mut self, name: &str, value: &str) -> Self {
        match self.environment.iter_mut().find(|(key, _)| key == name) {
            Some(entry) => entry.1 = value.to_string(),
            None => self.environment.push((name.to_string(), value.to_string())),
        }
        self
    }

    pub(crate) fn with_shell_version(mut self, shell: &str, first_line: Option<&str>) -> Self {
        self.shell_versions
            .insert(shell.to_string(), first_line.map(str::to_string));
        self
    }

    pub(crate) fn with_current_dir(mut self, directory: &str) -> Self {
        self.current_dir = Some(directory.to_string());
        self
    }

    pub(crate) fn with_current_exe(mut self, executable: &str) -> Self {
        self.current_exe = Some(executable.to_string());
        self
    }

    pub(crate) fn with_wsl(mut self, in_wsl: bool) -> Self {
        self.in_wsl = in_wsl;
        self
    }

    pub(crate) fn with_version(mut self, version: &str) -> Self {
        self.version = version.to_string();
        self
    }

    pub(crate) fn with_platform(mut self, platform: &str) -> Self {
        self.platform = platform.to_string();
        self
    }
}

#[cfg(test)]
impl DiagnosticHost for InMemoryDiagnosticHost {
    fn user(&self) -> String {
        self.user.clone()
    }

    fn env_text(&self, name: &str) -> Option<String> {
        self.environment
            .iter()
            .find(|(key, _)| key == name)
            .map(|(_, value)| value.clone())
            .filter(|value| !value.is_empty())
    }

    fn environment_variables(&self) -> Vec<(String, String)> {
        self.environment.clone()
    }

    fn shell_version(&self, shell: &str) -> Option<String> {
        self.shell_versions.get(shell).cloned().flatten()
    }

    fn current_dir(&self) -> Option<String> {
        self.current_dir.clone()
    }

    fn current_exe(&self) -> Option<String> {
        self.current_exe.clone()
    }

    fn in_wsl(&self) -> bool {
        self.in_wsl
    }

    fn version(&self) -> String {
        self.version.clone()
    }

    fn platform(&self) -> String {
        self.platform.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn should_describe_a_bare_machine_by_default() {
        let host = InMemoryDiagnosticHost::default();

        let described = (
            host.user(),
            host.env_text("SHELL"),
            host.environment_variables(),
            host.shell_version("/bin/zsh"),
            host.current_dir(),
            host.current_exe(),
            host.in_wsl(),
        );

        assert_eq!(
            described,
            (String::new(), None, Vec::new(), None, None, None, false)
        );
    }

    #[test]
    fn should_answer_the_version_and_platform_it_was_given() {
        let host = InMemoryDiagnosticHost::default()
            .with_version("1.2.3")
            .with_platform("macos aarch64");

        assert_eq!(
            (host.version(), host.platform()),
            ("1.2.3".to_string(), "macos aarch64".to_string())
        );
    }

    #[test]
    fn should_answer_the_user_directory_and_executable_it_was_given() {
        let host = InMemoryDiagnosticHost::default()
            .with_user("pedro")
            .with_current_dir("/work")
            .with_current_exe("/usr/bin/tabsmith");

        assert_eq!(
            (host.user(), host.current_dir(), host.current_exe()),
            (
                "pedro".to_string(),
                Some("/work".to_string()),
                Some("/usr/bin/tabsmith".to_string())
            )
        );
    }

    #[test]
    fn should_answer_env_text_for_a_non_empty_variable() {
        let host = InMemoryDiagnosticHost::default().with_env("SHELL", "/bin/zsh");

        let shell = host.env_text("SHELL");

        assert_eq!(shell, Some("/bin/zsh".to_string()));
    }

    #[test]
    fn should_treat_an_empty_variable_as_missing_for_env_text() {
        let host = InMemoryDiagnosticHost::default().with_env("CI", "");

        let ci = host.env_text("CI");

        assert_eq!(ci, None);
    }

    #[test]
    fn should_still_list_an_empty_variable_among_the_environment_variables() {
        let host = InMemoryDiagnosticHost::default().with_env("CI", "");

        let variables = host.environment_variables();

        assert_eq!(variables, [("CI".to_string(), String::new())]);
    }

    #[test]
    fn should_list_the_environment_variables_in_the_order_they_were_set() {
        let host = InMemoryDiagnosticHost::default()
            .with_env("ZDOTDIR", "/z")
            .with_env("PATH", "/bin")
            .with_env("TABSMITH_DEBUG", "1");

        let names: Vec<String> = host
            .environment_variables()
            .into_iter()
            .map(|(name, _)| name)
            .collect();

        assert_eq!(names, ["ZDOTDIR", "PATH", "TABSMITH_DEBUG"]);
    }

    #[test]
    fn should_replace_the_value_of_a_variable_set_twice() {
        let host = InMemoryDiagnosticHost::default()
            .with_env("TERM", "xterm")
            .with_env("TERM", "screen");

        assert_eq!(
            (host.environment_variables(), host.env_text("TERM")),
            (
                vec![("TERM".to_string(), "screen".to_string())],
                Some("screen".to_string())
            )
        );
    }

    #[test]
    fn should_answer_the_shell_version_scripted_for_that_shell() {
        let host = InMemoryDiagnosticHost::default()
            .with_shell_version("/bin/zsh", Some("zsh 5.9 (arm64)"));

        let version = host.shell_version("/bin/zsh");

        assert_eq!(version, Some("zsh 5.9 (arm64)".to_string()));
    }

    #[test]
    fn should_keep_a_blank_first_line_as_some_empty_text() {
        let host = InMemoryDiagnosticHost::default().with_shell_version("/bin/sh", Some(""));

        let version = host.shell_version("/bin/sh");

        assert_eq!(version, Some(String::new()));
    }

    #[test]
    fn should_answer_none_for_a_shell_that_prints_no_version() {
        let host = InMemoryDiagnosticHost::default().with_shell_version("/bin/dash", None);

        let version = host.shell_version("/bin/dash");

        assert_eq!(version, None);
    }

    #[test]
    fn should_answer_none_for_a_shell_nobody_scripted() {
        let host = InMemoryDiagnosticHost::default().with_shell_version("/bin/zsh", Some("zsh"));

        let version = host.shell_version("/bin/fish");

        assert_eq!(version, None);
    }

    #[test]
    fn should_report_wsl_when_the_host_was_told_so() {
        let host = InMemoryDiagnosticHost::default().with_wsl(true);

        assert!(host.in_wsl());
    }
}
