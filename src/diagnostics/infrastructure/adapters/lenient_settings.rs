use crate::diagnostics::application::ports::settings_snapshot::SettingsSnapshot;
use crate::settings::domain::repositories::settings_repository::SettingsRepository;
use crate::settings::domain::settings_error::SettingsError;
use crate::settings::domain::value_objects::settings::Settings;
use crate::shared::infrastructure::error_chain::display_chain;
use crate::shared::infrastructure::logger;
use std::rc::Rc;

/// The saved settings, or the defaults while they cannot be loaded (a file mid-edit, no home directory): doctor keeps
/// working where the settings commands refuse.
pub(crate) struct LenientSettings {
    repository: Rc<dyn SettingsRepository>,
}

impl LenientSettings {
    pub(crate) fn new(repository: Rc<dyn SettingsRepository>) -> Self {
        LenientSettings { repository }
    }
}

impl SettingsSnapshot for LenientSettings {
    fn load(&self) -> Settings {
        self.repository.load().unwrap_or_else(|error| {
            logger::warn(&warning(&error));
            Settings::default()
        })
    }
}

/// `settings: ` and the error with each cause after a colon, as anyhow's `{:#}` printed it.
fn warning(error: &SettingsError) -> String {
    format!("settings: {}", display_chain(error))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diagnostics::application::ports::settings_snapshot::SettingsSnapshot;
    use crate::settings::domain::repositories::settings_repository::{
        InMemorySettingsRepository, SettingsRepository,
    };
    use crate::settings::domain::value_objects::setting_value::SettingValue;
    use crate::settings::domain::value_objects::settings::Settings;
    use crate::settings::infrastructure::adapters::json_file_settings_repository::JsonFileSettingsRepository;
    use crate::shared::domain::value_objects::data_directory::DataDirectory;
    use crate::shared::infrastructure::testing::temp_dir::TempDir;
    use std::rc::Rc;

    fn corrupted_settings_file(temp_dir: &TempDir) -> JsonFileSettingsRepository {
        std::fs::write(temp_dir.path().join("settings.json"), "{oops").unwrap();
        JsonFileSettingsRepository::new(DataDirectory::new(Some(temp_dir.path().to_path_buf()), None))
    }

    #[test]
    fn should_give_doctor_the_settings_the_repository_loads() {
        let repository = Rc::new(InMemorySettingsRepository::default());
        let saved = Settings::default().with("inline.enabled", SettingValue::Bool(false));
        repository.save(&saved).unwrap();
        let snapshot = LenientSettings::new(repository);

        let loaded = snapshot.load();

        assert_eq!(loaded, saved);
    }

    /// The warning itself goes to the process-wide log, which a test cannot read: the defaults are what shows.
    #[test]
    fn should_fall_back_to_the_defaults_when_the_settings_cannot_be_loaded() {
        let temp_dir = TempDir::new("doctor-lenient-settings");
        let snapshot = LenientSettings::new(Rc::new(corrupted_settings_file(&temp_dir)));

        let loaded = snapshot.load();

        assert_eq!(loaded, Settings::default());
    }

    #[test]
    fn should_warn_with_the_settings_prefix_and_each_cause_after_a_colon() {
        let temp_dir = TempDir::new("doctor-lenient-warning");
        let error = corrupted_settings_file(&temp_dir).load().unwrap_err();

        let text = warning(&error);

        assert_eq!(
            text,
            format!(
                "settings: {} is corrupted: key must be a string at line 1 column 2",
                temp_dir.path().join("settings.json").display()
            )
        );
    }
}
