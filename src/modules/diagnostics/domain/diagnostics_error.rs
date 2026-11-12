use crate::modules::shared::domain::shared_error::SharedError;
use crate::modules::shell_integration::domain::shell_integration_error::ShellIntegrationError;
use std::fmt;

/// Why the diagnostics could not find out a fact they need; the text is the wrapped error's, unchanged.
#[derive(Debug)]
pub(crate) enum DiagnosticsError {
    IntegrationPaths(ShellIntegrationError),
    DataDirectory(SharedError),
}

impl fmt::Display for DiagnosticsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DiagnosticsError::IntegrationPaths(error) => write!(f, "{error}"),
            DiagnosticsError::DataDirectory(error) => write!(f, "{error}"),
        }
    }
}

// No `source()`: anyhow's `{:#}` would print the wrapped text a second time.
impl std::error::Error for DiagnosticsError {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::shared::domain::shared_error::SharedError;
    use crate::modules::shell_integration::domain::shell_integration_error::ShellIntegrationError;

    #[test]
    fn should_display_the_integration_paths_error_text_unchanged() {
        let wrapped = ShellIntegrationError::Environment("no home".into());
        let error = DiagnosticsError::IntegrationPaths(wrapped);

        let text = error.to_string();

        assert_eq!(text, "no home");
    }

    #[test]
    fn should_display_the_data_directory_error_text_unchanged() {
        let error = DiagnosticsError::DataDirectory(SharedError::HomeDirectoryNotFound);

        let text = error.to_string();

        assert_eq!(text, SharedError::HomeDirectoryNotFound.to_string());
    }

    #[test]
    fn should_print_the_integration_paths_text_once_in_the_alternate_form() {
        let error = DiagnosticsError::IntegrationPaths(ShellIntegrationError::Environment("no home".into()));

        let alternate = format!("{error:#}");

        assert_eq!(alternate, error.to_string());
    }

    #[test]
    fn should_print_the_data_directory_text_once_in_the_alternate_form() {
        let error = DiagnosticsError::DataDirectory(SharedError::HomeDirectoryNotFound);

        let alternate = format!("{error:#}");

        assert_eq!(alternate, error.to_string());
    }

    #[test]
    fn should_have_no_source_so_that_anyhow_does_not_repeat_the_text() {
        let error = DiagnosticsError::DataDirectory(SharedError::HomeDirectoryNotFound);

        let source = std::error::Error::source(&error);

        assert!(source.is_none());
    }
}
