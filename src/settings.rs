//! Paths, the flat-JSON settings/state store, typed getters and the settings catalog (SPEC §3).
pub(crate) mod domain;
pub(crate) mod infrastructure;

/// The catalog under its old path until Phase 2 switches the callers.
pub(crate) use crate::settings::domain::services::settings_catalog::catalog;
/// The settings snapshot under its old path until Phase 2 switches the callers.
pub(crate) use crate::settings::domain::value_objects::settings::Settings;

use crate::settings::infrastructure::adapters::setting_value_json::settings_from_json;
use crate::shared::infrastructure::data_directory_from_env::data_directory_from_env;
use crate::shared::infrastructure::json_document_store::JsonDocumentStore;
use crate::shared::infrastructure::logger;
use anyhow::{Context, Result};
use std::path::PathBuf;

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

/// Load settings.json; any error → empty (defaults).
pub(crate) fn load_or_defaults() -> Settings {
    match Store::load_settings() {
        Ok(s) => settings_from_json(s.map),
        Err(e) => {
            logger::warn(&format!("settings: {e:#}"));
            Settings::default()
        }
    }
}
