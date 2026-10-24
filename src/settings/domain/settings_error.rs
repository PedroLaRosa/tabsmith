use std::fmt;

/// Why the settings could not be loaded or saved.
#[derive(Debug)]
pub(crate) enum SettingsError {
    /// The settings file could not be located, read, parsed or written. Shown as the storage's own error.
    Storage(Box<dyn std::error::Error + Send + Sync>),
    /// `settings --delete PATTERN` found no key matching the pattern.
    NoMatchingSettings(String),
}

impl fmt::Display for SettingsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SettingsError::Storage(error) => write!(f, "{error}"),
            SettingsError::NoMatchingSettings(pattern) => {
                write!(f, "No settings found matching {pattern}")
            }
        }
    }
}

impl std::error::Error for SettingsError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            // Display already is the storage error: continue with its own cause so nothing prints twice.
            SettingsError::Storage(error) => error.source(),
            SettingsError::NoMatchingSettings(_) => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::error::Error;
    use std::fmt;

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

    #[test]
    fn should_describe_a_storage_failure_as_the_storage_error_itself() {
        let error = SettingsError::Storage(Box::new(CorruptedFile(ParserCause)));

        let text = error.to_string();

        assert_eq!(text, "/data/settings.json is corrupted");
    }

    /// `{:#}` prints each cause once: "<path> is corrupted: expected value at line 1 column 1".
    #[test]
    fn should_continue_with_the_causes_of_the_storage_error() {
        let error = SettingsError::Storage(Box::new(CorruptedFile(ParserCause)));

        let cause = error.source().map(ToString::to_string);

        assert_eq!(cause.as_deref(), Some("expected value at line 1 column 1"));
    }

    /// `tabsmith settings -d nope.key` prints `error: No settings found matching nope.key`.
    #[test]
    fn should_name_the_pattern_that_matched_no_setting() {
        let error = SettingsError::NoMatchingSettings("nope.key".to_string());

        let text = error.to_string();

        assert_eq!(text, "No settings found matching nope.key");
    }
}
