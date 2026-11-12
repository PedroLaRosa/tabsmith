use crate::self_update::application::ports::installed_executable::InstalledExecutable;
use crate::self_update::domain::self_update_error::SelfUpdateError;
use crate::self_update::domain::value_objects::install_location::InstallLocation;
use std::path::PathBuf;

/// The tabsmith binary on disk that this process runs from.
pub(crate) struct FileInstalledExecutable {
    executable: PathBuf,
    user_home: PathBuf,
}

impl FileInstalledExecutable {
    pub(crate) fn new(executable: PathBuf, user_home: PathBuf) -> Self {
        FileInstalledExecutable { executable, user_home }
    }
}

impl InstalledExecutable for FileInstalledExecutable {
    fn location(&self) -> InstallLocation {
        InstallLocation::new(self.executable.clone(), self.user_home.clone())
    }

    fn replace(&self, bytes: &[u8], _expected_version: &str) -> Result<(), SelfUpdateError> {
        std::fs::write(&self.executable, bytes)
            .map_err(|error| SelfUpdateError::InstallFailed(format!("{}: {error}", self.executable.display())))
    }
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use crate::shared::infrastructure::testing::temp_dir::TempDir;
    use std::fs;

    const RELEASE_9_9_9: &[u8] = b"#!/bin/sh\necho 'tabsmith 9.9.9'\n";

    /// An old binary at `<temp>/home/.local/bin/tabsmith`, and its path.
    fn native_install(temp_dir: &TempDir) -> (FileInstalledExecutable, PathBuf) {
        let home = temp_dir.path().join("home");
        let executable = home.join(".local").join("bin").join("tabsmith");
        fs::create_dir_all(executable.parent().unwrap()).unwrap();
        fs::write(&executable, b"old binary").unwrap();
        (FileInstalledExecutable::new(executable.clone(), home), executable)
    }

    #[test]
    fn should_replace_the_executable_with_a_binary_that_reports_the_expected_version() {
        let temp_dir = TempDir::new("self-update-replace");
        let (installed, executable) = native_install(&temp_dir);

        installed.replace(RELEASE_9_9_9, "9.9.9").unwrap();

        assert_eq!(fs::read(&executable).unwrap(), RELEASE_9_9_9);
    }
}
