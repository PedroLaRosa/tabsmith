use crate::terminal_session::domain::entities::session_context::SessionContext;
use crate::terminal_session::domain::terminal_session_error::TerminalSessionError;
use crate::terminal_session::domain::value_objects::session_id::SessionId;
use std::time::{Duration, SystemTime};

/// Where each session's latest context is kept, between the `_ pre-cmd` that reports it and the wrapper that reads
/// it.
pub(crate) trait SessionContextRepository {
    /// Keeps the context as its session's latest one.
    fn save(&self, context: &SessionContext) -> Result<(), TerminalSessionError>;
    /// The session's latest context; `None` when there is none or it cannot be read.
    fn find(&self, id: &SessionId) -> Option<SessionContext>;
    /// When the session's context last changed; `None` when there is none, so a removed context is no change.
    fn last_modified(&self, id: &SessionId) -> Option<SystemTime>;
    /// Best effort: forgets the session's context.
    fn remove(&self, id: &SessionId);
    /// Best effort: forgets every context left unchanged for longer than `max_age`.
    fn prune_older_than(&self, max_age: Duration);
}

/// Session contexts kept in memory, for tests.
#[cfg(test)]
#[derive(Default)]
pub(crate) struct InMemorySessionContextRepository;

#[cfg(test)]
impl SessionContextRepository for InMemorySessionContextRepository {
    fn save(&self, _context: &SessionContext) -> Result<(), TerminalSessionError> {
        Ok(())
    }

    fn find(&self, _id: &SessionId) -> Option<SessionContext> {
        None
    }

    fn last_modified(&self, _id: &SessionId) -> Option<SystemTime> {
        None
    }

    fn remove(&self, _id: &SessionId) {}

    fn prune_older_than(&self, _max_age: Duration) {}
}

#[cfg(test)]
mod tests {
    use super::*;

    fn session_id(id: &str) -> SessionId {
        SessionId::new(id.to_string()).unwrap()
    }

    #[test]
    fn should_find_no_context_for_a_session_that_never_reported() {
        let repository = InMemorySessionContextRepository::default();

        let found = repository.find(&session_id("abc123"));

        assert!(found.is_none());
    }
}
