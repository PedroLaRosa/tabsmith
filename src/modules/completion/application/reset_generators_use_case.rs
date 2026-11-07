use crate::modules::completion::domain::entities::completion_session::CompletionSession;

/// The command ended: the generators of the current argument and the served request are forgotten. Running
/// generators and the generator cache stay; a run that finishes later settles into no slot.
pub(crate) struct ResetGeneratorsUseCase;

impl ResetGeneratorsUseCase {
    pub(crate) fn execute(&self, session: &mut CompletionSession) {
        session.reset_generator_slots();
        session.forget_served_request();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::completion::domain::entities::suggestion_recency::SuggestionRecency;
    use crate::modules::completion::domain::test_fixtures::{arg, served_request};
    use crate::modules::completion::domain::value_objects::generator_key::GeneratorKey;
    use crate::modules::settings::domain::value_objects::settings::Settings;
    use std::collections::HashMap;

    #[test]
    fn should_forget_the_served_request_and_the_generator_results() {
        let mut session = CompletionSession::new(Settings::default(), HashMap::new(), SuggestionRecency::default());
        let argument = arg(false);
        session.assign_generator_slots(argument.clone(), vec![GeneratorKey::for_arg(&argument, 0)]);
        session.remember_search_term("src".into());
        session.serve(served_request());

        ResetGeneratorsUseCase.execute(&mut session);

        assert_eq!(
            (
                session.served().is_none(),
                session.slot_count(),
                session.slot_argument().is_none(),
                session.previous_search_term(),
            ),
            (true, 0, true, "")
        );
    }
}
