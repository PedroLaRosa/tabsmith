use crate::settings::application::ports::settings_editor::SettingsEditor;
use crate::settings::domain::settings_error::SettingsError;
use crate::shared::domain::value_objects::data_directory::DataDirectory;
use std::fmt;

/// `$EDITOR <settings file>` (waits), else macOS `open -t`, Windows `notepad`.
pub(crate) struct SystemSettingsEditor {
    data_directory: DataDirectory,
}

impl SystemSettingsEditor {
    pub(crate) fn new(data_directory: DataDirectory) -> Self {
        SystemSettingsEditor { data_directory }
    }
}

impl SettingsEditor for SystemSettingsEditor {
    fn open(&self) -> Result<bool, SettingsError> {
        let path = self
            .data_directory
            .settings_file()
            .map_err(|error| SettingsError::Storage(Box::new(error)))?;
        let mut cmd = if let Some(editor) = std::env::var("EDITOR")
            .ok()
            .filter(|e| !e.trim().is_empty())
        {
            // ponytail: whitespace split covers `code --wait`; quoted editor paths with spaces are not supported.
            let mut parts = editor.split_whitespace();
            let mut c = std::process::Command::new(parts.next().unwrap_or_default());
            c.args(parts);
            c
        } else if cfg!(target_os = "macos") {
            let mut c = std::process::Command::new("open");
            c.arg("-t");
            c
        } else if cfg!(windows) {
            std::process::Command::new("notepad")
        } else {
            return Err(SettingsError::Editor(Box::new(EditorError::NotSet)));
        };
        let status = cmd
            .arg(path)
            .status()
            .map_err(|error| SettingsError::Editor(Box::new(EditorError::Launch(error))))?;
        Ok(status.success())
    }
}

/// Why no editor ran on the settings file.
#[derive(Debug)]
enum EditorError {
    /// No EDITOR on a system without a default text editor (neither macOS nor Windows).
    NotSet,
    Launch(std::io::Error),
}

impl fmt::Display for EditorError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            EditorError::NotSet => f.write_str("The EDITOR environment variable is not set"),
            EditorError::Launch(_) => f.write_str("could not launch the editor"),
        }
    }
}

impl std::error::Error for EditorError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            EditorError::NotSet => None,
            EditorError::Launch(error) => Some(error),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Linux without EDITOR prints `error: The EDITOR environment variable is not set`.
    #[test]
    fn should_say_that_the_editor_variable_is_not_set() {
        let error = EditorError::NotSet;

        let text = error.to_string();

        assert_eq!(text, "The EDITOR environment variable is not set");
    }
}
