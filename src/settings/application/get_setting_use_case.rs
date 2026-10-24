use crate::settings::domain::repositories::settings_repository::SettingsRepository;
use crate::settings::domain::settings_error::SettingsError;
use crate::settings::domain::value_objects::setting_value::SettingValue;
use std::rc::Rc;

/// `tabsmith settings KEY`: the value the user saved for the key.
pub(crate) struct GetSettingUseCase {
    settings: Rc<dyn SettingsRepository>,
}

impl GetSettingUseCase {
    pub(crate) fn new(settings: Rc<dyn SettingsRepository>) -> Self {
        GetSettingUseCase { settings }
    }

    pub(crate) fn execute(&self, key: &str) -> Result<Option<SettingValue>, SettingsError> {
        Ok(self.settings.load()?.get(key).cloned())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::domain::repositories::settings_repository::InMemorySettingsRepository;
    use crate::settings::domain::value_objects::setting_value::SettingValue;
    use crate::settings::domain::value_objects::settings::Settings;
    use std::rc::Rc;

    #[test]
    fn should_give_the_value_the_user_set_for_the_key() {
        let repository = Rc::new(InMemorySettingsRepository::default());
        repository
            .save(&Settings::default().with(
                "autocomplete.theme",
                SettingValue::Text("light".to_string()),
            ))
            .unwrap();
        let use_case = GetSettingUseCase::new(repository);

        let value = use_case.execute("autocomplete.theme").unwrap();

        assert_eq!(value, Some(SettingValue::Text("light".to_string())));
    }

    /// `settings autocomplete.theme` on fresh settings errors instead of printing the catalog's "dark".
    #[test]
    fn should_give_no_value_for_an_unset_key_even_when_the_catalog_has_a_default() {
        let use_case = GetSettingUseCase::new(Rc::new(InMemorySettingsRepository::default()));

        let value = use_case.execute("autocomplete.theme").unwrap();

        assert_eq!(value, None);
    }
}
