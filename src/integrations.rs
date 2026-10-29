//! The internal `_ pre-cmd` command (SPEC §2.11): saves the session context the shell reports and prunes stale run
//! files.
use crate::shared::infrastructure::atomic_file::AtomicFile;
use crate::types::{now_ms, SessionCtx};
use anyhow::Context;
use std::path::Path;
use std::time::Duration;

pub fn pre_cmd(alias: Option<String>) -> anyhow::Result<i32> {
    // The id becomes a file name: accept only what the wrapper generates (hex), never a path.
    let Some(sid) = std::env::var("TABSMITH_SESSION_ID").ok().filter(|s| {
        !s.is_empty()
            && s.bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
    }) else {
        return Ok(1);
    };
    let env = std::env::vars_os()
        .filter_map(|(k, v)| Some((k.into_string().ok()?, v.into_string().ok()?)))
        .collect();
    let ctx = SessionCtx {
        env,
        alias,
        ts: now_ms(),
    };
    let path = SessionCtx::path(&sid)?;
    AtomicFile::new(&path).write_private(serde_json::to_string(&ctx)?.as_bytes())?;
    prune_run_files(path.parent().context("no parent directory")?);
    Ok(0)
}

/// Delete run files untouched for a day. A shell that exits right after a prompt leaves one behind: its background
/// `_ pre-cmd` writes the file again after the wrapper removed it. A live session rewrites its file at every prompt.
fn prune_run_files(dir: &Path) {
    for e in std::fs::read_dir(dir).into_iter().flatten().flatten() {
        let stale = e.metadata().is_ok_and(|m| {
            m.is_file()
                && m.modified()
                    .is_ok_and(|t| t.elapsed().unwrap_or_default() > Duration::from_secs(24 * 3600))
        });
        if stale {
            let _ = std::fs::remove_file(e.path());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shared::infrastructure::testing::temp_dir::TempDir;

    /// The file modes and the purge guard of this scenario are asserted in shared atomic_file.rs
    /// (`integrations_pre_cmd_file`); this half keeps the SessionCtx round trip.
    #[cfg(unix)]
    #[test]
    fn integrations_pre_cmd_file_session_ctx() {
        let temp_dir = TempDir::new("precmd");
        let dir = temp_dir.path().to_path_buf();
        let path = dir.join("run").join("abc123.json");
        let ctx = SessionCtx {
            env: [("A".to_string(), "1".to_string())].into(),
            alias: Some("ll='ls -l'".into()),
            ts: 7,
        };
        AtomicFile::new(&path).write_private(serde_json::to_string(&ctx).unwrap().as_bytes()).unwrap();
        let back = SessionCtx::load(&path).unwrap();
        assert_eq!(
            (back.env["A"].as_str(), back.alias.as_deref(), back.ts),
            ("1", Some("ll='ls -l'"), 7)
        );
        let _ = std::fs::remove_dir_all(dir);
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
