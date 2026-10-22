//! Inline ghost text: history store, local sources, LLM profiles/clients, completion cache, validation (SPEC §8,
//! §3.5, §6.11). OWNER: inline agent. Used by the engine (sources, LLM worker), the CLI (`inline` command) and
//! doctor (profile checks).
use crate::settings::Settings;
use crate::types::{HistoryEntry, Item, Shell};
use std::collections::HashMap;
use std::path::Path;

/// `~/.tabsmith/history.jsonl` (last 5000 entries in memory; file compacted past 10 000 lines).
pub struct History {
    entries: Vec<HistoryEntry>,
}

impl History {
    /// Load the history file (missing/corrupt lines skipped).
    pub fn load() -> History {
        todo!("inline agent")
    }

    /// In-memory history (tests).
    pub fn from_entries(entries: Vec<HistoryEntry>) -> History {
        History { entries }
    }

    /// Append to memory and to the file (non-empty commands only).
    pub fn append(&mut self, entry: HistoryEntry) {
        let _ = entry;
        todo!("inline agent")
    }

    /// Oldest → newest.
    pub fn entries(&self) -> &[HistoryEntry] {
        &self.entries
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Provider {
    OpenAi,
    Anthropic,
}

/// A user-defined LLM profile `inline.profiles.<name>` (SPEC §3.3).
#[derive(Clone, Debug, PartialEq)]
pub struct Profile {
    pub name: String,
    pub provider: Provider,
    pub model: String,
    pub base_url: String,
    pub api_key_env: String,
    pub max_tokens: u32,
    pub effort: Option<String>,
}

/// The selected inline source (`inline.source`, default `history+specs`).
#[derive(Clone, Debug, PartialEq)]
pub enum Source {
    History,
    Specs,
    HistoryAndSpecs,
    Profile(Profile),
}

/// Built-in source names with descriptions, in display order.
pub const BUILTIN_SOURCES: [(&str, &str); 3] = [
    ("history+specs", "Local history, then completion specs"),
    ("history", "Local history only"),
    ("specs", "Completion specs only"),
];

/// Every `inline.profiles.*` entry, parsed (`Err(reason)` for invalid objects), sorted by name.
pub fn profiles(settings: &Settings) -> Vec<(String, Result<Profile, String>)> {
    let _ = settings;
    todo!("inline agent")
}

/// Resolve `inline.source` (unset → HistoryAndSpecs). `Err` when it names a missing/invalid profile.
pub fn source(settings: &Settings) -> Result<Source, String> {
    let _ = settings;
    todo!("inline agent")
}

/// History source (SPEC §6.11): newest successful entry extending `buffer` → the remaining suffix.
pub fn history_ghost(history: &[HistoryEntry], buffer: &str) -> Option<String> {
    let _ = (history, buffer);
    todo!("inline agent")
}

/// Specs source (SPEC §6.11): first non-auto-execute/history item with `delete == 0`, prefix match and non-empty
/// text (trailing space trimmed).
pub fn spec_ghost(items: &[Item]) -> Option<String> {
    let _ = items;
    todo!("inline agent")
}

/// LLM completion cache (SPEC §8.4), cleared on every new command.
#[derive(Default)]
pub struct GhostCache {
    entries: Vec<(String, f64)>,
}

impl GhostCache {
    pub fn clear(&mut self) {
        self.entries.clear();
    }

    /// Store full texts (`buffer + completion`); the first gets priority 0.0, others 1.0.
    pub fn insert(&mut self, full_texts: &[String]) {
        let _ = full_texts;
        todo!("inline agent")
    }

    pub fn lookup(&self, buffer: &str) -> Option<String> {
        let _ = buffer;
        todo!("inline agent")
    }
}

/// Blocking LLM request (worker thread): prompt with privacy filter, HTTP, cleaning, validation (SPEC §8.5).
/// `None` on any error (logged) or when nothing should be suggested.
pub fn llm_complete(
    profile: &Profile,
    buffer: &str,
    cwd: &Path,
    shell: Shell,
    history: &[HistoryEntry],
    env: &HashMap<String, String>,
) -> Option<String> {
    let _ = (profile, buffer, cwd, shell, history, env);
    todo!("inline agent")
}

/// `cd` validation (SPEC §8.5): `full` = buffer + completion.
pub fn validate(full: &str, cwd: &Path, env: &HashMap<String, String>) -> bool {
    let _ = (full, cwd, env);
    todo!("inline agent")
}
