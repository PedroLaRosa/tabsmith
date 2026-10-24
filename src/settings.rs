//! Paths, the flat-JSON settings/state store, typed getters and the settings catalog (SPEC §3).
pub(crate) mod domain;

/// The catalog under its old path until Phase 2 switches the callers.
pub(crate) use crate::settings::domain::services::settings_catalog::catalog;

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
