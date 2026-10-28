use crate::shell_integration::domain::entities::integration::Integration;
use crate::shell_integration::domain::repositories::backup_repository::BackupRepository;
use crate::shell_integration::domain::shell_integration_error::ShellIntegrationError;
use crate::shell_integration::infrastructure::adapters::file_dotfile_repository::io_err;
use std::path::Path;

/// Backups as byte copies on disk (they follow symlinks and keep the permission bits).
pub(crate) struct FileBackupRepository;

impl BackupRepository for FileBackupRepository {
    fn back_up(&self, dotfile: &Integration, backup_dir: &Path) -> Result<(), ShellIntegrationError> {
        std::fs::create_dir_all(backup_dir).map_err(|e| io_err(backup_dir, e))?;
        std::fs::copy(dotfile.path(), backup_dir.join(dotfile.file_name()))
            .map_err(|e| io_err(dotfile.path(), e))?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shared::domain::value_objects::shell::Shell;
    use crate::shared::infrastructure::testing::temp_dir::TempDir;

    #[test]
    fn should_copy_the_dotfile_into_the_backup_dir_under_its_file_name() {
        let temp_dir = TempDir::new("backup-copy");
        let bashrc = temp_dir.path().join(".bashrc");
        std::fs::write(&bashrc, "alias ll='ls -l'\n").unwrap();
        let backup_dir = temp_dir.path().join("data/backups/100");

        FileBackupRepository
            .back_up(&Integration::dotfile(Shell::Bash, bashrc), &backup_dir)
            .unwrap();

        assert_eq!(
            std::fs::read_to_string(backup_dir.join(".bashrc")).unwrap(),
            "alias ll='ls -l'\n"
        );
    }
}
