use crate::shared::domain::shared_error::SharedError;
use std::path::PathBuf;

/// tabsmith's home (`~/.tabsmith` or `TABSMITH_HOME`) and every path under it (SPEC §3.1). The root may be unknown
/// (no home directory): only the paths asked for then fail, so commands that never touch the data directory keep
/// working without one.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct DataDirectory {
    root: Option<PathBuf>,
    settings_file_override: Option<PathBuf>,
}

impl DataDirectory {
    /// `root` is `None` when it cannot be determined; `settings_file_override` is a settings file kept outside it.
    pub(crate) fn new(root: Option<PathBuf>, settings_file_override: Option<PathBuf>) -> Self {
        DataDirectory {
            root,
            settings_file_override,
        }
    }

    pub(crate) fn root(&self) -> Result<PathBuf, SharedError> {
        self.root.clone().ok_or(SharedError::HomeDirectoryNotFound)
    }

    /// The override when there is one (it needs no root), else `settings.json` under the root.
    pub(crate) fn settings_file(&self) -> Result<PathBuf, SharedError> {
        match &self.settings_file_override {
            Some(path) => Ok(path.clone()),
            None => self.under("settings.json"),
        }
    }

    /// Always under the root: the settings override does not move it.
    pub(crate) fn state_file(&self) -> Result<PathBuf, SharedError> {
        self.under("state.json")
    }

    pub(crate) fn history_file(&self) -> Result<PathBuf, SharedError> {
        self.under("history.jsonl")
    }

    /// Session context files (`<session id>.json`) and debug-shell rc dirs.
    pub(crate) fn run_dir(&self) -> Result<PathBuf, SharedError> {
        self.under("run")
    }

    pub(crate) fn logs_dir(&self) -> Result<PathBuf, SharedError> {
        self.under("logs")
    }

    pub(crate) fn themes_dir(&self) -> Result<PathBuf, SharedError> {
        self.under("themes")
    }

    /// The dotfile integrations' script files.
    pub(crate) fn shell_dir(&self) -> Result<PathBuf, SharedError> {
        self.under("shell")
    }

    /// Copies of rc files taken before an integration rewrites them.
    pub(crate) fn backups_dir(&self) -> Result<PathBuf, SharedError> {
        self.under("backups")
    }

    /// The extracted spec bundles, one dir per version, and the user dir.
    pub(crate) fn specs_dir(&self) -> Result<PathBuf, SharedError> {
        self.under("specs")
    }

    /// The user's own specs, which override the bundled ones.
    pub(crate) fn user_specs_dir(&self) -> Result<PathBuf, SharedError> {
        Ok(self.specs_dir()?.join("user"))
    }

    fn under(&self, name: &str) -> Result<PathBuf, SharedError> {
        Ok(self.root()?.join(name))
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

    #[test]
    fn should_put_the_backups_directory_under_the_root() {
        let data_directory = DataDirectory::new(Some(PathBuf::from("/data")), None);

        let path = data_directory.backups_dir();

        assert_eq!(path, Ok(PathBuf::from("/data/backups")));
    }

    #[test]
    fn should_put_the_specs_directory_under_the_root() {
        let data_directory = DataDirectory::new(Some(PathBuf::from("/data")), None);

        let path = data_directory.specs_dir();

        assert_eq!(path, Ok(PathBuf::from("/data/specs")));
    }

    #[test]
    fn should_put_the_user_specs_directory_inside_the_specs_directory() {
        let data_directory = DataDirectory::new(Some(PathBuf::from("/data")), None);

        let path = data_directory.user_specs_dir();

        assert_eq!(path, Ok(PathBuf::from("/data/specs/user")));
    }

    #[test]
    fn should_keep_the_state_file_under_the_root_when_the_settings_file_is_overridden() {
        let data_directory = DataDirectory::new(
            Some(PathBuf::from("/data")),
            Some(PathBuf::from("/elsewhere/s.json")),
        );

        let state_file = data_directory.state_file();

        assert_eq!(state_file, Ok(PathBuf::from("/data/state.json")));
    }

    #[test]
    fn should_fail_every_path_under_an_unknown_root() {
        let data_directory = DataDirectory::new(None, None);

        let paths = [
            data_directory.settings_file(),
            data_directory.state_file(),
            data_directory.history_file(),
            data_directory.run_dir(),
            data_directory.logs_dir(),
            data_directory.themes_dir(),
            data_directory.shell_dir(),
            data_directory.backups_dir(),
            data_directory.specs_dir(),
            data_directory.user_specs_dir(),
        ];

        assert_eq!(
            paths,
            [const { Err(SharedError::HomeDirectoryNotFound) }; 10]
        );
    }
}
