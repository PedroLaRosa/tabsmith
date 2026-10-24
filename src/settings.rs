//! Paths, the flat-JSON settings/state store, typed getters and the settings catalog (SPEC §3).
pub(crate) mod domain;

use crate::shared::infrastructure::data_directory_from_env::data_directory_from_env;
use crate::shared::infrastructure::json_document_store::JsonDocumentStore;
use crate::shared::infrastructure::logger;
use anyhow::{Context, Result};
use serde_json::{Map, Value};
use std::path::PathBuf;
use std::sync::Arc;

/// `TABSMITH_HOME`, else `$HOME/.tabsmith` (Unix) / `%USERPROFILE%\.tabsmith` (Windows).
/// Delegates to [`data_directory_from_env`] until every caller uses `DataDirectory` (deleted in Phase 11).
pub fn home() -> Result<PathBuf> {
    Ok(data_directory_from_env().root()?)
}

/// The user's home directory (not the tabsmith dir).
pub fn user_home() -> Result<PathBuf> {
    std::env::var_os(if cfg!(windows) { "USERPROFILE" } else { "HOME" })
        .or_else(|| std::env::var_os("HOME"))
        .filter(|h| !h.is_empty())
        .map(PathBuf::from)
        .context("could not determine the home directory")
}

/// Delegates to [`data_directory_from_env`] until every caller uses `DataDirectory` (deleted in Phase 11).
pub fn settings_path() -> Result<PathBuf> {
    Ok(data_directory_from_env().settings_file()?)
}

/// Delegates to [`data_directory_from_env`] until every caller uses `DataDirectory` (deleted in Phase 11).
pub fn state_path() -> Result<PathBuf> {
    Ok(data_directory_from_env().state_file()?)
}

/// A flat JSON object file (`settings.json`, `state.json`): [`JsonDocumentStore`] under its old name until Phase 2
/// switches the callers.
pub(crate) type Store = JsonDocumentStore;

impl Store {
    pub fn load_settings() -> Result<Store> {
        Ok(Store::load(&settings_path()?)?)
    }

    pub fn load_state() -> Result<Store> {
        Ok(Store::load(&state_path()?)?)
    }
}

/// Immutable settings snapshot shared across threads. Typed getters fall back to the default on a wrong type.
#[derive(Clone, Default, Debug)]
pub struct Settings(pub Arc<Map<String, Value>>);

impl Settings {
    /// Load settings.json; any error → empty (defaults).
    pub fn load() -> Settings {
        match Store::load_settings() {
            Ok(s) => Settings(Arc::new(s.map)),
            Err(e) => {
                logger::warn(&format!("settings: {e:#}"));
                Settings::default()
            }
        }
    }

    pub fn from_map(map: Map<String, Value>) -> Settings {
        Settings(Arc::new(map))
    }

    pub fn get(&self, key: &str) -> Option<&Value> {
        self.0.get(key)
    }

    pub fn bool(&self, key: &str, default: bool) -> bool {
        self.get(key).and_then(Value::as_bool).unwrap_or(default)
    }

    pub fn string(&self, key: &str) -> Option<&str> {
        self.get(key).and_then(Value::as_str)
    }

    pub fn i64(&self, key: &str, default: i64) -> i64 {
        self.get(key).and_then(Value::as_i64).unwrap_or(default)
    }

    pub fn f64(&self, key: &str, default: f64) -> f64 {
        self.get(key).and_then(Value::as_f64).unwrap_or(default)
    }

    pub fn str_list(&self, key: &str) -> Vec<String> {
        self.get(key)
            .and_then(Value::as_array)
            .map(|a| {
                a.iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Keys with the given prefix, prefix stripped (e.g. `autocomplete.keybindings.`).
    pub fn with_prefix<'a>(
        &'a self,
        prefix: &'a str,
    ) -> impl Iterator<Item = (&'a str, &'a Value)> + 'a {
        self.0
            .iter()
            .filter_map(move |(k, v)| k.strip_prefix(prefix).map(|rest| (rest, v)))
    }
}

/// One row of `settings list --all` (SPEC §3.3).
pub struct CatalogEntry {
    pub key: &'static str,
    pub ty: &'static str,
    pub default: &'static str,
    pub description: &'static str,
}

const fn entry(
    key: &'static str,
    ty: &'static str,
    default: &'static str,
    description: &'static str,
) -> CatalogEntry {
    CatalogEntry {
        key,
        ty,
        default,
        description,
    }
}

/// `default` is a JSON literal, or empty when the key has none (templates like `<name>`).
static CATALOG: &[CatalogEntry] = &[
    entry(
        "autocomplete.disable",
        "bool",
        "false",
        "Never show the dropdown (ghost text unaffected)",
    ),
    entry(
        "autocomplete.insertSpaceAutomatically",
        "bool",
        "true",
        "Append a space after an inserted subcommand/option that takes an argument",
    ),
    entry(
        "autocomplete.immediatelyExecuteAfterSpace",
        "bool",
        "false",
        "With an empty search term, prepend an \"Immediately execute\" row",
    ),
    entry(
        "autocomplete.scrollWrapAround",
        "bool",
        "false",
        "Up/Down wrap around the list",
    ),
    entry(
        "autocomplete.alwaysSuggestCurrentToken",
        "bool",
        "false",
        "Prepend a row that executes the typed token",
    ),
    entry(
        "autocomplete.fuzzySearch",
        "bool",
        "false",
        "Fuzzy instead of prefix filtering",
    ),
    entry(
        "autocomplete.preferVerboseSuggestions",
        "bool",
        "false",
        "Insert the longest alias of an option/subcommand (--message over -m)",
    ),
    entry(
        "autocomplete.onlyShowOnTab",
        "bool",
        "false",
        "Never auto-open the menu; Tab opens it",
    ),
    entry(
        "autocomplete.disableForCommands",
        "string[]",
        "[]",
        "Commands that never get completions",
    ),
    entry(
        "autocomplete.sortMethod",
        "\"most recent\"|\"alphabetical\"",
        "\"most recent\"",
        "Boost recently used suggestions unless alphabetical",
    ),
    entry(
        "autocomplete.scriptTimeout",
        "number (ms)",
        if cfg!(windows) { "20000" } else { "5000" },
        "Generator script timeout floor",
    ),
    entry(
        "autocomplete.immediatelyRunDangerousCommands",
        "bool",
        "false",
        "Allow the execute row for dangerous exact matches",
    ),
    entry(
        "autocomplete.firstTokenCompletion",
        "bool",
        "false",
        "Complete the command name itself",
    ),
    entry(
        "autocomplete.hideAutoExecuteSuggestion",
        "bool",
        "false",
        "Never show execute rows",
    ),
    entry(
        "autocomplete.navigateToHistory",
        "bool",
        "false",
        "Up on the first row toggles history mode instead of hiding",
    ),
    entry(
        "autocomplete.alwaysShowDescription",
        "bool",
        "false",
        "Keep the description pop-out open",
    ),
    entry(
        "autocomplete.theme",
        "string",
        "\"dark\"",
        "dark, light, system, or a file stem in ~/.tabsmith/themes",
    ),
    entry(
        "autocomplete.width",
        "number (columns)",
        "40",
        "Menu width, clamped to the terminal width",
    ),
    entry(
        "autocomplete.height",
        "number (rows)",
        "7",
        "Max visible list rows",
    ),
    entry(
        "autocomplete.developerMode",
        "bool",
        "false",
        "Load specs from devCompletionsFolder first; disable spec caches",
    ),
    entry(
        "autocomplete.devCompletionsFolder",
        "string|null",
        "null",
        "Dev spec folder",
    ),
    entry(
        "autocomplete.keybindings.<keystroke>",
        "string",
        "",
        "Action id or \"ignore\"",
    ),
    entry(
        "beta.history.mode",
        "\"off\"|\"history_only\"|\"show\"",
        "\"off\"",
        "History rows in the dropdown",
    ),
    entry("inline.enabled", "bool", "true", "Ghost text on/off"),
    entry(
        "inline.source",
        "string",
        "\"history+specs\"",
        "history, specs, history+specs or an inline.profiles name",
    ),
    entry(
        "inline.profiles.<name>",
        "object",
        "",
        "LLM profile {provider, model, base_url?, api_key_env?, max_tokens?, effort?}",
    ),
    entry(
        "inline.debounceMs",
        "number",
        "300",
        "Quiet time before an LLM request (ms)",
    ),
    entry(
        "inline.historyCount",
        "number",
        "49",
        "History lines sent to the LLM",
    ),
];

/// Every documented setting (SPEC §3.3).
pub fn catalog() -> &'static [CatalogEntry] {
    CATALOG
}

/// Wildcard matcher for `settings --delete PATTERN`: `*` any run, `?` one char (backtracks to the last `*`).
pub fn wildcard_match(pattern: &str, s: &str) -> bool {
    let (p, s): (Vec<char>, Vec<char>) = (pattern.chars().collect(), s.chars().collect());
    let (mut pi, mut si, mut star, mut mark) = (0, 0, None, 0);
    while si < s.len() {
        if pi < p.len() && (p[pi] == '?' || p[pi] == s[si]) {
            pi += 1;
            si += 1;
        } else if pi < p.len() && p[pi] == '*' {
            star = Some(pi);
            mark = si;
            pi += 1;
        } else if let Some(st) = star {
            pi = st + 1;
            mark += 1;
            si = mark;
        } else {
            return false;
        }
    }
    p[pi..].iter().all(|&c| c == '*')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wildcard() {
        assert!(wildcard_match("autocomplete.theme", "autocomplete.theme"));
        assert!(!wildcard_match("autocomplete.theme", "autocomplete.themes"));
        assert!(wildcard_match(
            "autocomplete.*",
            "autocomplete.keybindings.control+r"
        ));
        assert!(!wildcard_match("autocomplete.*", "inline.enabled"));
        assert!(wildcard_match("*", ""));
        assert!(wildcard_match("inline.?nabled", "inline.enabled"));
        assert!(!wildcard_match("inline.?", "inline."));
        assert!(wildcard_match("*.width", "autocomplete.width"));
        assert!(wildcard_match("a*b*c", "a-x-b-y-b-c"));
        assert!(!wildcard_match("a*b*c", "a-x-b-y-c-d"));
    }

    #[test]
    fn catalog_keys_unique_and_defaults_valid() {
        let mut keys: Vec<_> = catalog().iter().map(|e| e.key).collect();
        let n = keys.len();
        keys.sort();
        keys.dedup();
        assert_eq!(keys.len(), n);
        for e in catalog() {
            assert!(
                e.default.is_empty() || serde_json::from_str::<Value>(e.default).is_ok(),
                "{}",
                e.key
            );
        }
        for k in [
            "autocomplete.theme",
            "autocomplete.height",
            "beta.history.mode",
            "inline.source",
            "inline.historyCount",
        ] {
            assert!(catalog().iter().any(|e| e.key == k), "{k}");
        }
    }
}
