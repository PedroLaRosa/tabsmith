use crate::shared::infrastructure::atomic_file_error::AtomicFileError;
use std::io::Write;
use std::path::Path;

/// Replaces a file in one step: the new bytes go to a temp file, which is then renamed over the target, so no
/// reader ever sees an empty or half-written file. Each kind of file tabsmith writes keeps its own rules, so each
/// has its own method; they differ on purpose (directory creation, modes, symlinks, temp names, error paths).
pub(crate) struct AtomicFile<'a> {
    path: &'a Path,
}

impl<'a> AtomicFile<'a> {
    pub(crate) fn new(path: &'a Path) -> AtomicFile<'a> {
        AtomicFile { path }
    }

    /// Atomic write (temp + rename); Unix: file 0600, parent dir 0700 (SPEC §2.11). The parent's parent (the data
    /// root) must exist: a running session never recreates a purged `~/.tabsmith`.
    pub(crate) fn write_private(&self, bytes: &[u8]) -> Result<(), AtomicFileError> {
        let dir = self.path.parent().ok_or(AtomicFileError::NoParentDirectory)?;
        match std::fs::create_dir(dir) {
            Err(e) if e.kind() != std::io::ErrorKind::AlreadyExists => return Err(io_failure(dir)(e)),
            _ => {}
        }
        let tmp = self.path.with_extension(format!("tmp-{}", std::process::id()));
        let mut opts = std::fs::OpenOptions::new();
        opts.write(true).create(true).truncate(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
            std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o700)).map_err(io_failure(dir))?;
            opts.mode(0o600);
        }
        opts.open(&tmp).and_then(|mut file| file.write_all(bytes)).map_err(io_failure(self.path))?;
        std::fs::rename(&tmp, self.path).map_err(rename_failure(self.path))?;
        Ok(())
    }
}

fn io_failure(path: &Path) -> impl FnOnce(std::io::Error) -> AtomicFileError + '_ {
    move |source| AtomicFileError::Io {
        path: path.to_path_buf(),
        source,
    }
}

fn rename_failure(path: &Path) -> impl FnOnce(std::io::Error) -> AtomicFileError + '_ {
    move |source| AtomicFileError::Rename {
        path: path.to_path_buf(),
        source,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shared::infrastructure::testing::temp_dir::TempDir;

    /// The write_private half of the `_ pre-cmd` run file scenario (the SessionCtx round trip stays in
    /// integrations.rs as `integrations_pre_cmd_file_session_ctx`). The bytes are that SessionCtx, serialized.
    #[cfg(unix)]
    #[test]
    fn integrations_pre_cmd_file() {
        use std::os::unix::fs::PermissionsExt;
        let temp_dir = TempDir::new("precmd");
        let dir = temp_dir.path().to_path_buf();
        let path = dir.join("run").join("abc123.json");
        AtomicFile::new(&path)
            .write_private(br#"{"env":{"A":"1"},"alias":"ll='ls -l'","ts":7}"#)
            .unwrap();
        assert_eq!(
            std::fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o600
        );
        assert_eq!(
            std::fs::metadata(dir.join("run"))
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o700
        );
        // A purged data root stays gone: a running session's next prompt does not recreate it.
        assert!(AtomicFile::new(&dir.join("purged").join("run").join("abc123.json"))
            .write_private(b"{}")
            .is_err());
        assert!(!dir.join("purged").exists());
        let _ = std::fs::remove_dir_all(dir);
    }
}
