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
}
