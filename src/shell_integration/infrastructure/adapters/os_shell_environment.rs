use crate::shell_integration::domain::shell_integration_error::ShellIntegrationError;
use std::error::Error;
use std::path::PathBuf;

/// The `$PROFILE` pwsh printed (trimmed), or why it is unknown.
fn profile_from(output: std::io::Result<String>) -> Result<PathBuf, ShellIntegrationError> {
    let out = output.map_err(|e| {
        ShellIntegrationError::Environment(format!(
            "could not ask pwsh for $PROFILE: {}",
            display_chain(&e)
        ))
    })?;
    match out.trim() {
        "" => Err(ShellIntegrationError::Environment(
            "pwsh printed an empty $PROFILE".to_string(),
        )),
        p => Ok(PathBuf::from(p)),
    }
}

/// A fact of the environment that could not be found out, with its causes rendered once (P3).
fn environment_error(error: impl Error + 'static) -> ShellIntegrationError {
    ShellIntegrationError::Environment(display_chain(&error))
}

/// `error` as anyhow's `{:#}` prints it: its Display, then `: ` and the Display of each `source()` in turn.
fn display_chain(error: &(dyn Error + 'static)) -> String {
    std::iter::successors(Some(error), |&e| e.source())
        .map(|e| e.to_string())
        .collect::<Vec<_>>()
        .join(": ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shared::domain::shared_error::SharedError;
    use crate::shared::infrastructure::os::executable_path_error::ExecutablePathError;
    use std::io;

    #[cfg(unix)]
    #[test]
    fn should_report_a_pwsh_that_cannot_start_as_the_io_error_under_the_pwsh_context() {
        let missing_program = io::Error::from_raw_os_error(2);

        let error = profile_from(Err(missing_program)).unwrap_err();

        assert_eq!(
            error.to_string(),
            "could not ask pwsh for $PROFILE: No such file or directory (os error 2)"
        );
    }

    #[test]
    fn should_report_a_profile_of_only_whitespace_as_empty() {
        let printed = Ok(" \r\n".to_string());

        let error = profile_from(printed).unwrap_err();

        assert_eq!(error.to_string(), "pwsh printed an empty $PROFILE");
    }

    #[test]
    fn should_report_an_executable_it_could_not_locate_then_the_io_cause() {
        let unlocatable = ExecutablePathError::from(io::Error::from(io::ErrorKind::NotFound));

        let error = environment_error(unlocatable);

        assert_eq!(
            error.to_string(),
            "could not locate the tabsmith executable: entity not found"
        );
    }

    /// `output_with_timeout`'s own error (`process_runner.rs` pins the same text).
    #[test]
    fn should_report_a_pwsh_still_running_at_the_timeout_under_the_pwsh_context() {
        let timed_out = io::Error::new(io::ErrorKind::TimedOut, "timed out after 0s");

        let error = profile_from(Err(timed_out)).unwrap_err();

        assert_eq!(
            error.to_string(),
            "could not ask pwsh for $PROFILE: timed out after 0s"
        );
    }

    #[test]
    fn should_render_every_cause_under_the_pwsh_context_as_anyhow_does() {
        let nested = || {
            io::Error::other(ExecutablePathError::from(io::Error::from(
                io::ErrorKind::NotFound,
            )))
        };

        let error = profile_from(Err(nested())).unwrap_err();

        let anyhow_text = format!(
            "{:#}",
            anyhow::Error::from(nested()).context("could not ask pwsh for $PROFILE")
        );
        assert_eq!(error.to_string(), anyhow_text);
    }

    #[test]
    fn should_give_the_profile_pwsh_printed_without_its_line_ending() {
        let printed = Ok("/home/u/.config/powershell/Microsoft.PowerShell_profile.ps1\r\n".to_string());

        let profile = profile_from(printed).unwrap();

        assert_eq!(
            profile,
            PathBuf::from("/home/u/.config/powershell/Microsoft.PowerShell_profile.ps1")
        );
    }

    #[test]
    fn should_report_an_unknown_home_by_the_shared_error_text_alone() {
        let unknown = [SharedError::UserHomeNotFound, SharedError::HomeDirectoryNotFound];

        let messages = unknown.map(|e| environment_error(e).to_string());

        assert_eq!(
            messages,
            [
                "could not determine the home directory",
                "could not determine the home directory (set HOME or TABSMITH_HOME)",
            ]
        );
    }
}
