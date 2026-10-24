use crate::settings::domain::settings_error::SettingsError;
use crate::settings::domain::value_objects::settings::Settings;
#[cfg(test)]
use std::cell::RefCell;

/// Where the user's settings are kept.
pub(crate) trait SettingsRepository {
    /// The saved settings; none saved yet is empty settings.
    fn load(&self) -> Result<Settings, SettingsError>;
    fn save(&self, settings: &Settings) -> Result<(), SettingsError>;
}

/// Settings kept in memory, for tests.
#[cfg(test)]
#[derive(Default)]
pub(crate) struct InMemorySettingsRepository {
    stored: RefCell<Settings>,
}

#[cfg(test)]
impl SettingsRepository for InMemorySettingsRepository {
    fn load(&self) -> Result<Settings, SettingsError> {
        Ok(self.stored.borrow().clone())
    }

    fn save(&self, settings: &Settings) -> Result<(), SettingsError> {
        *self.stored.borrow_mut() = settings.clone();
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::domain::value_objects::setting_value::SettingValue;

    #[test]
    fn should_load_no_settings_before_any_were_saved() {
        let repository = InMemorySettingsRepository::default();

        let settings = repository.load().unwrap();

        assert_eq!(settings.entries().count(), 0);
    }

    #[test]
    fn should_load_the_settings_it_last_saved() {
        let repository = InMemorySettingsRepository::default();
        let saved = Settings::default().with("autocomplete.theme", SettingValue::Text("light".to_string()));

        repository.save(&saved).unwrap();

        assert_eq!(repository.load().unwrap(), saved);
    }
}
