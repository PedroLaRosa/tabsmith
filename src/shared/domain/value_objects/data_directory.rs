use crate::shared::domain::shared_error::SharedError;
use std::path::PathBuf;

pub(crate) struct DataDirectory {
    root: Option<PathBuf>,
}

impl DataDirectory {
    pub(crate) fn new(root: Option<PathBuf>, _settings_file_override: Option<PathBuf>) -> Self {
        DataDirectory { root }
    }

    pub(crate) fn root(&self) -> Result<PathBuf, SharedError> {
        self.root.clone().ok_or(SharedError::HomeDirectoryNotFound)
    }

    pub(crate) fn settings_file(&self) -> Result<PathBuf, SharedError> {
        Ok(self.root()?.join("settings.json"))
    }

    pub(crate) fn state_file(&self) -> Result<PathBuf, SharedError> {
        Ok(self.root()?.join("state.json"))
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
}
