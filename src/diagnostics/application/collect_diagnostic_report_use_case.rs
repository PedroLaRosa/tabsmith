use crate::diagnostics::application::ports::diagnostic_host::DiagnosticHost;
use crate::diagnostics::domain::services::username_scrubber::UsernameScrubber;
use crate::diagnostics::domain::value_objects::diagnostic_report::DiagnosticReport;
use crate::diagnostics::domain::value_objects::diagnostic_section::DiagnosticSection;
use crate::diagnostics::domain::value_objects::diagnostic_value::DiagnosticValue;
use std::rc::Rc;

const SSH_VARIABLES: [&str; 3] = ["SSH_CLIENT", "SSH_CONNECTION", "SSH_TTY"];
const REPORTED_VARIABLES: [&str; 5] = ["SHELL", "TERM", "COLORTERM", "ZDOTDIR", "PATH"];
const REPORTED_PREFIX: &str = "TABSMITH_";

/// Describes the installation and the session it runs in, for a bug report: the user name is hidden in every
/// value that can carry a path, and the keys and flags are never touched.
pub(crate) struct CollectDiagnosticReportUseCase {
    host: Rc<dyn DiagnosticHost>,
}

impl CollectDiagnosticReportUseCase {
    pub(crate) fn new(host: Rc<dyn DiagnosticHost>) -> Self {
        CollectDiagnosticReportUseCase { host }
    }

    pub(crate) fn execute(&self) -> DiagnosticReport {
        let scrubber = UsernameScrubber::new(self.host.user());
        DiagnosticReport::new(vec![
            self.tabsmith_section(),
            self.environment_section(&scrubber),
            self.environment_variables_section(&scrubber),
        ])
    }

    fn tabsmith_section(&self) -> DiagnosticSection {
        let rows = vec![
            row("version", DiagnosticValue::Text(self.host.version())),
            row("os", DiagnosticValue::Text(self.host.platform())),
        ];
        DiagnosticSection::new("tabsmith".to_string(), rows)
    }

    fn environment_section(&self, scrubber: &UsernameScrubber) -> DiagnosticSection {
        let shell = self.host.non_empty_env("SHELL");
        let shell_version = shell.as_deref().and_then(|shell| self.host.shell_version(shell));
        let rows = vec![
            row("cwd", scrubbed_text(scrubber, self.host.current_dir())),
            row("cli-path", scrubbed_text(scrubber, self.host.current_exe())),
            row("shell-path", scrubbed_text(scrubber, shell)),
            row("shell-version", scrubbed_text(scrubber, shell_version)),
            row("terminal", scrubbed_text(scrubber, self.terminal())),
            row("in-ssh", DiagnosticValue::Flag(self.is_in_ssh())),
            row("in-ci", DiagnosticValue::Flag(self.host.non_empty_env("CI").is_some())),
            row("in-wsl", DiagnosticValue::Flag(self.host.in_wsl())),
        ];
        DiagnosticSection::new("environment".to_string(), rows)
    }

    fn terminal(&self) -> Option<String> {
        self.host
            .non_empty_env("TERM_PROGRAM")
            .or_else(|| self.host.non_empty_env("WT_SESSION").map(|_| "Windows Terminal".to_string()))
            .or_else(|| self.host.non_empty_env("TERM"))
    }

    fn is_in_ssh(&self) -> bool {
        SSH_VARIABLES.iter().any(|name| self.host.non_empty_env(name).is_some())
    }

    fn environment_variables_section(&self, scrubber: &UsernameScrubber) -> DiagnosticSection {
        let mut variables: Vec<(String, String)> = self
            .host
            .environment_variables()
            .into_iter()
            .filter(|(name, _)| is_reported_variable(name))
            .collect();
        variables.sort_by(|a, b| a.0.cmp(&b.0));
        let rows = variables
            .into_iter()
            .map(|(name, value)| (name, DiagnosticValue::Text(scrubber.clean(&value))))
            .collect();
        DiagnosticSection::new("env-vars".to_string(), rows)
    }
}

fn is_reported_variable(name: &str) -> bool {
    name.starts_with(REPORTED_PREFIX) || REPORTED_VARIABLES.contains(&name)
}

fn row(key: &str, value: DiagnosticValue) -> (String, DiagnosticValue) {
    (key.to_string(), value)
}

fn scrubbed_text(scrubber: &UsernameScrubber, text: Option<String>) -> DiagnosticValue {
    match text {
        Some(text) => DiagnosticValue::Text(scrubber.clean(&text)),
        None => DiagnosticValue::Missing,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diagnostics::application::ports::diagnostic_host::InMemoryDiagnosticHost;

    fn report_of(host: InMemoryDiagnosticHost) -> DiagnosticReport {
        CollectDiagnosticReportUseCase::new(Rc::new(host)).execute()
    }

    fn rows_of(report: &DiagnosticReport, section: &str) -> Vec<(String, DiagnosticValue)> {
        let found = report.sections().iter().find(|s| s.name() == section);
        found.map(|s| s.rows().to_vec()).unwrap_or_default()
    }

    fn environment_value(host: InMemoryDiagnosticHost, key: &str) -> DiagnosticValue {
        let rows = rows_of(&report_of(host), "environment");
        let row = rows.into_iter().find(|(name, _)| name == key);
        row.map(|(_, value)| value).expect("row is present")
    }

    fn text(value: &str) -> DiagnosticValue {
        DiagnosticValue::Text(value.to_string())
    }

    fn env_vars_of(host: InMemoryDiagnosticHost) -> Vec<(String, DiagnosticValue)> {
        rows_of(&report_of(host), "env-vars")
    }

    #[test]
    fn should_list_the_sections_tabsmith_environment_and_env_vars_in_that_order() {
        let report = report_of(InMemoryDiagnosticHost::default());

        let names: Vec<&str> = report.sections().iter().map(|s| s.name()).collect();

        assert_eq!(names, ["tabsmith", "environment", "env-vars"]);
    }

    #[test]
    fn should_report_the_version_and_the_platform_the_host_gives_without_scrubbing() {
        let host = InMemoryDiagnosticHost::default()
            .with_user("bob")
            .with_version("/bob/1.2.3")
            .with_platform("/bob macos aarch64");

        let rows = rows_of(&report_of(host), "tabsmith");

        assert_eq!(
            rows,
            [
                ("version".to_string(), text("/bob/1.2.3")),
                ("os".to_string(), text("/bob macos aarch64")),
            ]
        );
    }

    #[test]
    fn should_list_the_environment_rows_in_the_order_of_the_report() {
        let rows = rows_of(&report_of(InMemoryDiagnosticHost::default()), "environment");

        let keys: Vec<&str> = rows.iter().map(|(key, _)| key.as_str()).collect();

        assert_eq!(
            keys,
            [
                "cwd",
                "cli-path",
                "shell-path",
                "shell-version",
                "terminal",
                "in-ssh",
                "in-ci",
                "in-wsl"
            ]
        );
    }

    #[test]
    fn should_report_the_current_directory_and_executable_the_host_gives() {
        let host = InMemoryDiagnosticHost::default()
            .with_current_dir("/work")
            .with_current_exe("/usr/bin/tabsmith");

        let rows = rows_of(&report_of(host), "environment");

        assert_eq!(
            (rows[0].1.clone(), rows[1].1.clone()),
            (text("/work"), text("/usr/bin/tabsmith"))
        );
    }

    #[test]
    fn should_report_missing_directory_and_executable_when_the_host_has_none() {
        let rows = rows_of(&report_of(InMemoryDiagnosticHost::default()), "environment");

        assert_eq!(
            (rows[0].1.clone(), rows[1].1.clone()),
            (DiagnosticValue::Missing, DiagnosticValue::Missing)
        );
    }

    #[test]
    fn should_report_the_shell_path_and_version_when_shell_is_set() {
        let host = InMemoryDiagnosticHost::default()
            .with_env("SHELL", "/bin/zsh")
            .with_shell_version("/bin/zsh", Some("zsh 5.9"));

        let rows = rows_of(&report_of(host), "environment");

        assert_eq!(
            (rows[2].1.clone(), rows[3].1.clone()),
            (text("/bin/zsh"), text("zsh 5.9"))
        );
    }

    #[test]
    fn should_report_the_shell_path_and_version_missing_when_shell_is_unset() {
        let host = InMemoryDiagnosticHost::default().with_shell_version("/bin/zsh", Some("zsh 5.9"));

        let rows = rows_of(&report_of(host), "environment");

        assert_eq!(
            (rows[2].1.clone(), rows[3].1.clone()),
            (DiagnosticValue::Missing, DiagnosticValue::Missing)
        );
    }

    #[test]
    fn should_report_the_shell_path_and_version_missing_when_shell_is_empty() {
        let host = InMemoryDiagnosticHost::default()
            .with_env("SHELL", "")
            .with_shell_version("", Some("never asked"));

        let rows = rows_of(&report_of(host), "environment");

        assert_eq!(
            (rows[2].1.clone(), rows[3].1.clone()),
            (DiagnosticValue::Missing, DiagnosticValue::Missing)
        );
    }

    #[test]
    fn should_report_the_shell_version_missing_when_the_shell_prints_none() {
        let host = InMemoryDiagnosticHost::default()
            .with_env("SHELL", "/bin/dash")
            .with_shell_version("/bin/dash", None);

        let version = environment_value(host, "shell-version");

        assert_eq!(version, DiagnosticValue::Missing);
    }

    #[test]
    fn should_keep_an_empty_shell_version_as_empty_text() {
        let host = InMemoryDiagnosticHost::default()
            .with_env("SHELL", "/bin/sh")
            .with_shell_version("/bin/sh", Some(""));

        let version = environment_value(host, "shell-version");

        assert_eq!(version, text(""));
    }

    #[test]
    fn should_prefer_term_program_over_the_windows_terminal_session_and_term() {
        let host = InMemoryDiagnosticHost::default()
            .with_env("TERM_PROGRAM", "iTerm.app")
            .with_env("WT_SESSION", "guid")
            .with_env("TERM", "xterm");

        let terminal = environment_value(host, "terminal");

        assert_eq!(terminal, text("iTerm.app"));
    }

    #[test]
    fn should_name_windows_terminal_when_only_its_session_variable_is_set() {
        let host = InMemoryDiagnosticHost::default()
            .with_env("WT_SESSION", "guid")
            .with_env("TERM", "xterm");

        let terminal = environment_value(host, "terminal");

        assert_eq!(terminal, text("Windows Terminal"));
    }

    #[test]
    fn should_fall_back_to_term_for_the_terminal() {
        let host = InMemoryDiagnosticHost::default().with_env("TERM", "xterm-256color");

        let terminal = environment_value(host, "terminal");

        assert_eq!(terminal, text("xterm-256color"));
    }

    #[test]
    fn should_skip_an_empty_term_program_for_the_terminal() {
        let host = InMemoryDiagnosticHost::default()
            .with_env("TERM_PROGRAM", "")
            .with_env("TERM", "screen");

        let terminal = environment_value(host, "terminal");

        assert_eq!(terminal, text("screen"));
    }

    #[test]
    fn should_report_the_terminal_missing_when_nothing_names_it() {
        let terminal = environment_value(InMemoryDiagnosticHost::default(), "terminal");

        assert_eq!(terminal, DiagnosticValue::Missing);
    }

    #[test]
    fn should_report_ssh_when_ssh_client_is_set() {
        let host = InMemoryDiagnosticHost::default().with_env("SSH_CLIENT", "1.2.3.4 22 22");

        let in_ssh = environment_value(host, "in-ssh");

        assert_eq!(in_ssh, DiagnosticValue::Flag(true));
    }

    #[test]
    fn should_report_ssh_when_ssh_connection_is_set() {
        let host = InMemoryDiagnosticHost::default().with_env("SSH_CONNECTION", "a b c d");

        let in_ssh = environment_value(host, "in-ssh");

        assert_eq!(in_ssh, DiagnosticValue::Flag(true));
    }

    #[test]
    fn should_report_ssh_when_ssh_tty_is_set() {
        let host = InMemoryDiagnosticHost::default().with_env("SSH_TTY", "/dev/pts/0");

        let in_ssh = environment_value(host, "in-ssh");

        assert_eq!(in_ssh, DiagnosticValue::Flag(true));
    }

    #[test]
    fn should_not_report_ssh_when_the_ssh_variables_are_unset_or_empty() {
        let host = InMemoryDiagnosticHost::default().with_env("SSH_TTY", "");

        let in_ssh = environment_value(host, "in-ssh");

        assert_eq!(in_ssh, DiagnosticValue::Flag(false));
    }

    #[test]
    fn should_report_ci_when_ci_has_a_value() {
        let host = InMemoryDiagnosticHost::default().with_env("CI", "true");

        let in_ci = environment_value(host, "in-ci");

        assert_eq!(in_ci, DiagnosticValue::Flag(true));
    }

    #[test]
    fn should_not_report_ci_when_ci_is_empty() {
        let host = InMemoryDiagnosticHost::default().with_env("CI", "");

        let in_ci = environment_value(host, "in-ci");

        assert_eq!(in_ci, DiagnosticValue::Flag(false));
    }

    #[test]
    fn should_not_report_ci_when_ci_is_unset() {
        let in_ci = environment_value(InMemoryDiagnosticHost::default(), "in-ci");

        assert_eq!(in_ci, DiagnosticValue::Flag(false));
    }

    #[test]
    fn should_pass_the_wsl_answer_of_the_host_through() {
        let host = InMemoryDiagnosticHost::default().with_wsl(true);

        let in_wsl = environment_value(host, "in-wsl");

        assert_eq!(in_wsl, DiagnosticValue::Flag(true));
    }

    #[test]
    fn should_not_report_wsl_on_a_plain_host() {
        let in_wsl = environment_value(InMemoryDiagnosticHost::default(), "in-wsl");

        assert_eq!(in_wsl, DiagnosticValue::Flag(false));
    }

    #[test]
    fn should_scrub_the_user_from_the_directory_executable_and_shell_path() {
        let host = InMemoryDiagnosticHost::default()
            .with_user("bob")
            .with_current_dir("/home/bob/work")
            .with_current_exe("C:\\Users\\bob\\tabsmith.exe")
            .with_env("SHELL", "/home/bob/bin/zsh");

        let rows = rows_of(&report_of(host), "environment");

        assert_eq!(
            (rows[0].1.clone(), rows[1].1.clone(), rows[2].1.clone()),
            (
                text("/home/USER/work"),
                text("C:\\Users\\USER\\tabsmith.exe"),
                text("/home/USER/bin/zsh")
            )
        );
    }

    #[test]
    fn should_scrub_the_user_from_the_shell_version_and_the_terminal() {
        let host = InMemoryDiagnosticHost::default()
            .with_user("bob")
            .with_env("SHELL", "/bin/zsh")
            .with_shell_version("/bin/zsh", Some("zsh from /home/bob/zsh"))
            .with_env("TERM_PROGRAM", "/home/bob/term");

        let rows = rows_of(&report_of(host), "environment");

        assert_eq!(
            (rows[3].1.clone(), rows[4].1.clone()),
            (text("zsh from /home/USER/zsh"), text("/home/USER/term"))
        );
    }

    #[test]
    fn should_scrub_the_user_from_the_values_of_the_environment_variables() {
        let host = InMemoryDiagnosticHost::default()
            .with_user("bob")
            .with_env("PATH", "/home/bob/bin:/usr/bin");

        let variables = env_vars_of(host);

        assert_eq!(
            variables,
            [("PATH".to_string(), text("/home/USER/bin:/usr/bin"))]
        );
    }

    #[test]
    fn should_not_scrub_the_keys_of_the_environment_variables() {
        let host = InMemoryDiagnosticHost::default()
            .with_user("bob")
            .with_env("TABSMITH_/bob", "x");

        let variables = env_vars_of(host);

        assert_eq!(variables, [("TABSMITH_/bob".to_string(), text("x"))]);
    }

    #[test]
    fn should_keep_only_the_tabsmith_prefixed_and_the_named_environment_variables() {
        let host = InMemoryDiagnosticHost::default()
            .with_env("HOME", "/home/bob")
            .with_env("TABSMITH_HOME", "/t")
            .with_env("tabsmith_lower", "no")
            .with_env("TERM_PROGRAM", "no")
            .with_env("COLORTERM", "truecolor")
            .with_env("ZDOTDIR", "/z")
            .with_env("LANG", "en");

        let keys: Vec<String> = env_vars_of(host).into_iter().map(|(key, _)| key).collect();

        assert_eq!(keys, ["COLORTERM", "TABSMITH_HOME", "ZDOTDIR"]);
    }

    #[test]
    fn should_sort_the_environment_variables_by_the_bytes_of_their_keys() {
        let host = InMemoryDiagnosticHost::default()
            .with_env("ZDOTDIR", "1")
            .with_env("TERM", "1")
            .with_env("TABSMITH_B", "1")
            .with_env("SHELL", "1")
            .with_env("TABSMITH_A", "1")
            .with_env("PATH", "1")
            .with_env("COLORTERM", "1");

        let keys: Vec<String> = env_vars_of(host).into_iter().map(|(key, _)| key).collect();

        assert_eq!(
            keys,
            [
                "COLORTERM",
                "PATH",
                "SHELL",
                "TABSMITH_A",
                "TABSMITH_B",
                "TERM",
                "ZDOTDIR"
            ]
        );
    }

    #[test]
    fn should_keep_an_environment_variable_with_an_empty_value() {
        let host = InMemoryDiagnosticHost::default().with_env("TABSMITH_DEBUG", "");

        let variables = env_vars_of(host);

        assert_eq!(variables, [("TABSMITH_DEBUG".to_string(), text(""))]);
    }

    #[test]
    fn should_report_no_environment_variables_on_a_bare_host() {
        let variables = env_vars_of(InMemoryDiagnosticHost::default());

        assert_eq!(variables, []);
    }
}
