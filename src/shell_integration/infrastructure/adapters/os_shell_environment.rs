use crate::shell_integration::domain::shell_integration_error::ShellIntegrationError;
use std::error::Error;
use std::path::PathBuf;

/// The `$PROFILE` pwsh printed (trimmed), or why it is unknown.
fn profile_from(output: std::io::Result<String>) -> Result<PathBuf, ShellIntegrationError> {
    let out = output
        .map_err(|e| ShellIntegrationError::Environment(format!("could not ask pwsh for $PROFILE: {e}")))?;
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
}
