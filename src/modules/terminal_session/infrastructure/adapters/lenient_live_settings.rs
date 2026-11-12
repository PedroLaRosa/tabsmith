use crate::modules::settings::domain::repositories::settings_repository::SettingsRepository;
use crate::modules::settings::domain::settings_error::SettingsError;
use crate::modules::settings::domain::value_objects::settings::Settings;
use crate::modules::shared::infrastructure::error_chain::display_chain;
use crate::modules::shared::infrastructure::logger;
use crate::modules::terminal_session::application::ports::live_settings::LiveSettings;
use std::rc::Rc;
use std::time::SystemTime;

/// The saved settings, or the defaults while they cannot be loaded (a file mid-edit, no home directory): the
/// session keeps running where the settings commands refuse.
pub(crate) struct LenientLiveSettings {
    repository: Rc<dyn SettingsRepository>,
}

impl LenientLiveSettings {
    pub(crate) fn new(repository: Rc<dyn SettingsRepository>) -> Self {
        LenientLiveSettings { repository }
    }
}

impl LiveSettings for LenientLiveSettings {
    fn last_modified(&self) -> Option<SystemTime> {
        self.repository.last_modified()
    }

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
    use crate::modules::settings::domain::repositories::settings_repository::InMemorySettingsRepository;
    use crate::modules::settings::domain::value_objects::setting_value::SettingValue;
    use crate::modules::settings::infrastructure::adapters::json_file_settings_repository::JsonFileSettingsRepository;
    use crate::modules::shared::domain::value_objects::data_directory::DataDirectory;
    use crate::modules::shared::infrastructure::testing::temp_dir::TempDir;

    fn corrupted_settings_file(temp_dir: &TempDir) -> JsonFileSettingsRepository {
        std::fs::write(temp_dir.path().join("settings.json"), "{oops").unwrap();
        JsonFileSettingsRepository::new(DataDirectory::new(Some(temp_dir.path().to_path_buf()), None))
    }

    #[test]
    fn should_load_what_the_repository_saved() {
        let repository = Rc::new(InMemorySettingsRepository::default());
        let saved = Settings::default().with("inline.enabled", SettingValue::Bool(false));
        repository.save(&saved).unwrap();
        let live = LenientLiveSettings::new(repository);

        let loaded = live.load();

        assert_eq!(loaded, saved);
    }

    /// The warning itself goes to the process-wide log, which a test cannot read: the defaults are what shows.
    #[test]
    fn should_use_the_defaults_and_warn_when_the_settings_cannot_be_loaded() {
        let temp_dir = TempDir::new("live-settings-lenient");
        let live = LenientLiveSettings::new(Rc::new(corrupted_settings_file(&temp_dir)));

        let loaded = live.load();

        assert_eq!(loaded, Settings::default());
    }

    /// The text the legacy `load_or_defaults` logged (now `LenientSettings` in diagnostics): `settings: {e:#}`,
    /// every cause after a colon.
    #[test]
    fn should_word_the_session_settings_warning_like_the_load_or_defaults_one() {
        let temp_dir = TempDir::new("live-settings-warning");
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

    #[test]
    fn should_report_the_modification_time_of_the_repository() {
        let repository = Rc::new(InMemorySettingsRepository::default());
        repository.save(&Settings::default()).unwrap();
        let live = LenientLiveSettings::new(repository.clone());

        let modified = live.last_modified();

        assert_eq!((modified.is_some(), modified), (true, repository.last_modified()));
    }
}
