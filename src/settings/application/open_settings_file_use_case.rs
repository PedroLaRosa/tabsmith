use crate::settings::application::ports::settings_editor::SettingsEditor;
use crate::settings::domain::repositories::settings_repository::SettingsRepository;
use crate::settings::domain::settings_error::SettingsError;
use crate::settings::domain::value_objects::settings::Settings;
use std::rc::Rc;

/// `tabsmith settings open`: opens the settings file in the user's editor and waits for it.
pub(crate) struct OpenSettingsFileUseCase {
    settings: Rc<dyn SettingsRepository>,
    editor: Rc<dyn SettingsEditor>,
}

impl OpenSettingsFileUseCase {
    pub(crate) fn new(
        settings: Rc<dyn SettingsRepository>,
        editor: Rc<dyn SettingsEditor>,
    ) -> Self {
        OpenSettingsFileUseCase { settings, editor }
    }

    /// True when the editor exited successfully. Settings that were never saved are saved empty first, before an
    /// editor is even chosen, so the file is there for it.
    pub(crate) fn execute(&self) -> Result<bool, SettingsError> {
        if self.settings.last_modified().is_none() {
            self.settings.save(&Settings::default())?;
        }
        self.editor.open()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::application::ports::settings_editor::ScriptedSettingsEditor;
    use crate::settings::domain::repositories::settings_repository::InMemorySettingsRepository;
    use crate::settings::domain::value_objects::setting_value::SettingValue;

    #[test]
    fn should_report_whether_the_editor_exited_successfully() {
        let use_case = OpenSettingsFileUseCase::new(
            Rc::new(InMemorySettingsRepository::default()),
            Rc::new(ScriptedSettingsEditor::exiting(false)),
        );

        let exited_successfully = use_case.execute().unwrap();

        assert!(!exited_successfully);
    }

    /// A fresh install opens an editor on `{}` rather than on a file that does not exist.
    #[test]
    fn should_save_empty_settings_when_none_are_saved_yet() {
        let repository = Rc::new(InMemorySettingsRepository::default());
        let use_case = OpenSettingsFileUseCase::new(
            repository.clone(),
            Rc::new(ScriptedSettingsEditor::exiting(true)),
        );

        use_case.execute().unwrap();

        assert_eq!(
            (
                repository.last_modified().is_some(),
                repository.load().unwrap()
            ),
            (true, Settings::default())
        );
    }

    #[test]
    fn should_leave_saved_settings_as_they_are() {
        let repository = Rc::new(InMemorySettingsRepository::default());
        let saved = Settings::default().with("inline.enabled", SettingValue::Bool(false));
        repository.save(&saved).unwrap();
        let use_case = OpenSettingsFileUseCase::new(
            repository.clone(),
            Rc::new(ScriptedSettingsEditor::exiting(true)),
        );

        use_case.execute().unwrap();

        assert_eq!(repository.load().unwrap(), saved);
    }

    /// Linux without EDITOR fails, but the settings file exists afterwards, as it always did.
    #[test]
    fn should_save_empty_settings_even_when_no_editor_can_be_launched() {
        let repository = Rc::new(InMemorySettingsRepository::default());
        let use_case = OpenSettingsFileUseCase::new(
            repository.clone(),
            Rc::new(ScriptedSettingsEditor::unavailable()),
        );

        let opened = use_case.execute();

        assert_eq!(
            (opened.is_err(), repository.last_modified().is_some()),
            (true, true)
        );
    }
}
