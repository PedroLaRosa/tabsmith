use crate::modules::ghost_text::application::ghost_text_sources::GhostTextSources;
use crate::modules::ghost_text::domain::ghost_text_error::GhostTextError;
use crate::modules::ghost_text::domain::services::ghost_text_source_resolver::{profiles, selected_source};
use crate::modules::settings::domain::repositories::settings_repository::SettingsRepository;
use std::rc::Rc;

/// `tabsmith inline show-customizations`: the selected source and the configured profiles.
pub(crate) struct ListGhostTextSourcesUseCase {
    settings: Rc<dyn SettingsRepository>,
}

impl ListGhostTextSourcesUseCase {
    pub(crate) fn new(settings: Rc<dyn SettingsRepository>) -> Self {
        ListGhostTextSourcesUseCase { settings }
    }

    pub(crate) fn execute(&self) -> Result<GhostTextSources, GhostTextError> {
        let settings = self.settings.load()?;
        Ok(GhostTextSources {
            selected: selected_source(&settings).to_string(),
            profiles: profiles(&settings),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::ghost_text::domain::value_objects::llm_profile::{LlmProfile, LlmProfileParts};
    use crate::modules::ghost_text::domain::value_objects::llm_provider::LlmProvider;
    use crate::modules::settings::domain::repositories::settings_repository::InMemorySettingsRepository;
    use crate::modules::settings::domain::value_objects::settings::Settings;
    use crate::modules::settings::infrastructure::adapters::setting_value_json::settings_from_json;
    use serde_json::{json, Value};

    fn settings(value: Value) -> Settings {
        settings_from_json(value.as_object().unwrap().clone())
    }

    #[test]
    fn should_list_the_selected_source_and_each_profile_or_its_reason() {
        let repository = Rc::new(InMemorySettingsRepository::default());
        let saved = settings(json!({
            "inline.source": "b",
            "inline.profiles.a": {"provider": "openai", "model": "gpt"},
            "inline.profiles.c": {"provider": "gemini", "model": "x"},
        }));
        repository.save(&saved).unwrap();
        let use_case = ListGhostTextSourcesUseCase::new(repository);

        let sources = use_case.execute().unwrap();

        let a = LlmProfile::new(LlmProfileParts {
            name: "a".to_string(),
            provider: LlmProvider::OpenAi,
            model: "gpt".to_string(),
            base_url: "https://api.openai.com/v1".to_string(),
            api_key_env: "OPENAI_API_KEY".to_string(),
            max_tokens: 64,
            effort: None,
        });
        assert_eq!(
            (sources.selected, sources.profiles),
            (
                "b".to_string(),
                vec![
                    ("a".to_string(), Ok(a)),
                    (
                        "c".to_string(),
                        Err("unknown provider 'gemini' (expected openai or anthropic)".to_string())
                    )
                ]
            )
        );
    }
}
