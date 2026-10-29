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

    /// Written to a temp file then renamed over the symlink target (a symlinked file stays a symlink). Missing
    /// parent directories are created; Unix mode 0600, best effort (a failed chmod does not fail the write).
    pub(crate) fn write_document(&self, bytes: &[u8]) -> Result<(), AtomicFileError> {
        if let Some(dir) = self.path.parent() {
            std::fs::create_dir_all(dir).map_err(io_failure(dir))?;
        }
        let target = std::fs::canonicalize(self.path).unwrap_or_else(|_| self.path.to_path_buf());
        let tmp = target.with_extension(format!("tmp-{}", std::process::id()));
        std::fs::write(&tmp, bytes).map_err(io_failure(self.path))?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = std::fs::set_permissions(&tmp, std::fs::Permissions::from_mode(0o600));
        }
        std::fs::rename(&tmp, &target).map_err(rename_failure(self.path))?;
        Ok(())
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

    /// Write through symlinks, creating the parent directory. Atomic (temp file + rename, permissions kept): a failed or
    /// concurrent write never leaves the user's file empty or torn. A file we may not write stays PermissionDenied.
    /// ponytail: a hard-linked dotfile is split (rename); write in place when nlink > 1 if anyone links dotfiles.
    pub(crate) fn write_keeping_permissions(&self, bytes: &[u8]) -> Result<(), AtomicFileError> {
        // A dangling symlink (dotfiles checkout not populated yet) has no canonical path: write where it points.
        let target = std::fs::canonicalize(self.path).unwrap_or_else(|_| match std::fs::read_link(self.path) {
            Ok(link) => self.path.parent().unwrap_or(Path::new("")).join(link),
            Err(_) => self.path.to_path_buf(),
        });
        if let Some(dir) = target.parent() {
            std::fs::create_dir_all(dir).map_err(io_failure(dir))?;
        }
        let old = std::fs::metadata(&target).ok();
        if old.is_some() {
            std::fs::OpenOptions::new()
                .write(true)
                .open(&target)
                .map_err(io_failure(self.path))?;
        }
        let tmp = target.with_extension(format!("tabsmith-tmp-{}", std::process::id()));
        let r = std::fs::write(&tmp, bytes)
            .and_then(|()| old.map_or(Ok(()), |m| std::fs::set_permissions(&tmp, m.permissions())))
            .and_then(|()| std::fs::rename(&tmp, &target));
        if r.is_err() {
            let _ = std::fs::remove_file(&tmp);
        }
        r.map_err(io_failure(self.path))
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
    #[cfg(unix)]
    use crate::shared::infrastructure::json_document_store::JsonDocumentStore;
    #[cfg(unix)]
    use crate::shared::infrastructure::testing::temp_dir::TempDir;
    #[cfg(unix)]
    use serde_json::Map;

    /// The write_private half of the `_ pre-cmd` run file scenario (the session context round trip is
    /// `integrations_pre_cmd_file_session_ctx` in terminal_session's file_session_context_repository.rs). The bytes
    /// are that session context's run file.
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

    /// Dotfiles are replaced atomically: whoever holds the old file sees it whole, a symlinked dotfile stays a
    /// symlink, permissions stay, no temp file is left, and a read-only file still reports PermissionDenied.
    /// The write half of this scenario; installing and uninstalling an `Integration` on it is
    /// `integrations_dotfile_write_is_atomic_install` in shell_integration's file_dotfile_repository/tests.rs.
    #[cfg(unix)]
    #[test]
    fn integrations_dotfile_write_is_atomic() {
        use crate::shared::infrastructure::os::process_info::is_root;
        use std::os::unix::fs::PermissionsExt;
        let temp_dir = TempDir::new("atomic");
        let dir = temp_dir.path().to_path_buf();
        let home = dir.join("home");
        let real = dir.join("dotfiles").join("zshrc");
        std::fs::create_dir_all(real.parent().unwrap()).unwrap();
        std::fs::create_dir_all(&home).unwrap();
        std::fs::write(&real, "export A=1\n").unwrap();
        std::fs::set_permissions(&real, std::fs::Permissions::from_mode(0o600)).unwrap();
        let zshrc = home.join(".zshrc");
        std::os::unix::fs::symlink(&real, &zshrc).unwrap();
        let old = dir.join("old-zshrc");
        std::fs::hard_link(&real, &old).unwrap(); // the old inode, as a concurrent reader sees it
        let contents = "# tabsmith pre block\nexport A=1\n";
        AtomicFile::new(&zshrc)
            .write_keeping_permissions(contents.as_bytes())
            .unwrap();
        assert_eq!(
            std::fs::read_to_string(&old).unwrap(),
            "export A=1\n",
            "rewritten in place"
        );
        assert!(std::fs::symlink_metadata(&zshrc)
            .unwrap()
            .file_type()
            .is_symlink());
        assert_eq!(std::fs::read_to_string(&real).unwrap(), contents);
        assert_eq!(
            std::fs::metadata(&real).unwrap().permissions().mode() & 0o777,
            0o600
        );
        assert_eq!(
            std::fs::read_dir(real.parent().unwrap()).unwrap().count(),
            1,
            "temp file left behind"
        );
        if !is_root() {
            std::fs::set_permissions(&real, std::fs::Permissions::from_mode(0o444)).unwrap();
            assert!(matches!(
                AtomicFile::new(&zshrc).write_keeping_permissions(b"export A=2\n"),
                Err(AtomicFileError::Io { ref path, ref source })
                    if path == &zshrc && source.kind() == std::io::ErrorKind::PermissionDenied
            ));
            assert_eq!(std::fs::read_to_string(&real).unwrap(), contents);
        }
        // A dangling relative symlink (dotfiles checkout not populated yet): the link stays, its target is created.
        let bashrc = home.join(".bashrc");
        std::os::unix::fs::symlink("../dotfiles/bashrc", &bashrc).unwrap();
        AtomicFile::new(&bashrc)
            .write_keeping_permissions(contents.as_bytes())
            .unwrap();
        assert!(std::fs::symlink_metadata(&bashrc)
            .unwrap()
            .file_type()
            .is_symlink());
        assert_eq!(
            std::fs::read_to_string(dir.join("dotfiles/bashrc")).unwrap(),
            contents
        );
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn should_refuse_a_private_write_to_a_path_without_a_parent() {
        let file = AtomicFile::new(Path::new(""));

        let error = file.write_private(b"{}").err().unwrap();

        assert_eq!(format!("{:#}", anyhow::Error::from(error)), "no parent directory");
    }

    #[cfg(unix)]
    #[test]
    fn should_report_a_failed_private_write_as_the_bare_io_error() {
        let temp_dir = TempDir::new("private-missing-root");
        let path = temp_dir.path().join("purged").join("run").join("abc123.json");

        let error = AtomicFile::new(&path).write_private(b"{}").err().unwrap();

        assert_eq!(
            format!("{:#}", anyhow::Error::from(error)),
            "No such file or directory (os error 2)"
        );
    }

    #[cfg(unix)]
    #[test]
    fn save_keeps_a_symlinked_file() {
        let temp_dir = crate::shared::infrastructure::testing::temp_dir::TempDir::new("settings-symlink");
        let dir = temp_dir.path().to_path_buf();
        std::fs::create_dir_all(dir.join("dots")).unwrap();
        std::fs::write(dir.join("dots/settings.json"), "{\"a\":1}").unwrap();
        let link = dir.join("settings.json");
        std::os::unix::fs::symlink("dots/settings.json", &link).unwrap();
        let mut s = JsonDocumentStore::load(&link).unwrap();
        s.map.insert("b".into(), 2.into());
        s.save().unwrap();
        assert!(std::fs::symlink_metadata(&link)
            .unwrap()
            .file_type()
            .is_symlink());
        assert_eq!(
            JsonDocumentStore::load(&dir.join("dots/settings.json"))
                .unwrap()
                .map
                .len(),
            2
        );
        // A plain (or missing) file is still written in place.
        let plain = JsonDocumentStore {
            path: dir.join("state.json"),
            map: Map::new(),
        };
        plain.save().unwrap();
        assert!(dir.join("state.json").is_file());
        // The settings commands create a missing data root (only running sessions must not, R4-3).
        let fresh = JsonDocumentStore {
            path: dir.join("new/settings.json"),
            map: Map::new(),
        };
        fresh.save().unwrap();
        assert!(dir.join("new/settings.json").is_file());
        let _ = std::fs::remove_dir_all(dir);
    }
}
