use std::fmt;

/// Why a session context was not kept: the session id is not one, or the context could not be saved.
#[derive(Debug)]
pub(crate) enum TerminalSessionError {
    /// Not a session id: empty, or holding a byte other than an ASCII letter, digit, `-` or `_`.
    InvalidSessionId(String),
    /// The session context could not be located or written. Shown as the storage's own error.
    Storage(Box<dyn std::error::Error + Send + Sync>),
}

impl fmt::Display for TerminalSessionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TerminalSessionError::InvalidSessionId(id) => write!(f, "not a session id: {id}"),
            TerminalSessionError::Storage(error) => write!(f, "{error}"),
        }
    }
}

impl std::error::Error for TerminalSessionError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            TerminalSessionError::InvalidSessionId(_) => None,
            // Display already is the storage error: continue with its own cause so nothing prints twice.
            TerminalSessionError::Storage(error) => error.source(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::error::Error;
    use std::fmt;

    #[derive(Debug)]
    struct RenameFailure(std::io::Error);

    impl fmt::Display for RenameFailure {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.write_str("writing /data/run/abc123.json")
        }
    }

    impl Error for RenameFailure {
        fn source(&self) -> Option<&(dyn Error + 'static)> {
            Some(&self.0)
        }
    }

    fn is_a_directory() -> std::io::Error {
        std::io::Error::other("Is a directory (os error 21)")
    }

    #[test]
    fn should_describe_a_failed_save_as_the_storage_error_itself() {
        let error = TerminalSessionError::Storage(Box::new(RenameFailure(is_a_directory())));

        let text = error.to_string();

        assert_eq!(text, "writing /data/run/abc123.json");
    }

    /// `{:#}` prints each cause once: "writing <path>: Is a directory (os error 21)", as `_ pre-cmd` always did.
    #[test]
    fn should_continue_with_the_causes_of_the_failed_save() {
        let error = TerminalSessionError::Storage(Box::new(RenameFailure(is_a_directory())));

        let cause = error.source().map(ToString::to_string);

        assert_eq!(cause.as_deref(), Some("Is a directory (os error 21)"));
    }

    /// Never printed: `_ pre-cmd` exits 1 silently on an id that is not one.
    #[test]
    fn should_name_the_id_that_is_not_a_session_id() {
        let error = TerminalSessionError::InvalidSessionId("a/b".to_string());

        let text = error.to_string();

        assert_eq!(text, "not a session id: a/b");
    }

    #[test]
    fn should_have_no_cause_for_an_id_that_is_not_a_session_id() {
        let error = TerminalSessionError::InvalidSessionId("a/b".to_string());

        let cause = error.source();

        assert!(cause.is_none());
    }
}
