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
    use crate::settings::domain::repositories::settings_repository::{
        InMemorySettingsRepository, UnreadableSettingsRepository,
    };
    use crate::settings::domain::value_objects::setting_number::SettingNumber;
    use crate::settings::domain::value_objects::settings::Settings;

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

    /// `tabsmith settings "" v` stores the key "": keys are not validated (D1).
    #[test]
    fn should_store_the_value_under_the_empty_key() {
        let repository = Rc::new(InMemorySettingsRepository::default());
        let use_case = SetSettingUseCase::new(repository.clone());

        use_case
            .execute("", SettingValue::Text("v".to_string()))
            .unwrap();

        assert_eq!(
            repository.load().unwrap().get(""),
            Some(&SettingValue::Text("v".to_string()))
        );
    }

    #[test]
    fn should_keep_every_other_setting() {
        let repository = Rc::new(InMemorySettingsRepository::default());
        let saved = Settings::default().with("inline.enabled", SettingValue::Bool(false));
        repository.save(&saved).unwrap();
        let use_case = SetSettingUseCase::new(repository.clone());

        use_case
            .execute(
                "autocomplete.theme",
                SettingValue::Text("light".to_string()),
            )
            .unwrap();

        assert_eq!(
            repository.load().unwrap().get("inline.enabled"),
            Some(&SettingValue::Bool(false))
        );
    }

    /// `settings KEY VALUE` on a corrupted settings.json refuses instead of overwriting it with `{KEY: VALUE}`.
    #[test]
    fn should_not_save_over_settings_that_cannot_be_loaded() {
        let repository = Rc::new(UnreadableSettingsRepository::default());
        let use_case = SetSettingUseCase::new(repository.clone());

        let result = use_case.execute("autocomplete.height", SettingValue::Null);

        assert_eq!(
            (result.is_err(), repository.save_attempted()),
            (true, false)
        );
    }
}
