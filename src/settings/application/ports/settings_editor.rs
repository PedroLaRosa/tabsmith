use crate::settings::domain::settings_error::SettingsError;

/// Opens the settings file in the user's editor.
pub(crate) trait SettingsEditor {
    /// Waits for the editor to exit; true when it exited successfully.
    fn open(&self) -> Result<bool, SettingsError>;
}

/// Test double: an editor that exits the way it was scripted to.
#[cfg(test)]
pub(crate) struct ScriptedSettingsEditor {
    exits_successfully: bool,
}

#[cfg(test)]
impl ScriptedSettingsEditor {
    pub(crate) fn exiting(successfully: bool) -> Self {
        ScriptedSettingsEditor {
            exits_successfully: successfully,
        }
    }
}

#[cfg(test)]
impl SettingsEditor for ScriptedSettingsEditor {
    fn open(&self) -> Result<bool, SettingsError> {
        Ok(self.exits_successfully)
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
}
