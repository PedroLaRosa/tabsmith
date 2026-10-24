use crate::settings::application::delete_settings_use_case::DeleteSettingsUseCase;
use crate::settings::application::get_setting_use_case::GetSettingUseCase;
use crate::settings::application::list_setting_definitions_use_case::ListSettingDefinitionsUseCase;
use crate::settings::application::list_settings_use_case::ListSettingsUseCase;
use crate::settings::application::open_settings_file_use_case::OpenSettingsFileUseCase;
use crate::settings::application::set_setting_use_case::SetSettingUseCase;
use crate::settings::domain::repositories::settings_repository::SettingsRepository;
use crate::settings::infrastructure::adapters::json_file_settings_repository::JsonFileSettingsRepository;
use crate::settings::infrastructure::adapters::system_settings_editor::SystemSettingsEditor;
use crate::shared::domain::value_objects::data_directory::DataDirectory;
use crate::shared::infrastructure::shared_dependencies::SharedDependencies;
use std::rc::Rc;

/// The settings use cases over settings.json (or the file `TABSMITH_SETTINGS_PATH` names) and the system editor.
pub(crate) struct SettingsModule {
    /// Where the settings file is: the settings command locates it before anything else.
    pub(crate) data_directory: DataDirectory,
    pub(crate) get_setting: GetSettingUseCase,
    pub(crate) set_setting: SetSettingUseCase,
    pub(crate) delete_settings: DeleteSettingsUseCase,
    pub(crate) list_settings: ListSettingsUseCase,
    pub(crate) list_setting_definitions: ListSettingDefinitionsUseCase,
    pub(crate) open_settings_file: OpenSettingsFileUseCase,
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
