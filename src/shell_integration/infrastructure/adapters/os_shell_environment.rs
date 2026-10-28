use crate::shell_integration::domain::shell_integration_error::ShellIntegrationError;
use std::path::PathBuf;

/// The `$PROFILE` pwsh printed, or why it could not be asked.
fn profile_from(output: std::io::Result<String>) -> Result<PathBuf, ShellIntegrationError> {
    let out = output
        .map_err(|e| ShellIntegrationError::Environment(format!("could not ask pwsh for $PROFILE: {e}")))?;
    Ok(PathBuf::from(out))
}

#[cfg(test)]
mod tests {
    use super::*;
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
}
