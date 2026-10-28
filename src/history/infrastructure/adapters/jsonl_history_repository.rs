use crate::history::domain::entities::history_entry::{HistoryEntry, HistoryEntryPrimitives};
use serde::{Deserialize, Serialize};

/// One line of `~/.tabsmith/history.jsonl` (SPEC §3.5). The wire format is frozen: fields in this order, `None`
/// written as `null`, and no defaults, so a line missing `command`, `shell`, `session_id` or `start_time` does not
/// load. Unknown fields are ignored.
#[derive(Serialize, Deserialize)]
pub(crate) struct HistoryRecord {
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

#[cfg(test)]
mod tests {
    use super::*;

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
}
