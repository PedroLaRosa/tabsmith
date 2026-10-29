use crate::shared::domain::shared_error::SharedError;
use crate::shared::domain::value_objects::data_directory::DataDirectory;
use crate::shared::infrastructure::atomic_file::AtomicFile;
use crate::terminal_session::domain::entities::session_context::SessionContext;
use crate::terminal_session::domain::repositories::session_context_repository::SessionContextRepository;
use crate::terminal_session::domain::terminal_session_error::TerminalSessionError;
use crate::terminal_session::domain::value_objects::session_id::SessionId;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

/// `run/<session id>.json` in the data directory. The path is resolved on every call, so a missing home only fails
/// the calls that need it.
pub(crate) struct FileSessionContextRepository {
    data_directory: DataDirectory,
}

impl FileSessionContextRepository {
    pub(crate) fn new(data_directory: DataDirectory) -> Self {
        FileSessionContextRepository { data_directory }
    }

    fn file(&self, id: &SessionId) -> Result<PathBuf, SharedError> {
        Ok(self
            .data_directory
            .run_dir()?
            .join(format!("{}.json", id.as_str())))
    }
}

impl SessionContextRepository for FileSessionContextRepository {
    /// Compact JSON written with [`AtomicFile::write_private`]: `run/` 0700 (a purged data directory is not
    /// recreated), the file 0600.
    fn save(&self, context: &SessionContext) -> Result<(), TerminalSessionError> {
        let path = self.file(context.id()).map_err(storage_error)?;
        let json =
            serde_json::to_string(&SessionContextRecord::from(context)).map_err(storage_error)?;
        AtomicFile::new(&path)
            .write_private(json.as_bytes())
            .map_err(storage_error)
    }

    /// `None` for a file that is missing, unreadable, not UTF-8 or not a record (`"env": null` too), as
    /// `SessionCtx::load` was.
    fn find(&self, id: &SessionId) -> Option<SessionContext> {
        let path = self.file(id).ok()?;
        let record: SessionContextRecord =
            serde_json::from_str(&std::fs::read_to_string(path).ok()?).ok()?;
        Some(record.into_context(id.clone()))
    }

    /// Follows a symlinked run file, as the pty's `mtime` did; `None` when the file or its path is unknown.
    fn last_modified(&self, id: &SessionId) -> Option<SystemTime> {
        let path = self.file(id).ok()?;
        std::fs::metadata(path).ok()?.modified().ok()
    }

    /// A symlinked run file loses the link, not its target; every error is ignored.
    fn remove(&self, id: &SessionId) {
        if let Ok(path) = self.file(id) {
            let _ = std::fs::remove_file(path);
        }
    }

    /// Every regular file of `run/` (a stray temp file too) by its own mtime, measured with
    /// `SystemTime::elapsed`; symlinks and directories stay, every error is ignored.
    fn prune_older_than(&self, max_age: Duration) {
        if let Ok(dir) = self.data_directory.run_dir() {
            prune_files_older_than(&dir, max_age);
        }
    }
}

/// `~/.tabsmith/run/<sid>.json`, written by `tabsmith _ pre-cmd` (SPEC §2.11, D17).
#[derive(Serialize, Deserialize)]
struct SessionContextRecord {
    #[serde(default)]
    env: HashMap<String, String>,
    #[serde(default)]
    alias: Option<String>,
    #[serde(default)]
    ts: u64,
}

impl SessionContextRecord {
    fn into_context(self, id: SessionId) -> SessionContext {
        SessionContext::new(id, self.env, self.alias, self.ts)
    }
}

impl From<&SessionContext> for SessionContextRecord {
    fn from(context: &SessionContext) -> Self {
        SessionContextRecord {
            env: context.env().clone(),
            alias: context.alias().map(str::to_owned),
            ts: context.reported_at_ms(),
        }
    }
}

/// Delete the regular files of `dir` untouched for longer than `max_age`. A shell that exits right after a prompt
/// leaves one behind: its background `_ pre-cmd` writes the file again after the wrapper removed it. A live session
/// rewrites its file at every prompt.
fn prune_files_older_than(dir: &Path, max_age: Duration) {
    for e in std::fs::read_dir(dir).into_iter().flatten().flatten() {
        let stale = e.metadata().is_ok_and(|m| {
            m.is_file()
                && m.modified()
                    .is_ok_and(|t| t.elapsed().unwrap_or_default() > max_age)
        });
        if stale {
            let _ = std::fs::remove_file(e.path());
        }
    }
}

fn storage_error(error: impl std::error::Error + Send + Sync + 'static) -> TerminalSessionError {
    TerminalSessionError::Storage(Box::new(error))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shared::infrastructure::testing::temp_dir::TempDir;

    fn repository_in(temp_dir: &TempDir) -> FileSessionContextRepository {
        FileSessionContextRepository::new(DataDirectory::new(
            Some(temp_dir.path().to_path_buf()),
            None,
        ))
    }

    fn session_id(id: &str) -> SessionId {
        SessionId::new(id.to_string()).unwrap()
    }

    /// The bytes `_ pre-cmd` has always written: compact, no trailing newline, keys env, alias, ts in that order.
    #[test]
    fn should_save_the_context_as_compact_json_with_env_alias_and_ts_in_that_order() {
        let temp_dir = TempDir::new("session-save");
        let repository = repository_in(&temp_dir);
        let context = SessionContext::new(
            session_id("abc123"),
            HashMap::from([("A".to_string(), "1".to_string())]),
            Some("ll='ls -l'".to_string()),
            7,
        );

        repository.save(&context).unwrap();

        assert_eq!(
            std::fs::read_to_string(temp_dir.path().join("run/abc123.json")).unwrap(),
            r#"{"env":{"A":"1"},"alias":"ll='ls -l'","ts":7}"#
        );
    }

    /// Every field has its own default, so a partial file still reads.
    #[test]
    fn should_find_an_empty_record_as_no_env_no_alias_reported_at_zero() {
        let temp_dir = TempDir::new("session-empty");
        let repository = repository_in(&temp_dir);
        std::fs::create_dir(temp_dir.path().join("run")).unwrap();
        std::fs::write(temp_dir.path().join("run/abc123.json"), "{}").unwrap();

        let found = repository.find(&session_id("abc123")).unwrap();

        assert_eq!(
            (
                found.id().as_str(),
                found.env().is_empty(),
                found.alias(),
                found.reported_at_ms()
            ),
            ("abc123", true, None, 0)
        );
    }

    /// The pty reloads the context when this time changes; a session that never reported has none.
    #[test]
    fn should_give_the_modification_time_of_the_run_file_once_the_session_reported() {
        let temp_dir = TempDir::new("session-mtime");
        let repository = repository_in(&temp_dir);
        let context = SessionContext::new(session_id("abc123"), HashMap::new(), None, 0);
        let before = repository.last_modified(&session_id("abc123"));

        repository.save(&context).unwrap();

        let path = temp_dir.path().join("run/abc123.json");
        assert_eq!(
            (before, repository.last_modified(&session_id("abc123"))),
            (None, std::fs::metadata(path).unwrap().modified().ok())
        );
    }

    #[test]
    fn should_delete_the_run_file_of_a_removed_session() {
        let temp_dir = TempDir::new("session-remove");
        let repository = repository_in(&temp_dir);
        let context = SessionContext::new(session_id("abc123"), HashMap::new(), None, 0);
        repository.save(&context).unwrap();

        repository.remove(&session_id("abc123"));

        assert!(!temp_dir.path().join("run/abc123.json").exists());
    }

    fn set_modified(path: &Path, modified: SystemTime) {
        std::fs::File::options()
            .write(true)
            .open(path)
            .unwrap()
            .set_modified(modified)
            .unwrap();
    }

    /// A clock set back leaves a file modified in the future: it is not stale.
    #[test]
    fn should_keep_a_run_file_modified_in_the_future_when_pruning() {
        let temp_dir = TempDir::new("session-prune-future");
        let repository = repository_in(&temp_dir);
        let run = temp_dir.path().join("run");
        let (stale, future) = (run.join("stale.json"), run.join("future.json"));
        let two_days = Duration::from_secs(2 * 24 * 3600);
        std::fs::create_dir(&run).unwrap();
        std::fs::write(&stale, "{}").unwrap();
        std::fs::write(&future, "{}").unwrap();
        set_modified(&stale, SystemTime::now() - two_days);
        set_modified(&future, SystemTime::now() + two_days);

        repository.prune_older_than(Duration::from_secs(24 * 3600));

        assert_eq!((stale.exists(), future.exists()), (false, true));
    }
}
