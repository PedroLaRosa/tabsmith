use crate::shell_integration::domain::entities::integration::Integration;
use crate::shell_integration::domain::shell_integration_error::ShellIntegrationError;
#[cfg(test)]
use std::cell::RefCell;
use std::path::Path;
#[cfg(test)]
use std::path::PathBuf;

/// Copies of the dotfiles an install rewrites, taken before it touches them.
pub(crate) trait BackupRepository {
    /// Copy the dotfile into `backup_dir`, under the dotfile's own file name.
    fn back_up(&self, dotfile: &Integration, backup_dir: &Path) -> Result<(), ShellIntegrationError>;
}

/// Remembers which dotfile was backed up into which dir, for tests.
#[cfg(test)]
#[derive(Default)]
pub(crate) struct InMemoryBackupRepository {
    backups: RefCell<Vec<(PathBuf, PathBuf)>>,
}

#[cfg(test)]
impl InMemoryBackupRepository {
    /// `(dotfile path, backup dir)` of each backup, in the order they were taken.
    pub(crate) fn backups(&self) -> Vec<(PathBuf, PathBuf)> {
        self.backups.borrow().clone()
    }
}

#[cfg(test)]
impl BackupRepository for InMemoryBackupRepository {
    fn back_up(&self, dotfile: &Integration, backup_dir: &Path) -> Result<(), ShellIntegrationError> {
        self.backups
            .borrow_mut()
            .push((dotfile.path().into(), backup_dir.into()));
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shared::domain::value_objects::shell::Shell;

    #[test]
    fn should_record_each_dotfile_it_backs_up_with_its_backup_dir() {
        let repository = InMemoryBackupRepository::default();
        let bashrc = Integration::dotfile(Shell::Bash, PathBuf::from("/home/u/.bashrc"));
        let zshrc = Integration::dotfile(Shell::Zsh, PathBuf::from("/home/u/.zshrc"));

        repository.back_up(&bashrc, Path::new("/data/backups/100")).unwrap();
        repository.back_up(&zshrc, Path::new("/data/backups/101")).unwrap();

        assert_eq!(
            repository.backups(),
            [
                (PathBuf::from("/home/u/.bashrc"), PathBuf::from("/data/backups/100")),
                (PathBuf::from("/home/u/.zshrc"), PathBuf::from("/data/backups/101")),
            ]
        );
    }
}
