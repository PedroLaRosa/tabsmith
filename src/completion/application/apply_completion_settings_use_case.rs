use crate::completion::application::command_line_parser::CommandLineParser;
use crate::completion::domain::entities::completion_session::CompletionSession;
use crate::settings::domain::value_objects::settings::Settings;
use crate::specs::application::ports::spec_runtime::SpecRuntime;
use std::rc::Rc;

/// The settings were saved: the spec runtime and the session take them.
pub(crate) struct ApplyCompletionSettingsUseCase {
    runtime: Rc<dyn SpecRuntime>,
    parser: Rc<CommandLineParser>,
}

impl ApplyCompletionSettingsUseCase {
    pub(crate) fn new(runtime: Rc<dyn SpecRuntime>, parser: Rc<CommandLineParser>) -> Self {
        ApplyCompletionSettingsUseCase { runtime, parser }
    }

    pub(crate) fn execute(&self, session: &mut CompletionSession, settings: Settings) {
        self.runtime.apply_settings(&settings);
        session.replace_settings(settings);
        self.parser.clear_caches(session.parse_prefix_cache());
        session.reset_generator_slots();
        session.forget_first_token_suggestions();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::completion::domain::entities::suggestion_recency::SuggestionRecency;
    use crate::completion::domain::test_fixtures::{arg, served_request};
    use crate::completion::domain::value_objects::generator_key::GeneratorKey;
    use crate::completion::domain::value_objects::suggestion_candidate::SuggestionCandidate;
    use crate::settings::domain::services::settings_catalog::INLINE_ENABLED;
    use crate::settings::domain::value_objects::setting_value::SettingValue;
    use crate::specs::application::ports::spec_runtime::InMemorySpecRuntime;
    use crate::specs::domain::repositories::spec_repository::InMemorySpecRepository;
    use crate::specs::domain::value_objects::suggestion_kind::SuggestionKind;
    use std::cell::Cell;
    use std::collections::HashMap;
    use std::time::Instant;

    fn session() -> CompletionSession {
        CompletionSession::new(Settings::default(), HashMap::new(), SuggestionRecency::default())
    }

    fn use_case_over(
        runtime: &Rc<InMemorySpecRuntime>,
        specs: &Rc<InMemorySpecRepository>,
    ) -> ApplyCompletionSettingsUseCase {
        let parser = CommandLineParser::new(specs.clone(), runtime.clone());
        ApplyCompletionSettingsUseCase::new(runtime.clone(), Rc::new(parser))
    }

    #[test]
    fn should_hand_the_new_settings_to_the_spec_runtime() {
        let runtime = Rc::new(InMemorySpecRuntime::default());
        let use_case = use_case_over(&runtime, &Rc::new(InMemorySpecRepository::default()));
        let mut session = session();
        let saved = Settings::default().with(INLINE_ENABLED, SettingValue::Bool(false));

        use_case.execute(&mut session, saved.clone());

        assert_eq!(runtime.applied_settings(), vec![saved]);
    }

    #[test]
    fn should_forget_the_loaded_specs_when_settings_change() {
        let specs = Rc::new(InMemorySpecRepository::default());
        let use_case = use_case_over(&Rc::new(InMemorySpecRuntime::default()), &specs);
        let mut session = session();

        use_case.execute(&mut session, Settings::default());

        assert_eq!(specs.forgets(), 1);
    }

    #[test]
    fn should_drop_first_token_items_and_generator_results_when_settings_change() {
        let use_case = use_case_over(&Rc::new(InMemorySpecRuntime::default()), &Rc::new(InMemorySpecRepository::default()));
        let mut session = session();
        session.first_token_suggestions(&Instant::now, || vec![SuggestionCandidate::new("git", SuggestionKind::Special)]);
        let argument = arg(false);
        session.assign_generator_slots(argument.clone(), vec![GeneratorKey::for_arg(&argument, 0)]);
        session.remember_search_term("src".into());

        use_case.execute(&mut session, Settings::default());

        let listed_again = Cell::new(false);
        session.first_token_suggestions(&Instant::now, || {
            listed_again.set(true);
            Vec::new()
        });
        assert_eq!(
            (
                listed_again.get(),
                session.slot_count(),
                session.slot_argument().is_none(),
                session.previous_search_term(),
            ),
            (true, 0, true, "")
        );
    }

    #[test]
    fn should_keep_the_fuzzy_toggle_and_the_served_request_when_settings_change() {
        let use_case = use_case_over(&Rc::new(InMemorySpecRuntime::default()), &Rc::new(InMemorySpecRepository::default()));
        let mut session = session();
        session.enter_argument(1);
        session.toggle_fuzzy_search();
        session.serve(served_request());

        use_case.execute(&mut session, Settings::default());

        assert_eq!(
            (session.user_fuzzy(), session.served().map(|served| served.request_id())),
            (true, Some(1))
        );
    }
}
