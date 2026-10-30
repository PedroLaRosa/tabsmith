use crate::ghost_text::domain::ghost_text_error::GhostTextError;
use crate::settings::domain::repositories::settings_repository::SettingsRepository;
use crate::settings::domain::services::settings_catalog::INLINE_ENABLED;
use std::rc::Rc;

/// `tabsmith inline status`: whether ghost text is switched on.
pub(crate) struct GetInlineStatusUseCase {
    settings: Rc<dyn SettingsRepository>,
}

impl GetInlineStatusUseCase {
    pub(crate) fn new(settings: Rc<dyn SettingsRepository>) -> Self {
        GetInlineStatusUseCase { settings }
    }

    /// Reads strictly (a corrupted settings file is an error) but leniently per key: an unset or non-boolean
    /// `inline.enabled` reads as the catalog default, enabled.
    pub(crate) fn execute(&self) -> Result<bool, GhostTextError> {
        Ok(self.settings.load()?.bool(INLINE_ENABLED))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::domain::repositories::settings_repository::InMemorySettingsRepository;
    use crate::settings::domain::value_objects::setting_value::SettingValue;
    use crate::settings::domain::value_objects::settings::Settings;

    #[test]
    fn should_read_inline_as_enabled_when_unset() {
        let repository = Rc::new(InMemorySettingsRepository::default());
        let use_case = GetInlineStatusUseCase::new(repository);

        let enabled = use_case.execute().unwrap();

        assert!(enabled);
    }

    #[test]
    fn should_read_a_non_boolean_inline_switch_as_enabled() {
        let repository = Rc::new(InMemorySettingsRepository::default());
        let saved =
            Settings::default().with(INLINE_ENABLED, SettingValue::Text("false".to_string()));
        repository.save(&saved).unwrap();
        let use_case = GetInlineStatusUseCase::new(repository);

        let enabled = use_case.execute().unwrap();

        assert!(enabled);
    }
}
