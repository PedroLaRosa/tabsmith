use crate::modules::ghost_text::domain::value_objects::llm_prompt::LlmPrompt;
use crate::modules::history::domain::entities::history_entry::HistoryEntry;
use crate::modules::shared::domain::value_objects::shell::Shell;
use std::path::Path;

/// Snapshot for an LLM worker: the last `count` (`inline.historyCount`) entries with an exit code that pass the
/// privacy filter, oldest → newest.
pub(crate) fn llm_history(entries: &[HistoryEntry], count: usize) -> Vec<HistoryEntry> {
    let mut v: Vec<HistoryEntry> = entries
        .iter()
        .rev()
        .filter(|e| usable(e))
        .take(count)
        .cloned()
        .collect();
    v.reverse();
    v
}

pub(crate) const SYSTEM: &str = "You complete shell commands. The user message shows the user's recent shell history in \
`history` format; the last numbered line is the command being typed. Reply with ONLY the characters that should be \
appended to that last line to complete it: one line, no explanation, no quotes, no code fences. If unsure, reply \
with nothing.";

const PROMPT_MAX_BYTES: usize = 10_240;
const PRIVATE_WORDS: [&str; 8] = [
    "password",
    "passwd",
    "secret",
    "token",
    "api_key",
    "apikey",
    "authorization:",
    "bearer ",
];

fn is_private(s: &str) -> bool {
    let s = s.to_lowercase();
    PRIVATE_WORDS.iter().any(|w| s.contains(w))
}

fn usable(e: &HistoryEntry) -> bool {
    e.exit_code().is_some() && !is_private(e.command())
}

/// History-style prompt (SPEC §8.5): usable entries then the buffer, numbered from 1, oldest dropped until
/// ≤ 10 240 bytes. `None` when the buffer is private or alone too long.
fn history_prompt(history: &[HistoryEntry], buffer: &str) -> Option<String> {
    if is_private(buffer) {
        return None;
    }
    let mut lines: Vec<&str> = history
        .iter()
        .filter(|e| usable(e))
        .map(|e| e.command())
        .collect();
    lines.push(buffer);
    // Every line costs `{:>5}  ` + "\n" = 8 bytes (numbers < 100 000), so drop by arithmetic, then format once.
    let cost = |c: &str| c.len() + 8;
    let mut total = lines.iter().map(|c| cost(c)).sum::<usize>() - 1;
    let mut skip = 0;
    while total > PROMPT_MAX_BYTES {
        if skip + 1 == lines.len() {
            return None;
        }
        total -= cost(lines[skip]);
        skip += 1;
    }
    let numbered: Vec<String> = lines[skip..]
        .iter()
        .enumerate()
        .map(|(i, c)| format!("{:>5}  {c}", i + 1))
        .collect();
    Some(numbered.join("\n"))
}

/// The prompt for one request: the OS/shell/cwd header above the numbered history. `None` when the buffer is private
/// or alone too long (no request is made then).
pub(crate) fn llm_prompt(history: &[HistoryEntry], buffer: &str, cwd: &Path, shell: Shell) -> Option<LlmPrompt> {
    let prompt = history_prompt(history, buffer)?;
    let user = format!(
        "OS: {}\nShell: {}\nCWD: {}\n\n{prompt}",
        std::env::consts::OS,
        shell.name(),
        cwd.display()
    );
    Some(LlmPrompt::new(SYSTEM.to_string(), user))
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
    fn prompt_numbering_and_filters() {
        let h = [
            entry("echo hello", Some(0)),
            entry("never finished", None),
            entry("export API_KEY=1", Some(0)),
            entry("curl -H 'Authorization: x'", Some(0)),
            entry("echo world", Some(1)),
        ];
        assert_eq!(
            history_prompt(&h, "echo ").unwrap(),
            "    1  echo hello\n    2  echo world\n    3  echo "
        );
        assert_eq!(history_prompt(&h, "mysql --password=x"), None);
        assert_eq!(history_prompt(&[], "ls").unwrap(), "    1  ls");
    }

    #[test]
    fn prompt_truncation() {
        let big = "a".repeat(3000);
        let h: Vec<_> = (0..5)
            .map(|i| entry(&format!("{i}{big}"), Some(0)))
            .collect();
        let p = history_prompt(&h, "ls").unwrap();
        assert!(p.len() <= PROMPT_MAX_BYTES, "{}", p.len());
        // 3 history lines (3008 bytes each incl. prefix) + buffer fit; the oldest two are dropped.
        assert!(p.starts_with("    1  2aaa"));
        assert!(p.ends_with("\n    4  ls"));
        // Exactly at the limit is kept.
        let exact = "b".repeat(PROMPT_MAX_BYTES - 7);
        assert_eq!(
            history_prompt(&[entry("x", Some(0))], &exact)
                .unwrap()
                .len(),
            PROMPT_MAX_BYTES
        );
        assert_eq!(history_prompt(&[], &"c".repeat(PROMPT_MAX_BYTES)), None);
    }

    #[test]
    fn llm_history_snapshot() {
        let h = [
            entry("a", Some(0)),
            entry("b", None),
            entry("c token", Some(0)),
            entry("d", Some(2)),
            entry("e", Some(0)),
        ];
        let cmds: Vec<_> = llm_history(&h, 2).into_iter().map(|e| e.command().to_string()).collect();
        assert_eq!(cmds, ["d", "e"]);
        assert_eq!(llm_history(&h, 49).len(), 3);
    }

    #[test]
    fn should_send_the_os_shell_and_cwd_above_the_numbered_history() {
        let history = [entry("echo hi", Some(0))];

        let prompt = llm_prompt(&history, "ec", Path::new("/w"), Shell::Zsh);

        assert_eq!(
            prompt,
            Some(LlmPrompt::new(
                SYSTEM.to_string(),
                format!(
                    "OS: {}\nShell: zsh\nCWD: {}\n\n    1  echo hi\n    2  ec",
                    std::env::consts::OS,
                    Path::new("/w").display()
                )
            ))
        );
    }
}
