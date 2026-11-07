use crate::modules::diagnostics::application::ports::diagnostic_host::DiagnosticHost;

/// The machine `tabsmith debug` runs on, asked through the process environment and the operating system.
pub(crate) struct OsDiagnosticHost;

impl DiagnosticHost for OsDiagnosticHost {
    fn user(&self) -> String {
        std::env::var(if cfg!(windows) { "USERNAME" } else { "USER" }).unwrap_or_default()
    }

    fn non_empty_env(&self, name: &str) -> Option<String> {
        std::env::var(name).ok().filter(|value| !value.is_empty())
    }

    fn environment_variables(&self) -> Vec<(String, String)> {
        std::env::vars_os()
            .filter_map(|(name, value)| {
                let name = name.into_string().ok()?;
                Some((name, value.to_string_lossy().into_owned()))
            })
            .collect()
    }

    /// No timeout on purpose: a shell that blocks on `--version` hangs `tabsmith debug` today as well.
    fn shell_version(&self, shell: &str) -> Option<String> {
        let output = std::process::Command::new(shell)
            .arg("--version")
            .output()
            .ok()?;
        first_line_of(&output.stdout)
    }

    fn current_dir(&self) -> Option<String> {
        std::env::current_dir()
            .ok()
            .map(|path| path.display().to_string())
    }

    fn current_exe(&self) -> Option<String> {
        std::env::current_exe()
            .ok()
            .map(|path| path.display().to_string())
    }

    fn in_wsl(&self) -> bool {
        cfg!(target_os = "linux")
            && (self.non_empty_env("WSL_DISTRO_NAME").is_some()
                || std::fs::read_to_string("/proc/sys/kernel/osrelease")
                    .is_ok_and(|release| release.to_lowercase().contains("microsoft")))
    }

    fn version(&self) -> String {
        env!("CARGO_PKG_VERSION").to_string()
    }

    fn platform(&self) -> String {
        format!("{} {}", std::env::consts::OS, std::env::consts::ARCH)
    }
}

fn first_line_of(stdout: &[u8]) -> Option<String> {
    String::from_utf8_lossy(stdout)
        .lines()
        .next()
        .map(|line| line.trim().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn should_keep_the_trimmed_first_line_of_a_one_line_version() {
        let first_line = first_line_of(b"GNU bash, version 5.2\n");

        assert_eq!(first_line, Some("GNU bash, version 5.2".to_string()));
    }

    #[test]
    fn should_keep_a_leading_blank_line_of_the_shell_output_as_some_empty_text() {
        let first_line = first_line_of(b"\nGNU bash, version 5.2\n");

        assert_eq!(first_line, Some(String::new()));
    }

    #[test]
    fn should_keep_only_the_trimmed_first_of_several_lines() {
        let first_line = first_line_of(b"  fish, version 3.7.1  \nsecond");

        assert_eq!(first_line, Some("fish, version 3.7.1".to_string()));
    }

    #[test]
    fn should_give_none_when_the_shell_printed_nothing() {
        let first_line = first_line_of(b"");

        assert_eq!(first_line, None);
    }

    #[test]
    fn should_convert_invalid_utf8_lossily_in_the_first_line() {
        let first_line = first_line_of(b"zsh 5.9 \xff\n");

        assert_eq!(first_line, Some("zsh 5.9 \u{fffd}".to_string()));
    }

    #[test]
    fn should_lose_the_carriage_return_of_a_crlf_first_line() {
        let first_line = first_line_of(b"pwsh 7.4\r\nsecond\r\n");

        assert_eq!(first_line, Some("pwsh 7.4".to_string()));
    }

    #[test]
    fn should_give_no_shell_version_for_a_program_that_does_not_exist() {
        let version = OsDiagnosticHost.shell_version("/nonexistent/tabsmith-test-shell");

        assert_eq!(version, None);
    }

    #[test]
    fn should_give_no_text_for_a_variable_that_is_certainly_unset() {
        let value = OsDiagnosticHost.non_empty_env("TABSMITH_TEST_CERTAINLY_UNSET_VARIABLE");

        assert_eq!(value, None);
    }

    #[test]
    fn should_describe_the_platform_as_operating_system_and_architecture() {
        let platform = OsDiagnosticHost.platform();

        assert_eq!(
            platform,
            format!(
                "{} {}",
                std::env::consts::OS,
                std::env::consts::ARCH
            )
        );
    }

    #[test]
    fn should_report_the_crate_version() {
        let version = OsDiagnosticHost.version();

        assert_eq!(version, env!("CARGO_PKG_VERSION"));
    }

    #[test]
    fn should_have_a_space_between_the_operating_system_and_the_architecture() {
        let platform = OsDiagnosticHost.platform();

        assert!(platform.contains(' '));
    }

    #[test]
    fn should_have_a_non_empty_version() {
        let version = OsDiagnosticHost.version();

        assert!(!version.is_empty());
    }
}
