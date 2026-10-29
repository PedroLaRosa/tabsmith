use crate::shell_integration::application::ports::data_directory_purger::DataDirectoryPurger;
use std::path::Path;

/// The data directory on disk.
pub(crate) struct FileDataDirectoryPurger;

impl DataDirectoryPurger for FileDataDirectoryPurger {
    fn purge(&self, directory: &Path) -> std::io::Result<()> {
        std::fs::remove_dir_all(directory)
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
}
