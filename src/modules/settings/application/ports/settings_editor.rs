use crate::modules::settings::domain::settings_error::SettingsError;

/// Opens the settings file in the user's editor.
pub(crate) trait SettingsEditor {
    /// Waits for the editor to exit; true when it exited successfully.
    fn open(&self) -> Result<bool, SettingsError>;
}

/// Test double: an editor that exits the way it was scripted to, or none at all.
#[cfg(test)]
pub(crate) struct ScriptedSettingsEditor {
    /// `None` when no editor can be launched.
    exits_successfully: Option<bool>,
}

#[cfg(test)]
impl ScriptedSettingsEditor {
    pub(crate) fn exiting(successfully: bool) -> Self {
        ScriptedSettingsEditor {
            exits_successfully: Some(successfully),
        }
    }

    pub(crate) fn unavailable() -> Self {
        ScriptedSettingsEditor {
            exits_successfully: None,
        }
    }
}

#[cfg(test)]
impl SettingsEditor for ScriptedSettingsEditor {
    fn open(&self) -> Result<bool, SettingsError> {
        self.exits_successfully
            .ok_or_else(|| SettingsError::Editor("no editor to open the settings file".into()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn should_report_the_exit_it_was_scripted_with() {
        let editor = ScriptedSettingsEditor::exiting(false);

        let exited_successfully = editor.open().unwrap();

        assert!(!exited_successfully);
    }

    #[test]
    fn should_fail_to_open_when_scripted_to_have_no_editor() {
        let editor = ScriptedSettingsEditor::unavailable();

        let opened = editor.open();

        assert!(matches!(opened, Err(SettingsError::Editor(_))));
    }
}
