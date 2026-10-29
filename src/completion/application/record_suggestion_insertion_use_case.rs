use crate::completion::domain::entities::completion_session::CompletionSession;
use crate::completion::domain::repositories::recency_repository::RecencyRepository;
use crate::shared::application::ports::clock::Clock;
use std::rc::Rc;
use std::sync::Arc;

/// A suggestion was inserted: its time is recorded under the command and name, and the recency is saved. An
/// auto-execute row (a name holding `↪`) is neither recorded nor saved.
pub(crate) struct RecordSuggestionInsertionUseCase {
    recency: Rc<dyn RecencyRepository>,
    clock: Arc<dyn Clock + Send + Sync>,
}

impl RecordSuggestionInsertionUseCase {
    pub(crate) fn new(recency: Rc<dyn RecencyRepository>, clock: Arc<dyn Clock + Send + Sync>) -> Self {
        RecordSuggestionInsertionUseCase { recency, clock }
    }

    pub(crate) fn execute(&self, session: &mut CompletionSession, command: String, name: String) {
        if session.record_insertion(command, name, self.clock.now_ms()) {
            self.recency.save(session.recency());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::completion::domain::entities::suggestion_recency::SuggestionRecency;
    use crate::completion::domain::repositories::recency_repository::InMemoryRecencyRepository;
    use crate::settings::domain::value_objects::settings::Settings;
    use crate::shared::application::ports::clock::FixedClock;
    use std::collections::HashMap;

    const NOW_MS: u64 = 1_700_000_000_000;

    fn times(command: &str, name: &str, at_ms: u64) -> HashMap<String, HashMap<String, u64>> {
        [(command.to_string(), [(name.to_string(), at_ms)].into())].into()
    }

    #[test]
    fn should_record_when_a_suggestion_was_inserted_for_its_command() {
        let mut session = CompletionSession::new(Settings::default(), HashMap::new(), SuggestionRecency::default());
        let repository = Rc::new(InMemoryRecencyRepository::default());
        let use_case = RecordSuggestionInsertionUseCase::new(repository.clone(), Arc::new(FixedClock::new(NOW_MS)));

        use_case.execute(&mut session, "git".into(), "checkout".into());

        assert_eq!(
            (session.recency().to_primitives(), repository.load().to_primitives()),
            (times("git", "checkout", NOW_MS), times("git", "checkout", NOW_MS))
        );
    }

    /// The session already holds times the repository never saw, so a save would show in the repository.
    #[test]
    fn should_not_save_the_recency_after_an_auto_execute_row() {
        let recency = SuggestionRecency::new(times("git", "checkout", NOW_MS - 1));
        let mut session = CompletionSession::new(Settings::default(), HashMap::new(), recency);
        let repository = Rc::new(InMemoryRecencyRepository::default());
        let use_case = RecordSuggestionInsertionUseCase::new(repository.clone(), Arc::new(FixedClock::new(NOW_MS)));

        use_case.execute(&mut session, "git".into(), "↪ git status".into());

        assert_eq!(repository.load().of_command("git"), None);
    }
}
