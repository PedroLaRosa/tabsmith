use crate::history::domain::entities::history_entry::{HistoryEntry, HistoryEntryPrimitives};
use crate::history::domain::repositories::history_repository::{
    HistoryRepository, InMemoryHistoryRepository, HISTORY_IN_MEMORY,
};
use crate::shared::infrastructure::logger;
use serde::{Deserialize, Serialize};
use std::cell::Cell;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::rc::Rc;

/// One line of `~/.tabsmith/history.jsonl` (SPEC §3.5). The wire format is frozen: fields in this order, `None`
/// written as `null`, and no defaults, so a line missing `command`, `shell`, `session_id` or `start_time` does not
/// load. Unknown fields are ignored.
#[derive(Serialize, Deserialize)]
struct HistoryRecord {
    command: String,
    shell: String,
    pid: Option<i32>,
    session_id: String,
    cwd: Option<String>,
    start_time: u64,
    end_time: Option<u64>,
    exit_code: Option<i32>,
    hostname: Option<String>,
}

impl From<&HistoryEntry> for HistoryRecord {
    fn from(entry: &HistoryEntry) -> Self {
        let HistoryEntryPrimitives {
            command,
            shell,
            pid,
            session_id,
            cwd,
            start_time,
            end_time,
            exit_code,
            hostname,
        } = entry.to_primitives();
        HistoryRecord {
            command,
            shell,
            pid,
            session_id,
            cwd,
            start_time,
            end_time,
            exit_code,
            hostname,
        }
    }
}

impl From<HistoryRecord> for HistoryEntry {
    fn from(record: HistoryRecord) -> Self {
        let HistoryRecord {
            command,
            shell,
            pid,
            session_id,
            cwd,
            start_time,
            end_time,
            exit_code,
            hostname,
        } = record;
        HistoryEntry::new(HistoryEntryPrimitives {
            command,
            shell,
            pid,
            session_id,
            cwd,
            start_time,
            end_time,
            exit_code,
            hostname,
        })
    }
}

const HISTORY_COMPACT_AT: usize = 10_000;

/// `~/.tabsmith/history.jsonl` (last 5000 entries in memory; file compacted past 10 000 lines).
pub(crate) struct JsonlHistoryRepository {
    memory: InMemoryHistoryRepository,
    path: PathBuf,
    /// Lines in the file as far as this process knows (other sessions append too).
    lines: Cell<usize>,
}

impl JsonlHistoryRepository {
    /// Load (and later append to) the file at `path` (missing/corrupt lines skipped).
    pub(crate) fn load(path: PathBuf) -> JsonlHistoryRepository {
        let text = std::fs::read_to_string(&path).unwrap_or_default();
        let lines = text.lines().count();
        let mut entries: Vec<HistoryEntry> = text
            .lines()
            .filter_map(|l| serde_json::from_str::<HistoryRecord>(l).ok())
            .map(HistoryEntry::from)
            .collect();
        entries.drain(..entries.len().saturating_sub(HISTORY_IN_MEMORY));
        let h = JsonlHistoryRepository {
            memory: InMemoryHistoryRepository::new(entries),
            path,
            lines: Cell::new(lines),
        };
        if lines > HISTORY_COMPACT_AT {
            h.compact();
        }
        h
    }

    /// Rewrite the file with its last 5000 lines (temp + rename). Re-reads the file so lines appended by other
    /// sessions survive. ponytail: no lock; a line appended between read and rename is lost.
    fn compact(&self) {
        let path = &self.path;
        let res = (|| {
            let text = std::fs::read_to_string(path)?;
            let all: Vec<&str> = text.lines().collect();
            let keep = &all[all.len().saturating_sub(HISTORY_IN_MEMORY)..];
            let tmp = path.with_extension(format!("tmp-{}", std::process::id()));
            open_private(&tmp, false)?.write_all((keep.join("\n") + "\n").as_bytes())?;
            std::fs::rename(&tmp, path)?;
            Ok::<_, std::io::Error>(keep.len())
        })();
        match res {
            Ok(n) => self.lines.set(n),
            Err(e) => logger::warn(&format!("history: compacting {}: {e}", path.display())),
        }
    }
}

impl HistoryRepository for JsonlHistoryRepository {
    /// Append to memory and to the file (non-empty commands only).
    fn append(&self, entry: HistoryEntry) {
        if entry.command().trim().is_empty() {
            return;
        }
        let path = &self.path;
        let line = serde_json::to_string(&HistoryRecord::from(&entry)).unwrap_or_default() + "\n";
        // The data root is gone (`uninstall --purge` while this session runs): don't recreate it.
        let purged = path.parent().is_some_and(|p| !p.is_dir());
        let res = if purged {
            Ok(())
        } else {
            open_private(path, true).and_then(|mut f| f.write_all(line.as_bytes()))
        };
        match res {
            Ok(()) if purged => {}
            Ok(()) => self.lines.set(self.lines.get() + 1),
            Err(e) => {
                logger::warn(&format!("history: appending to {}: {e}", path.display()))
            }
        }
        self.memory.append(entry);
        if self.lines.get() > HISTORY_COMPACT_AT {
            self.compact();
        }
    }

    fn recent(&self) -> Rc<Vec<HistoryEntry>> {
        self.memory.recent()
    }
}

/// Open for append or truncate+write; created with mode 0600 on Unix.
fn open_private(path: &Path, append: bool) -> std::io::Result<std::fs::File> {
    let mut o = std::fs::OpenOptions::new();
    o.create(true);
    if append {
        o.append(true);
    } else {
        o.write(true).truncate(true);
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        o.mode(0o600);
    }
    o.open(path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shared::infrastructure::testing::temp_dir::TempDir;

    fn entry(cmd: &str, exit: Option<i32>) -> HistoryEntry {
        HistoryEntry::new(HistoryEntryPrimitives {
            command: cmd.into(),
            shell: "zsh".into(),
            exit_code: exit,
            ..Default::default()
        })
    }

    fn load(line: &str) -> Option<HistoryEntry> {
        serde_json::from_str::<HistoryRecord>(line)
            .ok()
            .map(HistoryEntry::from)
    }

    #[test]
    fn should_write_every_field_in_order_with_null_for_absent_values() {
        let entry = HistoryEntry::new(HistoryEntryPrimitives {
            command: "ls".into(),
            shell: "zsh".into(),
            pid: Some(7),
            session_id: "s1".into(),
            start_time: 3,
            ..Default::default()
        });

        let line = serde_json::to_string(&HistoryRecord::from(&entry)).unwrap();

        assert_eq!(
            line,
            r#"{"command":"ls","shell":"zsh","pid":7,"session_id":"s1","cwd":null,"start_time":3,"end_time":null,"exit_code":null,"hostname":null}"#
        );
    }

    #[test]
    fn should_drop_a_line_missing_command_shell_session_id_or_start_time() {
        let lines = [
            r#"{"shell":"zsh","session_id":"s1","start_time":3}"#,
            r#"{"command":"ls","session_id":"s1","start_time":3}"#,
            r#"{"command":"ls","shell":"zsh","start_time":3}"#,
            r#"{"command":"ls","shell":"zsh","session_id":"s1"}"#,
        ];

        let loaded: Vec<Option<HistoryEntry>> = lines.iter().map(|l| load(l)).collect();

        assert_eq!(loaded, [None, None, None, None]);
    }

    #[test]
    fn should_load_missing_optional_fields_as_none_and_ignore_unknown_fields() {
        let line = r#"{"command":"ls","shell":"zsh","session_id":"s1","start_time":3,"extra":1}"#;

        let entry = load(line);

        assert_eq!(
            entry,
            Some(HistoryEntry::new(HistoryEntryPrimitives {
                command: "ls".into(),
                shell: "zsh".into(),
                session_id: "s1".into(),
                start_time: 3,
                ..Default::default()
            }))
        );
    }

    #[test]
    fn history_file_round_trip_and_compaction() {
        let temp_dir = TempDir::new("hist");
        let d = temp_dir.path().to_path_buf();
        let path = d.join("sub/history.jsonl");
        // A missing data root (purged while a session runs) is not recreated; the entry stays in memory only.
        JsonlHistoryRepository::load(path.clone()).append(entry("gone", Some(0)));
        assert!(!d.join("sub").exists());
        std::fs::create_dir_all(d.join("sub")).unwrap();
        let h = JsonlHistoryRepository::load(path.clone());
        assert!(h.recent().is_empty());
        h.append(entry("echo hi", Some(0)));
        h.append(entry("   ", Some(0)));
        h.append(HistoryEntry::new(HistoryEntryPrimitives {
            pid: Some(7),
            cwd: Some("/x".into()),
            ..entry("ls", None).to_primitives()
        }));
        let again = JsonlHistoryRepository::load(path.clone());
        assert_eq!(again.recent(), h.recent());
        assert_eq!(again.recent().len(), 2);
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                std::fs::metadata(&path).unwrap().permissions().mode() & 0o777,
                0o600
            );
        }

        // 10 000 lines (one corrupt) stay; the append that goes past 10 000 rewrites the file with the last 5000.
        let lines = |n: usize| -> String {
            (0..n)
                .map(|i| serde_json::to_string(&HistoryRecord::from(&entry(&format!("c{i}"), Some(0)))).unwrap() + "\n")
                .collect()
        };
        std::fs::write(
            &path,
            String::from("not json\n") + &lines(HISTORY_COMPACT_AT - 1),
        )
        .unwrap();
        let h = JsonlHistoryRepository::load(path.clone());
        assert_eq!(h.recent().len(), HISTORY_IN_MEMORY);
        assert_eq!(
            std::fs::read_to_string(&path).unwrap().lines().count(),
            HISTORY_COMPACT_AT,
            "not yet"
        );
        h.append(entry("newest", Some(0)));
        let text = std::fs::read_to_string(&path).unwrap();
        assert_eq!(text.lines().count(), HISTORY_IN_MEMORY);
        assert!(text
            .lines()
            .last()
            .unwrap()
            .contains("\"command\":\"newest\""));
        assert_eq!(h.recent().len(), HISTORY_IN_MEMORY);
        assert_eq!(h.recent().last().unwrap().command(), "newest");
        let reloaded = JsonlHistoryRepository::load(path.clone());
        assert_eq!(reloaded.recent().first().unwrap().command(), "c5000");
        assert_eq!(reloaded.recent().len(), HISTORY_IN_MEMORY);

        // A file already past 10 000 lines is compacted at load.
        std::fs::write(&path, lines(HISTORY_COMPACT_AT + 1)).unwrap();
        let h = JsonlHistoryRepository::load(path.clone());
        assert_eq!(
            std::fs::read_to_string(&path).unwrap().lines().count(),
            HISTORY_IN_MEMORY
        );
        assert_eq!(h.recent().first().unwrap().command(), "c5001");
        assert!(!d.join("sub").read_dir().unwrap().any(|e| e
            .unwrap()
            .file_name()
            .to_string_lossy()
            .contains("tmp")));
        std::fs::remove_dir_all(&d).unwrap();
    }
}
