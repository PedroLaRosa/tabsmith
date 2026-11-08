use crate::modules::settings::application::delete_settings_use_case::DeleteSettingsUseCase;
use crate::modules::settings::application::get_setting_use_case::GetSettingUseCase;
use crate::modules::settings::application::list_setting_definitions_use_case::ListSettingDefinitionsUseCase;
use crate::modules::settings::application::list_settings_use_case::ListSettingsUseCase;
use crate::modules::settings::application::open_settings_file_use_case::OpenSettingsFileUseCase;
use crate::modules::settings::application::set_setting_use_case::SetSettingUseCase;
use crate::modules::settings::domain::repositories::settings_repository::SettingsRepository;
use crate::modules::settings::infrastructure::adapters::json_file_settings_repository::JsonFileSettingsRepository;
use crate::modules::settings::infrastructure::adapters::system_settings_editor::SystemSettingsEditor;
use crate::modules::shared::domain::shared_error::SharedError;
use crate::modules::shared::domain::value_objects::data_directory::DataDirectory;
use crate::modules::shared::infrastructure::shared_dependencies::SharedDependencies;
use std::path::PathBuf;
use std::rc::Rc;

/// The settings use cases over settings.json (or the file `TABSMITH_SETTINGS_PATH` names) and the system editor.
pub(crate) struct SettingsModule {
    data_directory: DataDirectory,
    pub(crate) get_setting: GetSettingUseCase,
    pub(crate) set_setting: SetSettingUseCase,
    pub(crate) delete_settings: DeleteSettingsUseCase,
    pub(crate) list_settings: ListSettingsUseCase,
    pub(crate) list_setting_definitions: ListSettingDefinitionsUseCase,
    pub(crate) open_settings_file: OpenSettingsFileUseCase,
}

impl SettingsModule {
    /// Where the settings file is, or the "no home" error: the settings command locates it before anything else.
    pub(crate) fn locate_settings_file(&self) -> Result<PathBuf, SharedError> {
        self.data_directory.settings_file()
    }
}

pub(crate) fn build_settings_module(dependencies: &SharedDependencies) -> SettingsModule {
    let data_directory = dependencies.data_directory.clone();
    let settings: Rc<dyn SettingsRepository> =
        Rc::new(JsonFileSettingsRepository::new(data_directory.clone()));
    let editor = Rc::new(SystemSettingsEditor::new(data_directory.clone()));
    SettingsModule {
        data_directory,
        get_setting: GetSettingUseCase::new(settings.clone()),
        set_setting: SetSettingUseCase::new(settings.clone()),
        delete_settings: DeleteSettingsUseCase::new(settings.clone()),
        list_settings: ListSettingsUseCase::new(settings.clone()),
        list_setting_definitions: ListSettingDefinitionsUseCase,
        open_settings_file: OpenSettingsFileUseCase::new(settings, editor),
    }
}
