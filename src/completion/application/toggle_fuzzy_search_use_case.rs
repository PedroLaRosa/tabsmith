use crate::completion::domain::entities::completion_session::CompletionSession;

/// The user toggled the fuzzy search: it flips until the current argument changes. Nothing is replied.
pub(crate) struct ToggleFuzzySearchUseCase;

impl ToggleFuzzySearchUseCase {
    pub(crate) fn execute(&self, session: &mut CompletionSession) {
        session.toggle_fuzzy_search();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::completion::domain::entities::suggestion_recency::SuggestionRecency;
    use crate::settings::domain::value_objects::settings::Settings;
    use std::collections::HashMap;

    #[test]
    fn should_flip_the_fuzzy_search_of_the_current_argument() {
        let mut session = CompletionSession::new(Settings::default(), HashMap::new(), SuggestionRecency::default());
        session.enter_argument(1);
        let before = session.user_fuzzy();

        ToggleFuzzySearchUseCase.execute(&mut session);

        assert_eq!((before, session.user_fuzzy()), (false, true));
    }
}
