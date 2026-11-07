use crate::modules::history::domain::entities::history_entry::HistoryEntry;

/// History source (SPEC §6.11): newest successful entry extending `buffer` → the remaining suffix.
pub(crate) fn history_ghost(history: &[HistoryEntry], buffer: &str) -> Option<String> {
    if buffer.is_empty() {
        return None;
    }
    let newest = |ok_only: bool| {
        history.iter().rev().find(|e| {
            e.command().len() > buffer.len()
                && e.command().starts_with(buffer)
                && (!ok_only || e.exit_code() == Some(0))
        })
    };
    newest(true)
        .or_else(|| newest(false))
        .map(|e| e.command()[buffer.len()..].to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::history::domain::entities::history_entry::HistoryEntryPrimitives;

    fn entry(cmd: &str, exit: Option<i32>) -> HistoryEntry {
        HistoryEntry::new(HistoryEntryPrimitives {
            command: cmd.into(),
            shell: "zsh".into(),
            exit_code: exit,
            ..Default::default()
        })
    }

    #[test]
    fn history_ghost_rules() {
        let h = [
            entry("git status", Some(0)),
            entry("git stash", Some(1)),
            entry("git switch", None),
        ];
        assert_eq!(
            history_ghost(&h, "git st").as_deref(),
            Some("atus"),
            "newest successful wins"
        );
        assert_eq!(
            history_ghost(&h, "git sw").as_deref(),
            Some("itch"),
            "fallback: any"
        );
        assert_eq!(history_ghost(&h, "git status"), None);
        assert_eq!(history_ghost(&h, ""), None);
        assert_eq!(history_ghost(&h, "ls"), None);
        let h2 = [entry("git stash", Some(0)), entry("git status", Some(0))];
        assert_eq!(history_ghost(&h2, "git st").as_deref(), Some("atus"));
    }
}
