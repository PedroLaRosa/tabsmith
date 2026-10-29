use crate::terminal_session::domain::entities::session_context::SessionContext;
use crate::terminal_session::domain::terminal_session_error::TerminalSessionError;
use crate::terminal_session::domain::value_objects::session_id::SessionId;
#[cfg(test)]
use std::cell::RefCell;
#[cfg(test)]
use std::collections::HashMap;
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
pub(crate) struct InMemorySessionContextRepository {
    contexts: RefCell<HashMap<SessionId, (SessionContext, SystemTime)>>,
}

#[cfg(test)]
impl SessionContextRepository for InMemorySessionContextRepository {
    fn save(&self, context: &SessionContext) -> Result<(), TerminalSessionError> {
        self.contexts
            .borrow_mut()
            .insert(context.id().clone(), (context.clone(), SystemTime::now()));
        Ok(())
    }

    fn find(&self, id: &SessionId) -> Option<SessionContext> {
        let contexts = self.contexts.borrow();
        let (context, _) = contexts.get(id)?;
        Some(context.clone())
    }

    fn last_modified(&self, id: &SessionId) -> Option<SystemTime> {
        let contexts = self.contexts.borrow();
        let (_, modified) = contexts.get(id)?;
        Some(*modified)
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

    fn empty_context(id: &str) -> SessionContext {
        SessionContext::new(session_id(id), HashMap::new(), None, 0)
    }

    #[test]
    fn should_find_no_context_for_a_session_that_never_reported() {
        let repository = InMemorySessionContextRepository::default();

        let found = repository.find(&session_id("abc123"));

        assert!(found.is_none());
    }

    #[test]
    fn should_find_the_context_the_session_last_reported() {
        let repository = InMemorySessionContextRepository::default();
        let first = empty_context("abc123");
        let last = SessionContext::new(
            session_id("abc123"),
            HashMap::from([("A".to_string(), "1".to_string())]),
            Some("ll='ls -l'".to_string()),
            2,
        );
        repository.save(&first).unwrap();

        repository.save(&last).unwrap();

        let found = repository.find(&session_id("abc123")).unwrap();
        assert_eq!(
            (found.env(), found.alias(), found.reported_at_ms()),
            (last.env(), last.alias(), 2)
        );
    }

    /// The pty reloads the context when this time changes.
    #[test]
    fn should_stamp_the_time_a_context_was_saved() {
        let repository = InMemorySessionContextRepository::default();
        let before = SystemTime::now();

        repository.save(&empty_context("abc123")).unwrap();

        let modified = repository.last_modified(&session_id("abc123")).unwrap();
        assert!(modified >= before && modified <= SystemTime::now());
    }
}
