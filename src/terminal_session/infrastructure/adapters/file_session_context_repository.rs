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

    fn run_file(&self, id: &SessionId) -> Result<PathBuf, SharedError> {
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
        let path = self.run_file(context.id()).map_err(storage_error)?;
        let json =
            serde_json::to_string(&SessionContextRecord::from(context)).map_err(storage_error)?;
        AtomicFile::new(&path)
            .write_private(json.as_bytes())
            .map_err(storage_error)
    }

    /// `None` for a file that is missing, unreadable, not UTF-8 or not a record (`"env": null` too), as
    /// `SessionCtx::load` was.
    fn find(&self, id: &SessionId) -> Option<SessionContext> {
        let path = self.run_file(id).ok()?;
        let record: SessionContextRecord =
            serde_json::from_str(&std::fs::read_to_string(path).ok()?).ok()?;
        Some(record.into_context(id.clone()))
    }

    /// Follows a symlinked run file, as the pty's `mtime` did; `None` when the file or its path is unknown.
    fn last_modified(&self, id: &SessionId) -> Option<SystemTime> {
        let path = self.run_file(id).ok()?;
        std::fs::metadata(path).ok()?.modified().ok()
    }

    /// A symlinked run file loses the link, not its target; every error is ignored.
    fn remove(&self, id: &SessionId) {
        if let Ok(path) = self.run_file(id) {
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

/// Delete the regular files of `dir` untouched for longer than `max_age` (why they are pruned: see
/// `ReportSessionContextUseCase`).
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

    fn empty_context(id: &str) -> SessionContext {
        SessionContext::new(session_id(id), HashMap::new(), None, 0)
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
        let context = empty_context("abc123");
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
        let context = empty_context("abc123");
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

    #[test]
    fn should_save_no_alias_as_null() {
        let temp_dir = TempDir::new("session-no-alias");
        let repository = repository_in(&temp_dir);
        let context = SessionContext::new(session_id("abc123"), HashMap::new(), None, 7);

        repository.save(&context).unwrap();

        assert_eq!(
            std::fs::read_to_string(temp_dir.path().join("run/abc123.json")).unwrap(),
            r#"{"env":{},"alias":null,"ts":7}"#
        );
    }

    /// `#[serde(default)]` does not take an explicit null, so `{"env":null}` is no record either.
    #[test]
    fn should_find_nothing_in_a_missing_run_file_bad_json_or_a_null_env() {
        let temp_dir = TempDir::new("session-unreadable");
        let repository = repository_in(&temp_dir);
        let run = temp_dir.path().join("run");
        std::fs::create_dir(&run).unwrap();
        std::fs::write(run.join("bad.json"), "{oops").unwrap();
        std::fs::write(run.join("nulled.json"), r#"{"env":null}"#).unwrap();

        let found =
            ["missing", "bad", "nulled"].map(|id| repository.find(&session_id(id)).is_some());

        assert_eq!(found, [false; 3]);
    }

    /// `_ pre-cmd` with a valid id and no home prints this through main's `{:#}`.
    #[test]
    fn should_fail_to_save_without_a_home_directory_with_the_original_message() {
        let repository = FileSessionContextRepository::new(DataDirectory::new(None, None));
        let context = empty_context("abc123");

        let error = repository.save(&context).unwrap_err();

        assert_eq!(
            format!("{:#}", anyhow::Error::from(error)),
            "could not determine the home directory (set HOME or TABSMITH_HOME)"
        );
    }

    /// A running session's next prompt does not bring back a data directory `uninstall --purge` removed.
    #[cfg(unix)]
    #[test]
    fn should_fail_to_save_into_a_purged_data_directory_without_recreating_it() {
        let temp_dir = TempDir::new("session-purged");
        let root = temp_dir.path().join("purged");
        let repository =
            FileSessionContextRepository::new(DataDirectory::new(Some(root.clone()), None));
        let context = empty_context("abc123");

        let error = repository.save(&context).unwrap_err();

        assert_eq!(
            (format!("{:#}", anyhow::Error::from(error)), root.exists()),
            ("No such file or directory (os error 2)".to_string(), false)
        );
    }

    #[cfg(unix)]
    #[test]
    fn should_report_a_failed_save_as_writing_the_run_file() {
        let temp_dir = TempDir::new("session-unwritable");
        let repository = repository_in(&temp_dir);
        let path = temp_dir.path().join("run/abc123.json");
        std::fs::create_dir_all(&path).unwrap();
        let context = empty_context("abc123");

        let error = repository.save(&context).unwrap_err();

        assert_eq!(
            format!("{:#}", anyhow::Error::from(error)),
            format!("writing {}: Is a directory (os error 21)", path.display())
        );
    }

    /// The entry's own metadata is read: a link is not a regular file, so neither it nor its stale target goes.
    #[cfg(unix)]
    #[test]
    fn should_keep_a_link_to_a_stale_file_when_pruning() {
        let temp_dir = TempDir::new("session-prune-link");
        let repository = repository_in(&temp_dir);
        let run = temp_dir.path().join("run");
        let (stale, target, link) = (
            run.join("stale.json"),
            temp_dir.path().join("old-target"),
            run.join("link.json"),
        );
        let two_days_ago = SystemTime::now() - Duration::from_secs(2 * 24 * 3600);
        std::fs::create_dir(&run).unwrap();
        std::fs::write(&stale, "{}").unwrap();
        std::fs::write(&target, "{}").unwrap();
        set_modified(&stale, two_days_ago);
        set_modified(&target, two_days_ago);
        std::os::unix::fs::symlink(&target, &link).unwrap();

        repository.prune_older_than(Duration::from_secs(24 * 3600));

        assert_eq!(
            (
                stale.exists(),
                link.symlink_metadata().is_ok(),
                target.exists()
            ),
            (false, true, true)
        );
    }

    /// The pty still runs without a home: it just never finds, reloads or removes a context.
    #[test]
    fn should_find_nothing_and_remove_nothing_without_a_home_directory() {
        let repository = FileSessionContextRepository::new(DataDirectory::new(None, None));

        repository.remove(&session_id("abc123"));
        repository.prune_older_than(Duration::from_secs(24 * 3600));

        assert_eq!(
            (
                repository.find(&session_id("abc123")).is_none(),
                repository.last_modified(&session_id("abc123"))
            ),
            (true, None)
        );
    }

    /// The file modes and the purge guard of this scenario are asserted in shared atomic_file.rs
    /// (`integrations_pre_cmd_file`); this half keeps the SessionCtx round trip.
    #[cfg(unix)]
    #[test]
    fn integrations_pre_cmd_file_session_ctx() {
        let temp_dir = TempDir::new("precmd");
        let dir = temp_dir.path().to_path_buf();
        let repository = FileSessionContextRepository::new(DataDirectory::new(Some(dir.clone()), None));
        let ctx = SessionContext::new(
            SessionId::new("abc123".to_string()).unwrap(),
            [("A".to_string(), "1".to_string())].into(),
            Some("ll='ls -l'".into()),
            7,
        );
        repository.save(&ctx).unwrap();
        let back = repository.find(ctx.id()).unwrap();
        assert_eq!(
            (back.env()["A"].as_str(), back.alias(), back.reported_at_ms()),
            ("1", Some("ll='ls -l'"), 7)
        );
        let _ = std::fs::remove_dir_all(dir);
    }

    /// The prune `_ pre-cmd` runs after each save, over a bare dir, for `integrations_prune_run_files`.
    fn prune_run_files(dir: &Path) {
        prune_files_older_than(dir, Duration::from_secs(24 * 3600));
    }

    /// Run files untouched for a day go (a quick exit can leave one behind); fresh files and directories stay.
    #[test]
    fn integrations_prune_run_files() {
        let temp_dir = TempDir::new("prune");
        let dir = temp_dir.path().to_path_buf();
        let (old, new, sub) = (
            dir.join("old.json"),
            dir.join("new.json"),
            dir.join("debug-shell-1"),
        );
        std::fs::write(&old, "{}").unwrap();
        std::fs::write(&new, "{}").unwrap();
        std::fs::create_dir(&sub).unwrap();
        let day_ago = std::time::SystemTime::now() - Duration::from_secs(2 * 24 * 3600);
        std::fs::File::options()
            .write(true)
            .open(&old)
            .unwrap()
            .set_modified(day_ago)
            .unwrap();
        prune_run_files(&dir);
        assert!(!old.exists() && new.exists() && sub.exists());
        let _ = std::fs::remove_dir_all(dir);
    }
}
