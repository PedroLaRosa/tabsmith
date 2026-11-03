use crate::shell_integration::application::ports::data_directory_purger::DataDirectoryPurger;
use std::path::Path;

/// The data directory on disk.
pub(crate) struct FileDataDirectoryPurger;

impl DataDirectoryPurger for FileDataDirectoryPurger {
    fn purge(&self, directory: &Path) -> std::io::Result<()> {
        match std::fs::remove_dir_all(directory) {
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            removed => removed,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shared::infrastructure::testing::temp_dir::TempDir;

    #[test]
    fn should_remove_the_directory_with_everything_in_it() {
        let dir = TempDir::new("purge-removes");
        let data_root = dir.path().join(".tabsmith");
        std::fs::create_dir_all(data_root.join("shell")).unwrap();
        std::fs::write(data_root.join("shell/zshrc.pre.zsh"), "pre").unwrap();

        let purged = FileDataDirectoryPurger.purge(&data_root);

        assert!(purged.is_ok() && !data_root.exists(), "{purged:?}");
    }

    #[test]
    fn should_accept_a_directory_that_is_not_there() {
        let dir = TempDir::new("purge-missing");

        let purged = FileDataDirectoryPurger.purge(&dir.path().join(".tabsmith"));

        assert!(purged.is_ok(), "{purged:?}");
    }

    /// The CLI prints `Could not remove <data>: <error>` with the io error's own text, no path added (T34).
    #[cfg(unix)]
    #[test]
    fn should_return_any_other_failure_as_the_raw_io_error() {
        let dir = TempDir::new("purge-not-a-directory");
        let data_root = dir.path().join(".tabsmith");
        std::fs::write(&data_root, "not a directory").unwrap();

        let failure = FileDataDirectoryPurger.purge(&data_root).unwrap_err();

        assert_eq!(failure.to_string(), "Not a directory (os error 20)");
    }
}
