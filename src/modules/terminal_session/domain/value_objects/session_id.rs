use crate::modules::terminal_session::domain::terminal_session_error::TerminalSessionError;

/// The id a wrapped shell's session goes by: the `TABSMITH_SESSION_ID` the wrapper exports and the name of the
/// session's run file.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(crate) struct SessionId(String);

impl SessionId {
    /// Non-empty and only ASCII letters, digits, `-` and `_`, so it can never name a path. Not narrowed to the hex
    /// ids the wrapper generates: `_ pre-cmd` has always accepted any such id.
    pub(crate) fn new(id: String) -> Result<SessionId, TerminalSessionError> {
        let valid = !id.is_empty()
            && id
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_');
        if !valid {
            return Err(TerminalSessionError::InvalidSessionId(id));
        }
        Ok(SessionId(id))
    }

    pub(crate) fn as_str(&self) -> &str {
        &self.0
    }

    /// The id the wrapper gives a session it starts `nanos` after the Unix epoch in process `pid`: 32 lowercase hex
    /// digits, `{nanos:016x}{pid ^ nanos.rotate_left(17):016x}`, as the shell scripts echo it in `NewCmd={id}`. The
    /// caller reads the clock and the process id; this only formats them.
    pub(crate) fn from_entropy(nanos: u64, pid: u32) -> SessionId {
        SessionId(format!(
            "{nanos:016x}{:016x}",
            pid as u64 ^ nanos.rotate_left(17)
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn should_format_the_nanoseconds_and_the_mixed_pid_as_two_zero_padded_hex_words() {
        let id = SessionId::from_entropy(0x0123_4567_89ab_cdef, 0x2a);

        assert_eq!(id.as_str(), "0123456789abcdef8acf13579bde026c");
    }

    #[test]
    fn should_generate_an_id_the_validation_accepts() {
        let id = SessionId::from_entropy(u64::MAX, u32::MAX);

        assert_eq!(SessionId::new(id.as_str().to_string()).ok(), Some(id));
    }

    /// What the wrapper exports: 32 lowercase hex digits.
    #[test]
    fn should_accept_the_hex_id_the_wrapper_generates() {
        let id = "0123456789abcdef0123456789abcdef".to_string();

        let session_id = SessionId::new(id.clone()).unwrap();

        assert_eq!(session_id.as_str(), id);
    }

    /// Not only hex: `_ pre-cmd` has always taken upper case, `-` and `_` too.
    #[test]
    fn should_accept_any_id_of_ascii_letters_digits_dashes_and_underscores() {
        let id = "Ab-_9".to_string();

        let session_id = SessionId::new(id.clone()).unwrap();

        assert_eq!(session_id.as_str(), id);
    }

    /// The id becomes a file name, so nothing that could name a path, or no file at all, is one.
    #[test]
    fn should_reject_an_empty_id_or_one_with_any_other_byte() {
        let ids = ["", "a.b", "a/b", "..", "a b", "é", "abc\n"];

        let rejected = ids.map(|id| match SessionId::new(id.to_string()) {
            Err(TerminalSessionError::InvalidSessionId(given)) => given == id,
            _ => false,
        });

        assert_eq!(rejected, [true; 7]);
    }
}
