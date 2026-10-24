use crate::settings::domain::repositories::settings_repository::SettingsRepository;
use crate::settings::domain::settings_error::SettingsError;
use crate::settings::domain::value_objects::settings::Settings;
use std::rc::Rc;

/// `tabsmith settings list`: every setting the user saved, in key order.
pub(crate) struct ListSettingsUseCase {
    settings: Rc<dyn SettingsRepository>,
}

impl ListSettingsUseCase {
    pub(crate) fn new(settings: Rc<dyn SettingsRepository>) -> Self {
        ListSettingsUseCase { settings }
    }

    pub(crate) fn execute(&self) -> Result<Settings, SettingsError> {
        self.settings.load()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::domain::repositories::settings_repository::InMemorySettingsRepository;
    use crate::settings::domain::value_objects::setting_value::SettingValue;

    #[test]
    fn should_list_the_settings_the_user_saved() {
        let repository = Rc::new(InMemorySettingsRepository::default());
        let saved = Settings::default()
            .with("b.key", SettingValue::Text("x".to_string()))
            .with("a.key", SettingValue::Bool(true));
        repository.save(&saved).unwrap();
        let use_case = ListSettingsUseCase::new(repository);

        let listed = use_case.execute().unwrap();

        assert_eq!(listed, saved);
    }
}
