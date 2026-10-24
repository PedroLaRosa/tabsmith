use crate::shared::domain::shared_error::SharedError;
use std::path::PathBuf;

pub(crate) struct DataDirectory {
    root: Option<PathBuf>,
    settings_file_override: Option<PathBuf>,
}

impl DataDirectory {
    pub(crate) fn new(root: Option<PathBuf>, settings_file_override: Option<PathBuf>) -> Self {
        DataDirectory {
            root,
            settings_file_override,
        }
    }

    pub(crate) fn root(&self) -> Result<PathBuf, SharedError> {
        self.root.clone().ok_or(SharedError::HomeDirectoryNotFound)
    }

    pub(crate) fn settings_file(&self) -> Result<PathBuf, SharedError> {
        if let Some(path) = &self.settings_file_override {
            return Ok(path.clone());
        }
        Ok(self.root()?.join("settings.json"))
    }

    pub(crate) fn state_file(&self) -> Result<PathBuf, SharedError> {
        Ok(self.root()?.join("state.json"))
    }

    pub(crate) fn history_file(&self) -> Result<PathBuf, SharedError> {
        Ok(self.root()?.join("history.jsonl"))
    }

    pub(crate) fn run_dir(&self) -> Result<PathBuf, SharedError> {
        Ok(self.root()?.join("run"))
    }

    pub(crate) fn logs_dir(&self) -> Result<PathBuf, SharedError> {
        Ok(self.root()?.join("logs"))
    }

    pub(crate) fn themes_dir(&self) -> Result<PathBuf, SharedError> {
        Ok(self.root()?.join("themes"))
    }

    pub(crate) fn shell_dir(&self) -> Result<PathBuf, SharedError> {
        Ok(self.root()?.join("shell"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn should_report_an_unknown_root_with_the_original_message() {
        let data_directory = DataDirectory::new(None, None);

        let error = data_directory.root().unwrap_err();

        assert_eq!(
            error.to_string(),
            "could not determine the home directory (set HOME or TABSMITH_HOME)"
        );
    }

    #[test]
    fn should_be_rooted_where_it_was_built() {
        let data_directory = DataDirectory::new(Some(PathBuf::from("/data")), None);

        let root = data_directory.root();

        assert_eq!(root, Ok(PathBuf::from("/data")));
    }

    #[test]
    fn should_put_the_state_file_under_the_root() {
        let data_directory = DataDirectory::new(Some(PathBuf::from("/data")), None);

        let state_file = data_directory.state_file();

        assert_eq!(state_file, Ok(PathBuf::from("/data/state.json")));
    }

    #[test]
    fn should_put_the_settings_file_under_the_root_when_it_is_not_overridden() {
        let data_directory = DataDirectory::new(Some(PathBuf::from("/data")), None);

        let settings_file = data_directory.settings_file();

        assert_eq!(settings_file, Ok(PathBuf::from("/data/settings.json")));
    }

    #[test]
    fn should_use_the_settings_file_override_even_without_a_root() {
        let data_directory = DataDirectory::new(None, Some(PathBuf::from("/elsewhere/s.json")));

        let settings_file = data_directory.settings_file();

        assert_eq!(settings_file, Ok(PathBuf::from("/elsewhere/s.json")));
    }

    #[test]
    fn should_put_the_history_file_under_the_root() {
        let data_directory = DataDirectory::new(Some(PathBuf::from("/data")), None);

        let path = data_directory.history_file();

        assert_eq!(path, Ok(PathBuf::from("/data/history.jsonl")));
    }

    #[test]
    fn should_put_the_run_directory_under_the_root() {
        let data_directory = DataDirectory::new(Some(PathBuf::from("/data")), None);

        let path = data_directory.run_dir();

        assert_eq!(path, Ok(PathBuf::from("/data/run")));
    }

    #[test]
    fn should_put_the_logs_directory_under_the_root() {
        let data_directory = DataDirectory::new(Some(PathBuf::from("/data")), None);

        let path = data_directory.logs_dir();

        assert_eq!(path, Ok(PathBuf::from("/data/logs")));
    }

    #[test]
    fn should_put_the_themes_directory_under_the_root() {
        let data_directory = DataDirectory::new(Some(PathBuf::from("/data")), None);

        let path = data_directory.themes_dir();

        assert_eq!(path, Ok(PathBuf::from("/data/themes")));
    }

    #[test]
    fn should_put_the_shell_scripts_directory_under_the_root() {
        let data_directory = DataDirectory::new(Some(PathBuf::from("/data")), None);

        let path = data_directory.shell_dir();

        assert_eq!(path, Ok(PathBuf::from("/data/shell")));
    }
}
