use crate::settings::domain::repositories::settings_repository::SettingsRepository;
use crate::settings::domain::value_objects::settings::Settings;

/// The saved settings, or the defaults when they cannot be loaded (a file mid-edit, no home directory): the pty and
/// doctor keep working where the settings commands refuse.
pub(crate) fn load_or_defaults(repository: &dyn SettingsRepository) -> Settings {
    repository.load().unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::domain::repositories::settings_repository::InMemorySettingsRepository;
    use crate::settings::domain::value_objects::setting_value::SettingValue;

    #[test]
    fn should_give_the_settings_the_repository_loads() {
        let repository = InMemorySettingsRepository::default();
        let saved = Settings::default().with("inline.enabled", SettingValue::Bool(false));
        repository.save(&saved).unwrap();

        let settings = load_or_defaults(&repository);

        assert_eq!(settings, saved);
    }
}
