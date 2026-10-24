use crate::settings::domain::repositories::settings_repository::SettingsRepository;
use crate::settings::domain::settings_error::SettingsError;
use crate::settings::domain::value_objects::setting_value::SettingValue;
use std::rc::Rc;

/// `tabsmith settings KEY VALUE`: saves the value under the key, keeping every other setting.
pub(crate) struct SetSettingUseCase {
    settings: Rc<dyn SettingsRepository>,
}

impl SetSettingUseCase {
    pub(crate) fn new(settings: Rc<dyn SettingsRepository>) -> Self {
        SetSettingUseCase { settings }
    }

    pub(crate) fn execute(&self, key: &str, value: SettingValue) -> Result<(), SettingsError> {
        let updated = self.settings.load()?.with(key, value);
        self.settings.save(&updated)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::domain::repositories::settings_repository::InMemorySettingsRepository;
    use crate::settings::domain::value_objects::setting_number::SettingNumber;

    #[test]
    fn should_store_the_value_under_the_given_key() {
        let repository = Rc::new(InMemorySettingsRepository::default());
        let use_case = SetSettingUseCase::new(repository.clone());

        use_case
            .execute(
                "autocomplete.height",
                SettingValue::Number(SettingNumber::from(10u64)),
            )
            .unwrap();

        assert_eq!(
            repository.load().unwrap().get("autocomplete.height"),
            Some(&SettingValue::Number(SettingNumber::from(10u64)))
        );
    }
}
