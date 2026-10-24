use crate::settings::application::ports::settings_editor::SettingsEditor;
use crate::settings::domain::repositories::settings_repository::SettingsRepository;
use crate::settings::domain::settings_error::SettingsError;
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

    /// True when the editor exited successfully.
    pub(crate) fn execute(&self) -> Result<bool, SettingsError> {
        self.editor.open()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::application::ports::settings_editor::ScriptedSettingsEditor;
    use crate::settings::domain::repositories::settings_repository::InMemorySettingsRepository;

    #[test]
    fn should_report_whether_the_editor_exited_successfully() {
        let use_case = OpenSettingsFileUseCase::new(
            Rc::new(InMemorySettingsRepository::default()),
            Rc::new(ScriptedSettingsEditor::exiting(false)),
        );

        let exited_successfully = use_case.execute().unwrap();

        assert!(!exited_successfully);
    }
}
