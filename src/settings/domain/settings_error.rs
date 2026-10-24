use std::fmt;

/// Why the settings could not be loaded or saved.
#[derive(Debug)]
pub(crate) enum SettingsError {
    /// The settings file could not be located, read, parsed or written. Shown as the storage's own error.
    Storage(Box<dyn std::error::Error + Send + Sync>),
}

impl fmt::Display for SettingsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SettingsError::Storage(error) => write!(f, "{error}"),
        }
    }
}

impl std::error::Error for SettingsError {}

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
}
