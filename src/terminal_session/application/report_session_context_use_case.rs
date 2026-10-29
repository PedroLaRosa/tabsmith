use crate::shared::application::ports::clock::Clock;
use crate::terminal_session::domain::entities::session_context::SessionContext;
use crate::terminal_session::domain::repositories::session_context_repository::SessionContextRepository;
use crate::terminal_session::domain::terminal_session_error::TerminalSessionError;
use crate::terminal_session::domain::value_objects::session_id::SessionId;
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::Arc;
use std::time::Duration;

/// Delete run files untouched for a day. A shell that exits right after a prompt leaves one behind: its background
/// `_ pre-cmd` writes the file again after the wrapper removed it. A live session rewrites its file at every prompt.
const SESSION_CONTEXT_MAX_AGE: Duration = Duration::from_secs(24 * 3600);

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
        self.session_contexts.save(&context)?;
        self.session_contexts
            .prune_older_than(SESSION_CONTEXT_MAX_AGE);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shared::application::ports::clock::FixedClock;
    use crate::terminal_session::domain::repositories::session_context_repository::{
        InMemorySessionContextRepository, UnwritableSessionContextRepository,
    };
    use std::time::SystemTime;

    fn session_id(id: &str) -> SessionId {
        SessionId::new(id.to_string()).unwrap()
    }

    fn empty_context(id: &str) -> SessionContext {
        SessionContext::new(session_id(id), HashMap::new(), None, 0)
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

    /// A minute either side of a day, so the test pins the day and not the time it takes to run.
    #[test]
    fn should_prune_the_contexts_left_unchanged_for_more_than_a_day_once_saved() {
        let session_contexts = Rc::new(InMemorySessionContextRepository::default());
        let day = Duration::from_secs(24 * 3600);
        let minute = Duration::from_secs(60);
        let now = SystemTime::now();
        session_contexts.save_modified_at(&empty_context("stale"), now - day - minute);
        session_contexts.save_modified_at(&empty_context("recent"), now - day + minute);
        let use_case = ReportSessionContextUseCase::new(
            session_contexts.clone(),
            Arc::new(FixedClock::new(0)),
        );

        use_case
            .execute(session_id("abc123"), HashMap::new(), None)
            .unwrap();

        assert_eq!(
            ["stale", "recent", "abc123"]
                .map(|id| session_contexts.find(&session_id(id)).is_some()),
            [false, true, true]
        );
    }

    /// A purged data directory: the run file cannot be written, so nothing is pruned either (T43).
    #[test]
    fn should_give_back_the_failed_save_without_pruning() {
        let session_contexts = Rc::new(UnwritableSessionContextRepository::default());
        let use_case = ReportSessionContextUseCase::new(
            session_contexts.clone(),
            Arc::new(FixedClock::new(0)),
        );

        let result = use_case.execute(session_id("abc123"), HashMap::new(), None);

        assert_eq!(
            (
                result.map_err(|error| error.to_string()),
                session_contexts.prune_ran()
            ),
            (
                Err("No such file or directory (os error 2)".to_string()),
                false
            )
        );
    }
}
