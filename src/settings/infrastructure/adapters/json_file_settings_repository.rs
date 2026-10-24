use crate::settings::domain::repositories::settings_repository::SettingsRepository;
use crate::settings::domain::settings_error::SettingsError;
use crate::settings::domain::value_objects::settings::Settings;
use crate::settings::infrastructure::adapters::setting_value_json::{
    settings_from_json, settings_to_json,
};
use crate::shared::domain::value_objects::data_directory::DataDirectory;
use crate::shared::infrastructure::json_document_store::JsonDocumentStore;
use std::time::SystemTime;

/// settings.json in the data directory, or the file `TABSMITH_SETTINGS_PATH` names. The path is resolved on every
/// call, so a missing home only fails the calls that need it.
pub(crate) struct JsonFileSettingsRepository {
    data_directory: DataDirectory,
}

impl JsonFileSettingsRepository {
    pub(crate) fn new(data_directory: DataDirectory) -> Self {
        JsonFileSettingsRepository { data_directory }
    }
}

impl SettingsRepository for JsonFileSettingsRepository {
    /// A missing or blank file is no settings; a file that is not a JSON object is an error.
    fn load(&self) -> Result<Settings, SettingsError> {
        let path = self.data_directory.settings_file().map_err(storage_error)?;
        let document = JsonDocumentStore::load(&path).map_err(storage_error)?;
        Ok(settings_from_json(document.map))
    }

    /// Written with [`JsonDocumentStore::save`]: pretty JSON, a symlinked file stays a symlink, Unix mode 0600.
    fn save(&self, settings: &Settings) -> Result<(), SettingsError> {
        let document = JsonDocumentStore {
            path: self.data_directory.settings_file().map_err(storage_error)?,
            map: settings_to_json(settings),
        };
        document.save().map_err(storage_error)
    }

    /// `None` when the file is missing, unreadable or has no known path (no home), as the pty's `mtime` was.
    fn last_modified(&self) -> Option<SystemTime> {
        let path = self.data_directory.settings_file().ok()?;
        std::fs::metadata(path).ok()?.modified().ok()
    }
}

fn storage_error(error: impl std::error::Error + Send + Sync + 'static) -> SettingsError {
    SettingsError::Storage(Box::new(error))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::domain::value_objects::setting_number::SettingNumber;
    use crate::settings::domain::value_objects::setting_value::SettingValue;
    use crate::shared::infrastructure::testing::temp_dir::TempDir;

    fn repository_in(temp_dir: &TempDir) -> JsonFileSettingsRepository {
        JsonFileSettingsRepository::new(DataDirectory::new(Some(temp_dir.path().to_path_buf()), None))
    }

    #[test]
    fn should_load_no_settings_from_a_missing_settings_file() {
        let temp_dir = TempDir::new("settings-missing");
        let repository = repository_in(&temp_dir);

        let settings = repository.load().unwrap();

        assert_eq!(settings.entries().count(), 0);
    }

    /// The text `settings autocomplete.height 5` has always written: pretty, keys sorted, a trailing newline.
    #[test]
    fn should_save_settings_as_pretty_json_with_sorted_keys_and_a_trailing_newline() {
        let temp_dir = TempDir::new("settings-save");
        let repository = repository_in(&temp_dir);
        let settings = Settings::default()
            .with("autocomplete.theme", SettingValue::Text("light".to_string()))
            .with("autocomplete.height", SettingValue::Number(SettingNumber::from(5u64)));

        repository.save(&settings).unwrap();

        assert_eq!(
            std::fs::read_to_string(temp_dir.path().join("settings.json")).unwrap(),
            "{\n  \"autocomplete.height\": 5,\n  \"autocomplete.theme\": \"light\"\n}\n"
        );
    }

    /// The pty reloads its settings when this time changes.
    #[test]
    fn should_give_the_modification_time_of_the_settings_file() {
        let temp_dir = TempDir::new("settings-mtime");
        let repository = repository_in(&temp_dir);
        let path = temp_dir.path().join("settings.json");
        std::fs::write(&path, "{}\n").unwrap();

        let modified = repository.last_modified();

        assert_eq!(modified, std::fs::metadata(&path).unwrap().modified().ok());
    }
}
