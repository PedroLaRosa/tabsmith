use crate::shared::application::ports::clock::Clock;
use crate::terminal_session::domain::entities::session_context::SessionContext;
use crate::terminal_session::domain::repositories::session_context_repository::SessionContextRepository;
use crate::terminal_session::domain::terminal_session_error::TerminalSessionError;
use crate::terminal_session::domain::value_objects::session_id::SessionId;
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::Arc;

/// A shell reported its env and alias text at a prompt (`_ pre-cmd`, SPEC §2.11): it becomes its session's latest
/// context, stamped with the report time.
pub(crate) struct ReportSessionContextUseCase {
    session_contexts: Rc<dyn SessionContextRepository>,
    clock: Arc<dyn Clock + Send + Sync>,
}

impl ReportSessionContextUseCase {
    pub(crate) fn new(
        session_contexts: Rc<dyn SessionContextRepository>,
        clock: Arc<dyn Clock + Send + Sync>,
    ) -> Self {
        ReportSessionContextUseCase {
            session_contexts,
            clock,
        }
    }

    pub(crate) fn execute(
        &self,
        id: SessionId,
        env: HashMap<String, String>,
        alias: Option<String>,
    ) -> Result<(), TerminalSessionError> {
        let context = SessionContext::new(id, env, alias, self.clock.now_ms());
        self.session_contexts.save(&context)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shared::application::ports::clock::FixedClock;
    use crate::terminal_session::domain::repositories::session_context_repository::InMemorySessionContextRepository;

    fn session_id(id: &str) -> SessionId {
        SessionId::new(id.to_string()).unwrap()
    }

    #[test]
    fn should_save_the_reported_context_stamped_with_the_clock_time() {
        let session_contexts = Rc::new(InMemorySessionContextRepository::default());
        let use_case = ReportSessionContextUseCase::new(
            session_contexts.clone(),
            Arc::new(FixedClock::new(1_700_000_123_456)),
        );
        let env = HashMap::from([("A".to_string(), "1".to_string())]);

        use_case
            .execute(
                session_id("abc123"),
                env.clone(),
                Some("ll='ls -l'".to_string()),
            )
            .unwrap();

        let saved = session_contexts.find(&session_id("abc123")).unwrap();
        assert_eq!(
            (saved.env(), saved.alias(), saved.reported_at_ms()),
            (&env, Some("ll='ls -l'"), 1_700_000_123_456)
        );
    }
}
