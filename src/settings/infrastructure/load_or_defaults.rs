use crate::settings::domain::repositories::settings_repository::SettingsRepository;
use crate::settings::domain::settings_error::SettingsError;
use crate::settings::domain::value_objects::settings::Settings;
use crate::shared::infrastructure::logger;
use std::error::Error;

/// The saved settings, or the defaults when they cannot be loaded (a file mid-edit, no home directory): the pty and
/// doctor keep working where the settings commands refuse.
pub(crate) fn load_or_defaults(repository: &dyn SettingsRepository) -> Settings {
    repository.load().unwrap_or_else(|error| {
        logger::warn(&warning(&error));
        Settings::default()
    })
}

/// `settings: ` and the error with each cause after a colon, as anyhow's `{:#}` printed it.
fn warning(error: &SettingsError) -> String {
    let mut text = format!("settings: {error}");
    let mut cause = error.source();
    while let Some(error) = cause {
        text += &format!(": {error}");
        cause = error.source();
    }
    text
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::domain::repositories::settings_repository::InMemorySettingsRepository;
    use crate::settings::domain::value_objects::setting_value::SettingValue;
    use crate::settings::infrastructure::adapters::json_file_settings_repository::JsonFileSettingsRepository;
    use crate::shared::domain::value_objects::data_directory::DataDirectory;
    use crate::shared::infrastructure::testing::temp_dir::TempDir;

    fn corrupted_settings_file(temp_dir: &TempDir) -> JsonFileSettingsRepository {
        std::fs::write(temp_dir.path().join("settings.json"), "{oops").unwrap();
        JsonFileSettingsRepository::new(DataDirectory::new(
            Some(temp_dir.path().to_path_buf()),
            None,
        ))
    }

    #[test]
    fn should_give_the_settings_the_repository_loads() {
        let repository = InMemorySettingsRepository::default();
        let saved = Settings::default().with("inline.enabled", SettingValue::Bool(false));
        repository.save(&saved).unwrap();

        let settings = load_or_defaults(&repository);

        assert_eq!(settings, saved);
    }

    /// The text `Settings::load` logged: `settings: {e:#}`, every cause after a colon.
    #[test]
    fn should_word_the_warning_as_settings_then_the_error_and_each_cause() {
        let temp_dir = TempDir::new("settings-warning");
        let repository = corrupted_settings_file(&temp_dir);
        let error = repository.load().unwrap_err();

        let text = warning(&error);

        assert_eq!(
            text,
            format!(
                "settings: {} is corrupted: key must be a string at line 1 column 2",
                temp_dir.path().join("settings.json").display()
            )
        );
    }

    /// A settings.json saved mid-edit turns the live settings back to the defaults; it does not keep the old ones.
    #[test]
    fn should_fall_back_to_the_default_settings_when_they_cannot_be_loaded() {
        let temp_dir = TempDir::new("settings-lenient");
        let repository = corrupted_settings_file(&temp_dir);

        let settings = load_or_defaults(&repository);

        assert_eq!(settings, Settings::default());
    }
}
