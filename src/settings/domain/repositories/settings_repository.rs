use crate::settings::domain::settings_error::SettingsError;
use crate::settings::domain::value_objects::settings::Settings;
#[cfg(test)]
use std::cell::{Cell, RefCell};
use std::time::SystemTime;

/// Where the user's settings are kept.
pub(crate) trait SettingsRepository {
    /// The saved settings; none saved yet is empty settings.
    fn load(&self) -> Result<Settings, SettingsError>;
    fn save(&self, settings: &Settings) -> Result<(), SettingsError>;
    /// When the saved settings last changed; `None` while there are none, so removing them counts as a change.
    fn last_modified(&self) -> Option<SystemTime>;
}

/// Settings kept in memory, for tests.
#[cfg(test)]
#[derive(Default)]
pub(crate) struct InMemorySettingsRepository {
    stored: RefCell<Settings>,
    saved_at: Cell<Option<SystemTime>>,
}

#[cfg(test)]
impl SettingsRepository for InMemorySettingsRepository {
    fn load(&self) -> Result<Settings, SettingsError> {
        Ok(self.stored.borrow().clone())
    }

    fn save(&self, settings: &Settings) -> Result<(), SettingsError> {
        *self.stored.borrow_mut() = settings.clone();
        self.saved_at.set(Some(SystemTime::now()));
        Ok(())
    }

    fn last_modified(&self) -> Option<SystemTime> {
        self.saved_at.get()
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

    /// The pty compares this time to decide to reload; nothing saved yet reads as no time at all.
    #[test]
    fn should_have_no_modification_time_before_any_settings_were_saved() {
        let repository = InMemorySettingsRepository::default();

        let modified = repository.last_modified();

        assert_eq!(modified, None);
    }

    #[test]
    fn should_have_a_modification_time_once_settings_were_saved() {
        let repository = InMemorySettingsRepository::default();

        repository.save(&Settings::default()).unwrap();

        assert!(repository.last_modified().is_some());
    }
}
