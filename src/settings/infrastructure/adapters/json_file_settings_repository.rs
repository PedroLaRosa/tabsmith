use crate::settings::domain::repositories::settings_repository::SettingsRepository;
use crate::settings::domain::settings_error::SettingsError;
use crate::settings::domain::value_objects::settings::Settings;
use crate::settings::infrastructure::adapters::setting_value_json::settings_from_json;
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

    fn save(&self, _settings: &Settings) -> Result<(), SettingsError> {
        todo!()
    }

    fn last_modified(&self) -> Option<SystemTime> {
        todo!()
    }
}

fn storage_error(error: impl std::error::Error + Send + Sync + 'static) -> SettingsError {
    SettingsError::Storage(Box::new(error))
}

#[cfg(test)]
mod tests {
    use super::*;
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
}
