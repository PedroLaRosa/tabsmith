use crate::shared::domain::shared_error::SharedError;
use std::path::PathBuf;

pub(crate) struct DataDirectory {}

impl DataDirectory {
    pub(crate) fn new(_root: Option<PathBuf>, _settings_file_override: Option<PathBuf>) -> Self {
        DataDirectory {}
    }

    pub(crate) fn root(&self) -> Result<PathBuf, SharedError> {
        Err(SharedError::HomeDirectoryNotFound)
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
}
