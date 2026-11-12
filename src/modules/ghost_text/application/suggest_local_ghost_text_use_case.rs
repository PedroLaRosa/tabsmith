use crate::modules::completion::domain::value_objects::completion_result::CompletionResult;
use crate::modules::ghost_text::domain::services::ghost_text_source_resolver::source;
use crate::modules::ghost_text::domain::services::history_ghost_text::history_ghost;
use crate::modules::ghost_text::domain::services::spec_ghost_text::spec_ghost;
use crate::modules::ghost_text::domain::value_objects::ghost_text_source::GhostTextSource;
use crate::modules::history::domain::repositories::history_repository::HistoryRepository;
use crate::modules::settings::domain::services::settings_catalog::INLINE_ENABLED;
use crate::modules::settings::domain::value_objects::settings::Settings;
use std::rc::Rc;

/// The local ghost text of one reply (SPEC section 6.11), also for error replies.
pub(crate) struct SuggestLocalGhostTextUseCase {
    history: Rc<dyn HistoryRepository>,
}

impl SuggestLocalGhostTextUseCase {
    pub(crate) fn new(history: Rc<dyn HistoryRepository>) -> Self {
        SuggestLocalGhostTextUseCase { history }
    }

    pub(crate) fn execute(&self, settings: &Settings, result: &CompletionResult) -> Option<String> {
        if !settings.bool(INLINE_ENABLED)
            || result.buffer().is_empty()
            || result.cursor() != result.buffer().len()
        {
            return None;
        }
        let history = || history_ghost(&self.history.recent(), result.buffer());
        let specs = || spec_ghost(result.suggestions());
        match source(settings) {
            Ok(GhostTextSource::Specs) => specs(),
            Ok(GhostTextSource::History) | Ok(GhostTextSource::Profile(_)) => history(),
            Ok(GhostTextSource::HistoryAndSpecs) | Err(_) => history().or_else(specs),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::completion::domain::value_objects::completion_phase::CompletionPhase;
    use crate::modules::completion::domain::value_objects::completion_result::CompletionResultParts;
    use crate::modules::completion::domain::value_objects::insertion::{Insertion, InsertionParts};
    use crate::modules::completion::domain::value_objects::suggestion::{Suggestion, SuggestionParts};
    use crate::modules::history::domain::entities::history_entry::{HistoryEntry, HistoryEntryPrimitives};
    use crate::modules::history::domain::repositories::history_repository::InMemoryHistoryRepository;
    use crate::modules::settings::infrastructure::adapters::setting_value_json::settings_from_json;
    use crate::modules::specs::domain::value_objects::suggestion_kind::SuggestionKind;
    use serde_json::json;

    fn entry(command: &str, exit_code: i32) -> HistoryEntry {
        HistoryEntry::new(HistoryEntryPrimitives {
            command: command.into(),
            shell: "zsh".into(),
            exit_code: Some(exit_code),
            ..Default::default()
        })
    }

    fn settings(value: serde_json::Value) -> Settings {
        settings_from_json(value.as_object().unwrap().clone())
    }

    fn suggestion(text: &str) -> Suggestion {
        Suggestion::new(SuggestionParts {
            names: vec!["n".into()],
            display_name: None,
            description: None,
            kind: SuggestionKind::Subcommand,
            original_kind: None,
            args_hint: String::new(),
            highlight: Vec::new(),
            query: String::new(),
            insert: Insertion::new(InsertionParts {
                delete: 0,
                text: text.into(),
                cursor_back: 0,
                execute: false,
            }),
            generator: None,
            prefix_match: true,
            icon: None,
        })
    }

    fn result(buffer: &str, cursor: usize, suggestions: Vec<Suggestion>) -> CompletionResult {
        CompletionResult::new(CompletionResultParts {
            req: 1,
            phase: CompletionPhase::Parse,
            error: false,
            buffer: buffer.into(),
            cursor,
            tokens: Vec::new(),
            command_text: String::new(),
            command_index: 0,
            arg_key: 0,
            current_arg: None,
            search_term: String::new(),
            triggered_generators: Vec::new(),
            items: suggestions,
            loading: false,
            ghost: None,
        })
    }

    fn use_case_over(history: Vec<HistoryEntry>) -> SuggestLocalGhostTextUseCase {
        SuggestLocalGhostTextUseCase::new(Rc::new(InMemoryHistoryRepository::new(history)))
    }

    #[test]
    fn should_suggest_the_rest_of_the_newest_successful_history_command() {
        let use_case = use_case_over(vec![entry("git status", 0), entry("git stash", 1)]);

        let ghost = use_case.execute(&Settings::default(), &result("git sta", 7, Vec::new()));

        assert_eq!(ghost.as_deref(), Some("tus"));
    }

    #[test]
    fn should_fall_back_to_the_first_spec_suggestion_when_no_history_command_extends_the_buffer() {
        let use_case = use_case_over(vec![entry("ls", 0)]);

        let ghost = use_case.execute(&Settings::default(), &result("git ch", 6, vec![suggestion("eckout ")]));

        assert_eq!(ghost.as_deref(), Some("eckout"));
    }

    #[test]
    fn should_suggest_only_specs_for_the_specs_source() {
        let use_case = use_case_over(vec![entry("git checkout main", 0)]);
        let specs_only = settings(json!({"inline.source": "specs"}));

        let ghost = use_case.execute(&specs_only, &result("git ch", 6, vec![suggestion("erry-pick ")]));

        assert_eq!(ghost.as_deref(), Some("erry-pick"));
    }

    #[test]
    fn should_suggest_only_history_for_an_llm_profile_source() {
        let use_case = use_case_over(vec![entry("ls", 0)]);
        let profile_source = settings(json!({
            "inline.source": "p",
            "inline.profiles.p": {"provider": "openai", "model": "m"}
        }));

        let ghost = use_case.execute(&profile_source, &result("git ch", 6, vec![suggestion("eckout ")]));

        assert_eq!(ghost, None);
    }

    #[test]
    fn should_fall_back_from_history_to_specs_for_a_source_that_names_no_valid_profile() {
        let use_case = use_case_over(vec![entry("ls", 0)]);
        let unknown_source = settings(json!({"inline.source": "nope"}));

        let ghost = use_case.execute(&unknown_source, &result("git ch", 6, vec![suggestion("eckout ")]));

        assert_eq!(ghost.as_deref(), Some("eckout"));
    }

    #[test]
    fn should_suggest_nothing_while_inline_is_disabled() {
        let use_case = use_case_over(vec![entry("git status", 0)]);
        let disabled = settings(json!({"inline.enabled": false}));

        let ghost = use_case.execute(&disabled, &result("git sta", 7, Vec::new()));

        assert_eq!(ghost, None);
    }

    #[test]
    fn should_suggest_nothing_when_the_cursor_is_not_at_the_end() {
        let use_case = use_case_over(vec![entry("git status", 0)]);

        let ghost = use_case.execute(&Settings::default(), &result("git sta", 3, Vec::new()));

        assert_eq!(ghost, None);
    }
}
