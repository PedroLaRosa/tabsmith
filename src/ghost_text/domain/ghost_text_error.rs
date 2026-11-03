use crate::settings::domain::settings_error::SettingsError;
use std::fmt;

/// Why an inline command failed: the settings could not be loaded or saved, the chosen customization does not
/// exist, none was chosen, or the prompt that asks for one failed.
#[derive(Debug)]
pub(crate) enum GhostTextError {
    /// The settings could not be loaded or saved. Shown as the settings error itself.
    Settings(SettingsError),
    /// The named customization is neither a built-in source nor a valid profile.
    CustomizationNotFound,
    /// The prompt ended without a customization: no terminal, or an answer outside the list.
    NoCustomizationSelected,
    /// The prompt could not be written or read. Shown as the io error itself.
    SourceChooser(std::io::Error),
}

impl fmt::Display for GhostTextError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            GhostTextError::Settings(error) => write!(f, "{error}"),
            GhostTextError::SourceChooser(error) => write!(f, "{error}"),
            GhostTextError::CustomizationNotFound => write!(f, "Customization not found"),
            GhostTextError::NoCustomizationSelected => write!(f, "No customization selected"),
        }
    }
}

impl std::error::Error for GhostTextError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            // Display already is the wrapped error: continue with its own cause so nothing prints twice.
            GhostTextError::Settings(error) => error.source(),
            GhostTextError::SourceChooser(error) => error.source(),
            GhostTextError::CustomizationNotFound => None,
            GhostTextError::NoCustomizationSelected => None,
        }
    }
}

impl From<SettingsError> for GhostTextError {
    fn from(error: SettingsError) -> Self {
        GhostTextError::Settings(error)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug)]
    struct ParserCause;

    impl fmt::Display for ParserCause {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.write_str("expected value at line 1 column 1")
        }
    }

    impl std::error::Error for ParserCause {}

    #[derive(Debug)]
    struct CorruptedFile(ParserCause);

    impl fmt::Display for CorruptedFile {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.write_str("/data/settings.json is corrupted")
        }
    }

    impl std::error::Error for CorruptedFile {
        fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
            Some(&self.0)
        }
    }

    fn corrupted_file() -> CorruptedFile {
        CorruptedFile(ParserCause)
    }

    #[test]
    fn should_print_a_settings_failure_exactly_as_the_settings_error() {
        let wrapped = GhostTextError::Settings(SettingsError::Storage(Box::new(corrupted_file())));
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
    fn should_print_a_chooser_failure_exactly_as_its_io_error() {
        let wrapped = GhostTextError::SourceChooser(std::io::Error::other(corrupted_file()));
        let bare = std::io::Error::other(corrupted_file());

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
    fn should_print_an_unknown_customization_as_the_cli_did() {
        let error = GhostTextError::CustomizationNotFound;

        let printed = format!("{:#}", anyhow::Error::from(error));

        assert_eq!(printed, "Customization not found");
    }
}
