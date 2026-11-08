use crate::modules::ghost_text::domain::ghost_text_error::GhostTextError;
use crate::modules::settings::domain::repositories::settings_repository::SettingsRepository;
use crate::modules::settings::domain::services::settings_catalog::INLINE_ENABLED;
use crate::modules::settings::domain::value_objects::setting_value::SettingValue;
use std::rc::Rc;

/// `tabsmith inline enable|disable`: saves the inline switch, keeping every other setting.
pub(crate) struct SetInlineEnabledUseCase {
    settings: Rc<dyn SettingsRepository>,
}

impl SetInlineEnabledUseCase {
    pub(crate) fn new(settings: Rc<dyn SettingsRepository>) -> Self {
        SetInlineEnabledUseCase { settings }
    }

    pub(crate) fn execute(&self, enabled: bool) -> Result<(), GhostTextError> {
        let settings = self.settings.load()?;
        self.settings
            .save(&settings.with(INLINE_ENABLED, SettingValue::Bool(enabled)))?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::settings::domain::repositories::settings_repository::{
        InMemorySettingsRepository, UnreadableSettingsRepository,
    };
    use crate::modules::settings::domain::value_objects::settings::Settings;

    #[test]
    fn should_save_the_inline_switch() {
        let repository = Rc::new(InMemorySettingsRepository::default());
        let saved = Settings::default().with(
            "autocomplete.theme",
            SettingValue::Text("light".to_string()),
        );
        repository.save(&saved).unwrap();
        let use_case = SetInlineEnabledUseCase::new(repository.clone());

        use_case.execute(false).unwrap();

        let settings = repository.load().unwrap();
        assert_eq!(
            (
                settings.get(INLINE_ENABLED),
                settings.get("autocomplete.theme")
            ),
            (
                Some(&SettingValue::Bool(false)),
                Some(&SettingValue::Text("light".to_string()))
            )
        );
    }

    #[test]
    fn should_not_save_the_inline_switch_over_settings_that_cannot_be_loaded() {
        let repository = Rc::new(UnreadableSettingsRepository::default());
        let use_case = SetInlineEnabledUseCase::new(repository.clone());

        let result = use_case.execute(false);

        assert_eq!(
            (
                matches!(result, Err(GhostTextError::Settings(_))),
                repository.save_attempted()
            ),
            (true, false)
        );
    }
}
