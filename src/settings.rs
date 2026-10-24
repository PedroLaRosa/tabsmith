//! Paths, the flat-JSON settings/state store, typed getters and the settings catalog (SPEC §3).
pub(crate) mod application;
pub(crate) mod domain;
pub(crate) mod infrastructure;

/// The settings snapshot under its old path until Phase 2 switches the callers.
pub(crate) use crate::settings::domain::value_objects::settings::Settings;
