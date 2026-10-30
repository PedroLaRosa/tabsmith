use crate::settings::domain::settings_error::SettingsError;
use crate::shared::domain::shared_error::SharedError;
use std::fmt;
use std::path::PathBuf;

/// Why a theme command failed: the settings or the data directory could not be used, or the theme asked for is
/// not a valid theme file or does not exist.
#[derive(Debug)]
pub(crate) enum DropdownError {
    /// The settings could not be loaded or saved. Shown as the settings error itself.
    Settings(SettingsError),
    /// The data directory is unknown (no home). Shown as the shared error itself.
    DataDirectory(SharedError),
    /// The theme file is readable but not a valid theme. `reason` is the parser's text, already rendered.
    InvalidThemeFile { path: PathBuf, reason: String },
    /// No theme file and no built-in theme has this name.
    ThemeNotFound { name: String, folder: PathBuf },
}

impl fmt::Display for DropdownError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DropdownError::Settings(error) => write!(f, "{error}"),
            DropdownError::DataDirectory(error) => write!(f, "{error}"),
            DropdownError::InvalidThemeFile { path, reason } => {
                write!(f, "invalid theme file {}: {reason}", path.display())
            }
            DropdownError::ThemeNotFound { name, folder } => {
                write!(f, "'{name}' does not exist in {}", folder.display())
            }
        }
    }
}

impl std::error::Error for DropdownError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            // Display already is the wrapped error: continue with its own cause so nothing prints twice.
            DropdownError::Settings(error) => error.source(),
            DropdownError::DataDirectory(error) => error.source(),
            DropdownError::InvalidThemeFile { .. } | DropdownError::ThemeNotFound { .. } => None,
        }
    }
}

impl From<SettingsError> for DropdownError {
    fn from(error: SettingsError) -> Self {
        DropdownError::Settings(error)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::error::Error;

    #[derive(Debug)]
    struct ParserCause;

    impl fmt::Display for ParserCause {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.write_str("expected value at line 1 column 1")
        }
    }

    impl Error for ParserCause {}

    #[derive(Debug)]
    struct CorruptedFile(ParserCause);

    impl fmt::Display for CorruptedFile {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.write_str("/data/settings.json is corrupted")
        }
    }

    impl Error for CorruptedFile {
        fn source(&self) -> Option<&(dyn Error + 'static)> {
            Some(&self.0)
        }
    }

    fn corrupted_file() -> CorruptedFile {
        CorruptedFile(ParserCause)
    }

    #[test]
    fn should_print_a_theme_settings_failure_exactly_as_the_settings_error() {
        let wrapped = DropdownError::Settings(SettingsError::Storage(Box::new(corrupted_file())));
        let bare = SettingsError::Storage(Box::new(corrupted_file()));

        let printed = format!("{:#}", anyhow::Error::from(wrapped));

        assert_eq!(
            (printed.as_str(), printed.clone()),
            (
                "/data/settings.json is corrupted: expected value at line 1 column 1",
                format!("{:#}", anyhow::Error::from(bare))
            )
        );
    }

    #[test]
    fn should_continue_the_source_chain_of_a_settings_failure() {
        let error = DropdownError::Settings(SettingsError::Storage(Box::new(corrupted_file())));

        let cause = error.source().map(ToString::to_string);

        assert_eq!(cause.as_deref(), Some("expected value at line 1 column 1"));
    }

    #[test]
    fn should_print_an_invalid_theme_file_and_a_missing_theme_as_the_cli_did() {
        let invalid = DropdownError::InvalidThemeFile {
            path: PathBuf::from("/data/themes/x.json"),
            reason: "expected value at line 1 column 1".to_string(),
        };
        let missing = DropdownError::ThemeNotFound {
            name: "nope".to_string(),
            folder: PathBuf::from("/data/themes"),
        };

        let printed = (invalid.to_string(), missing.to_string());

        assert_eq!(
            printed,
            (
                "invalid theme file /data/themes/x.json: expected value at line 1 column 1"
                    .to_string(),
                "'nope' does not exist in /data/themes".to_string()
            )
        );
    }

    #[test]
    fn should_print_a_missing_home_exactly_as_the_shared_error() {
        let error = DropdownError::DataDirectory(SharedError::HomeDirectoryNotFound);

        let printed = format!("{:#}", anyhow::Error::from(error));

        assert_eq!(
            printed,
            "could not determine the home directory (set HOME or TABSMITH_HOME)"
        );
    }
}
