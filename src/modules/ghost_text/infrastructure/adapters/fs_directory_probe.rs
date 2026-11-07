use crate::modules::ghost_text::application::ports::directory_probe::DirectoryProbe;
use crate::modules::shared::infrastructure::data_directory_from_env::user_home_from_env;
use std::path::Path;

/// The real file system and the process environment.
pub(crate) struct FsDirectoryProbe;

impl DirectoryProbe for FsDirectoryProbe {
    fn is_dir(&self, path: &Path) -> bool {
        path.is_dir()
    }

    fn resolves_to_directory(&self, path: &Path) -> bool {
        path.canonicalize().is_ok_and(|p| p.is_dir())
    }

    fn home_directory(&self) -> Option<String> {
        user_home_from_env()
            .ok()
            .map(|p| p.to_string_lossy().into_owned())
    }
}
