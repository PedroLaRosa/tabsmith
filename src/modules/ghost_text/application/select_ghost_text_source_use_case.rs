use crate::modules::ghost_text::application::ports::source_chooser::SourceChooser;
use crate::modules::ghost_text::domain::ghost_text_error::GhostTextError;
use crate::modules::ghost_text::domain::services::ghost_text_source_resolver::profiles;
use crate::modules::ghost_text::domain::value_objects::ghost_text_source::BUILTIN_SOURCES;
use crate::modules::ghost_text::domain::value_objects::llm_profile::LlmProfile;
use crate::modules::settings::domain::repositories::settings_repository::SettingsRepository;
use crate::modules::settings::domain::services::settings_catalog::INLINE_SOURCE;
use crate::modules::settings::domain::value_objects::setting_value::SettingValue;
use std::rc::Rc;

/// `tabsmith inline set-customization [NAME]`: saves the source named or chosen, or unsets it when the user
/// chooses "None".
pub(crate) struct SelectGhostTextSourceUseCase {
    settings: Rc<dyn SettingsRepository>,
    chooser: Rc<dyn SourceChooser>,
}

impl SelectGhostTextSourceUseCase {
    pub(crate) fn new(
        settings: Rc<dyn SettingsRepository>,
        chooser: Rc<dyn SourceChooser>,
    ) -> Self {
        SelectGhostTextSourceUseCase { settings, chooser }
    }

    /// The selected source, or `None` when the source was unset. The settings are loaded once, before the
    /// chooser is asked, and saved over that snapshot.
    pub(crate) fn execute(&self, name: Option<String>) -> Result<Option<String>, GhostTextError> {
        let settings = self.settings.load()?;
        // Only valid profiles can be selected (an invalid one would only make doctor fail).
        let valid_profiles: Vec<(String, LlmProfile)> = profiles(&settings)
            .into_iter()
            .filter_map(|(profile_name, profile)| Some((profile_name, profile.ok()?)))
            .collect();
        let name = match name {
            Some(named) => named,
            None => match self.chooser.choose(&valid_profiles)? {
                Some(chosen) => chosen,
                None => {
                    self.settings.save(&settings.without(INLINE_SOURCE))?;
                    return Ok(None);
                }
            },
        };
        if !BUILTIN_SOURCES
            .iter()
            .any(|(builtin_name, _)| *builtin_name == name)
            && !valid_profiles
                .iter()
                .any(|(profile_name, _)| *profile_name == name)
        {
            return Err(GhostTextError::CustomizationNotFound);
        }
        self.settings
            .save(&settings.with(INLINE_SOURCE, SettingValue::Text(name.clone())))?;
        Ok(Some(name))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::ghost_text::application::ports::source_chooser::FixedSourceChooser;
    use crate::modules::settings::domain::repositories::settings_repository::{
        InMemorySettingsRepository, UnreadableSettingsRepository,
    };
    use crate::modules::settings::domain::value_objects::settings::Settings;
    use crate::modules::settings::infrastructure::adapters::setting_value_json::settings_from_json;
    use serde_json::{json, Value};

    fn settings(value: Value) -> Settings {
        settings_from_json(value.as_object().unwrap().clone())
    }

    fn repository_with_profiles() -> Rc<InMemorySettingsRepository> {
        let repository = Rc::new(InMemorySettingsRepository::default());
        let saved = settings(json!({
            "autocomplete.height": 10,
            "inline.source": "history",
            "inline.profiles.a": {"provider": "openai", "model": "gpt"},
            "inline.profiles.b": {"provider": "anthropic", "model": "claude"},
            "inline.profiles.c": {"provider": "gemini", "model": "x"},
        }));
        repository.save(&saved).unwrap();
        repository
    }

    fn select(
        repository: &Rc<InMemorySettingsRepository>,
        chooser: &Rc<FixedSourceChooser>,
        name: Option<&str>,
    ) -> Result<Option<String>, GhostTextError> {
        SelectGhostTextSourceUseCase::new(repository.clone(), chooser.clone())
            .execute(name.map(String::from))
    }

    fn stored(repository: &InMemorySettingsRepository, key: &str) -> Option<SettingValue> {
        repository.load().unwrap().get(key).cloned()
    }

    #[test]
    fn should_select_a_builtin_source_by_name() {
        let repository = repository_with_profiles();
        let chooser = Rc::new(FixedSourceChooser::choosing(None));

        let selected = select(&repository, &chooser, Some("specs")).unwrap();

        assert_eq!(
            (selected, stored(&repository, INLINE_SOURCE)),
            (
                Some("specs".to_string()),
                Some(SettingValue::Text("specs".to_string()))
            )
        );
    }

    #[test]
    fn should_select_a_valid_profile_by_name() {
        let repository = repository_with_profiles();
        let chooser = Rc::new(FixedSourceChooser::choosing(None));

        let selected = select(&repository, &chooser, Some("b")).unwrap();

        assert_eq!(
            (selected, stored(&repository, INLINE_SOURCE)),
            (
                Some("b".to_string()),
                Some(SettingValue::Text("b".to_string()))
            )
        );
    }

    #[test]
    fn should_refuse_a_source_that_is_neither_builtin_nor_a_valid_profile() {
        let repository = repository_with_profiles();
        let before = repository.load().unwrap();
        let chooser = Rc::new(FixedSourceChooser::choosing(None));

        let result = select(&repository, &chooser, Some("c"));

        assert_eq!(
            (
                matches!(result, Err(GhostTextError::CustomizationNotFound)),
                repository.load().unwrap() == before
            ),
            (true, true)
        );
    }

    #[test]
    fn should_unset_the_source_when_none_is_chosen() {
        let repository = repository_with_profiles();
        let chooser = Rc::new(FixedSourceChooser::choosing(None));

        let selected = select(&repository, &chooser, None).unwrap();

        assert_eq!(
            (
                selected,
                stored(&repository, INLINE_SOURCE),
                stored(&repository, "autocomplete.height").is_some()
            ),
            (None, None, true)
        );
    }

    #[test]
    fn should_offer_the_chooser_only_the_valid_profiles() {
        let repository = repository_with_profiles();
        let chooser = Rc::new(FixedSourceChooser::choosing(Some("a")));

        select(&repository, &chooser, None).unwrap();

        assert_eq!(chooser.offered(), ["a", "b"]);
    }

    #[test]
    fn should_not_save_a_source_over_settings_that_cannot_be_loaded() {
        let repository = Rc::new(UnreadableSettingsRepository::default());
        let chooser = Rc::new(FixedSourceChooser::choosing(None));
        let use_case = SelectGhostTextSourceUseCase::new(repository.clone(), chooser);

        let result = use_case.execute(Some("history".to_string()));

        assert_eq!(
            (
                matches!(result, Err(GhostTextError::Settings(_))),
                repository.save_attempted()
            ),
            (true, false)
        );
    }
}
