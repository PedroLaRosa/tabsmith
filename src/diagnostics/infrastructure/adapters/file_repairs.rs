use crate::shared::application::ports::clock::Clock;
use std::path::Path;
use std::sync::Arc;

/// The file-system repairs doctor offers for a missing data directory and a corrupted settings file.
pub(crate) struct FileRepairs {
    clock: Arc<dyn Clock + Send + Sync>,
}

impl FileRepairs {
    pub(crate) fn new(clock: Arc<dyn Clock + Send + Sync>) -> Self {
        FileRepairs { clock }
    }

    /// With the default umask, not 0700: the data directory was never created private by doctor.
    pub(crate) fn create_data_directory(&self, path: &Path) -> std::io::Result<()> {
        std::fs::create_dir_all(path)
    }

    /// Keeps the corrupted bytes in `settings.json.bak-<seconds>` (the extension is replaced, not appended) and writes
    /// an empty object over the file. Not atomic, follows symlinks and keeps the existing mode: a corrupt file must
    /// not stop the repair, so neither the settings repository nor an atomic write fit here.
    pub(crate) fn reset_settings(&self, path: &Path) -> std::io::Result<()> {
        std::fs::copy(
            path,
            path.with_extension(format!("json.bak-{}", self.clock.now_secs())),
        )?;
        std::fs::write(path, "{}\n")?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shared::application::ports::clock::FixedClock;
    use crate::shared::infrastructure::testing::temp_dir::TempDir;
    use std::io::ErrorKind;

    fn repairs() -> FileRepairs {
        FileRepairs::new(Arc::new(FixedClock::new(1_700_000_000_000)))
    }

    #[test]
    fn should_back_up_the_old_bytes_next_to_the_settings_file_named_after_the_clock_seconds() {
        let temp_dir = TempDir::new("repairs-backup");
        let path = temp_dir.path().join("settings.json");
        std::fs::write(&path, "{oops").unwrap();

        repairs().reset_settings(&path).unwrap();

        let backup = temp_dir.path().join("settings.json.bak-1700000000");
        assert_eq!(std::fs::read_to_string(backup).unwrap(), "{oops");
    }

    #[test]
    fn should_reset_the_settings_file_to_an_empty_object() {
        let temp_dir = TempDir::new("repairs-reset");
        let path = temp_dir.path().join("settings.json");
        std::fs::write(&path, "{oops").unwrap();

        repairs().reset_settings(&path).unwrap();

        assert_eq!(std::fs::read_to_string(&path).unwrap(), "{}\n");
    }

    #[test]
    fn should_replace_another_extension_with_json_bak_when_backing_up() {
        let temp_dir = TempDir::new("repairs-other-extension");
        let path = temp_dir.path().join("settings.conf");
        std::fs::write(&path, "old").unwrap();

        repairs().reset_settings(&path).unwrap();

        let backup = temp_dir.path().join("settings.json.bak-1700000000");
        assert_eq!(std::fs::read_to_string(backup).unwrap(), "old");
    }

    #[test]
    fn should_append_json_bak_to_a_file_name_without_extension() {
        let temp_dir = TempDir::new("repairs-no-extension");
        let path = temp_dir.path().join("x");
        std::fs::write(&path, "old").unwrap();

        repairs().reset_settings(&path).unwrap();

        let backup = temp_dir.path().join("x.json.bak-1700000000");
        assert_eq!(std::fs::read_to_string(backup).unwrap(), "old");
    }

    #[test]
    fn should_fail_with_not_found_and_write_nothing_when_the_settings_file_is_missing() {
        let temp_dir = TempDir::new("repairs-missing");
        let path = temp_dir.path().join("settings.json");

        let result = repairs().reset_settings(&path);

        let written: Vec<_> = std::fs::read_dir(temp_dir.path()).unwrap().collect();
        assert_eq!(
            (result.unwrap_err().kind(), written.len()),
            (ErrorKind::NotFound, 0)
        );
    }

    // unix only: the mode bits and symlinks of the test below are not portable; the test only chmods and links data files.
    #[cfg(unix)]
    #[test]
    fn should_keep_the_mode_of_the_existing_settings_file() {
        use std::os::unix::fs::PermissionsExt;
        let temp_dir = TempDir::new("repairs-mode");
        let path = temp_dir.path().join("settings.json");
        std::fs::write(&path, "{oops").unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).unwrap();

        repairs().reset_settings(&path).unwrap();

        let mode = std::fs::metadata(&path).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o644);
    }

    // unix only: symlinks need privileges on Windows; the test only links data files.
    #[cfg(unix)]
    #[test]
    fn should_write_through_a_symlinked_settings_file() {
        let temp_dir = TempDir::new("repairs-symlink");
        let target = temp_dir.path().join("target.json");
        let link = temp_dir.path().join("settings.json");
        std::fs::write(&target, "{oops").unwrap();
        std::os::unix::fs::symlink(&target, &link).unwrap();

        repairs().reset_settings(&link).unwrap();

        let still_a_link = std::fs::symlink_metadata(&link).unwrap().file_type().is_symlink();
        assert_eq!(
            (still_a_link, std::fs::read_to_string(&target).unwrap()),
            (true, "{}\n".to_string())
        );
    }

    #[test]
    fn should_create_the_nested_data_directories() {
        let temp_dir = TempDir::new("repairs-create");
        let path = temp_dir.path().join("a").join("b");

        repairs().create_data_directory(&path).unwrap();

        assert!(path.is_dir());
    }

    #[test]
    fn should_accept_a_data_directory_that_already_exists() {
        let temp_dir = TempDir::new("repairs-create-existing");

        let result = repairs().create_data_directory(temp_dir.path());

        assert!(result.is_ok());
    }
}
