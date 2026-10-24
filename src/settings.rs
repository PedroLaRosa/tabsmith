//! Paths, the flat-JSON settings/state store, typed getters and the settings catalog (SPEC §3).
pub(crate) mod application;
pub(crate) mod domain;
pub(crate) mod infrastructure;

/// The settings snapshot under its old path until Phase 2 switches the callers.
pub(crate) use crate::settings::domain::value_objects::settings::Settings;

use crate::shared::infrastructure::data_directory_from_env::data_directory_from_env;
use crate::shared::infrastructure::json_document_store::JsonDocumentStore;
use anyhow::Result;
use std::path::PathBuf;

/// `TABSMITH_HOME`, else `$HOME/.tabsmith` (Unix) / `%USERPROFILE%\.tabsmith` (Windows).
/// Delegates to [`data_directory_from_env`] until every caller uses `DataDirectory` (deleted in Phase 11).
pub fn home() -> Result<PathBuf> {
    Ok(data_directory_from_env().root()?)
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
