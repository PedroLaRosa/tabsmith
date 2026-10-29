use crate::terminal_session::domain::terminal_session_error::TerminalSessionError;

/// The id a wrapped shell's session goes by: the `TABSMITH_SESSION_ID` the wrapper exports and the name of the
/// session's run file.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(crate) struct SessionId(String);

impl SessionId {
    pub(crate) fn new(id: String) -> Result<SessionId, TerminalSessionError> {
        Ok(SessionId(id))
    }

    pub(crate) fn as_str(&self) -> &str {
        &self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// What the wrapper exports: 32 lowercase hex digits.
    #[test]
    fn should_accept_the_hex_id_the_wrapper_generates() {
        let id = "0123456789abcdef0123456789abcdef".to_string();

        let session_id = SessionId::new(id.clone()).unwrap();

        assert_eq!(session_id.as_str(), id);
    }
}
