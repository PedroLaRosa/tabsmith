use crate::shared::infrastructure::atomic_file::AtomicFile;
use crate::shared::infrastructure::atomic_file_error::AtomicFileError;
use crate::shell_integration::domain::repositories::dotfile_repository::DotfileRepository;
use crate::shell_integration::domain::shell_integration_error::ShellIntegrationError;
use std::path::Path;

/// The dotfiles, script files and fish `conf.d` files on disk.
pub(crate) struct FileDotfileRepository;

impl DotfileRepository for FileDotfileRepository {
    /// Follows symlinks: a dangling one does not exist.
    fn exists(&self, path: &Path) -> bool {
        path.exists()
    }

    fn read(&self, path: &Path) -> Result<String, ShellIntegrationError> {
        std::fs::read_to_string(path).map_err(|e| io_err(path, e))
    }

    /// See [`AtomicFile::write_keeping_permissions`]; its failures keep their ShellIntegrationError (the parent directory or
    /// the dotfile path, as before).
    fn write(&self, path: &Path, contents: &str) -> Result<(), ShellIntegrationError> {
        AtomicFile::new(path)
            .write_keeping_permissions(contents.as_bytes())
            .map_err(|e| match e {
                AtomicFileError::Io { path: failed, source } | AtomicFileError::Rename { path: failed, source } => {
                    io_err(&failed, source)
                }
                // Not produced by this write (it never needs a parent); mapped to stay exhaustive.
                no_parent @ AtomicFileError::NoParentDirectory => io_err(path, std::io::Error::other(no_parent)),
            })
    }

    /// Delete a file; missing is fine.
    fn remove(&self, path: &Path) -> Result<(), ShellIntegrationError> {
        match std::fs::remove_file(path) {
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(io_err(path, e)),
            _ => Ok(()),
        }
    }
}

/// An io failure on `path` as the integration error users see; the backup adapter reports its failures with it too.
pub(crate) fn io_err(path: &Path, e: std::io::Error) -> ShellIntegrationError {
    match e.kind() {
        std::io::ErrorKind::NotFound => ShellIntegrationError::FileDoesNotExist(path.into()),
        std::io::ErrorKind::PermissionDenied => ShellIntegrationError::PermissionDenied(path.into()),
        _ => ShellIntegrationError::Io(path.into(), e),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shared::infrastructure::testing::temp_dir::TempDir;

    #[test]
    fn should_report_a_missing_dotfile_as_file_does_not_exist() {
        let temp_dir = TempDir::new("dotfile-missing");
        let path = temp_dir.path().join(".bashrc");

        let error = FileDotfileRepository.read(&path).unwrap_err();

        assert!(matches!(error, ShellIntegrationError::FileDoesNotExist(p) if p == path));
    }

    /// Windows refuses to open a directory (access denied), so this holds on Unix only.
    #[cfg(unix)]
    #[test]
    fn should_report_reading_a_directory_as_an_io_failure_on_its_path() {
        let temp_dir = TempDir::new("dotfile-directory");
        let path = temp_dir.path().join(".bashrc");
        std::fs::create_dir(&path).unwrap();

        let error = FileDotfileRepository.read(&path).unwrap_err();

        assert_eq!(
            (matches!(error, ShellIntegrationError::Io(..)), error.to_string()),
            (true, format!("{}: Is a directory (os error 21)", path.display()))
        );
    }

    #[cfg(unix)]
    #[test]
    fn should_report_an_unreadable_dotfile_as_permission_denied() {
        use crate::shared::infrastructure::os::process_info::is_root;
        use std::os::unix::fs::PermissionsExt;
        if is_root() {
            return; // root reads it anyway
        }
        let temp_dir = TempDir::new("dotfile-unreadable");
        let path = temp_dir.path().join(".zshrc");
        std::fs::write(&path, "export A=1\n").unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o000)).unwrap();

        let error = FileDotfileRepository.read(&path).unwrap_err();

        assert!(matches!(error, ShellIntegrationError::PermissionDenied(p) if p == path));
    }

    #[test]
    fn should_accept_removing_a_file_that_is_not_there() {
        let temp_dir = TempDir::new("dotfile-remove-missing");

        let removed = FileDotfileRepository.remove(&temp_dir.path().join("bashrc.pre.bash"));

        assert!(removed.is_ok());
    }

    #[cfg(unix)]
    #[test]
    fn should_not_take_a_dangling_symlink_for_an_existing_dotfile() {
        let temp_dir = TempDir::new("dotfile-dangling");
        let path = temp_dir.path().join(".bashrc");
        std::os::unix::fs::symlink("../dotfiles/bashrc", &path).unwrap();

        let exists = FileDotfileRepository.exists(&path);

        assert!(!exists);
    }
}
