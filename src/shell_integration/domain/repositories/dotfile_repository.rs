use crate::shell_integration::domain::shell_integration_error::ShellIntegrationError;
#[cfg(test)]
use std::cell::RefCell;
#[cfg(test)]
use std::collections::HashMap;
use std::path::Path;
#[cfg(test)]
use std::path::PathBuf;

/// The user's dotfiles and the files tabsmith writes for an integration (script files, fish `conf.d` files).
pub(crate) trait DotfileRepository {
    /// A missing file is `FileDoesNotExist(path)`.
    fn read(&self, path: &Path) -> Result<String, ShellIntegrationError>;
    fn write(&self, path: &Path, contents: &str) -> Result<(), ShellIntegrationError>;
}

/// Dotfiles kept in memory, for tests.
#[cfg(test)]
#[derive(Default)]
pub(crate) struct InMemoryDotfileRepository {
    files: RefCell<HashMap<PathBuf, String>>,
}

#[cfg(test)]
impl DotfileRepository for InMemoryDotfileRepository {
    fn read(&self, path: &Path) -> Result<String, ShellIntegrationError> {
        self.files
            .borrow()
            .get(path)
            .cloned()
            .ok_or_else(|| ShellIntegrationError::FileDoesNotExist(path.into()))
    }

    fn write(&self, path: &Path, contents: &str) -> Result<(), ShellIntegrationError> {
        self.files.borrow_mut().insert(path.into(), contents.into());
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn should_read_back_the_dotfile_it_wrote() {
        let repository = InMemoryDotfileRepository::default();
        let path = Path::new("/home/u/.bashrc");
        repository.write(path, "alias ll='ls -l'\n").unwrap();

        let contents = repository.read(path).unwrap();

        assert_eq!(contents, "alias ll='ls -l'\n");
    }

    #[test]
    fn should_report_a_dotfile_never_written_as_missing() {
        let repository = InMemoryDotfileRepository::default();

        let error = repository.read(Path::new("/home/u/.zshrc")).unwrap_err();

        assert!(matches!(error, ShellIntegrationError::FileDoesNotExist(p) if p == Path::new("/home/u/.zshrc")));
    }
}
