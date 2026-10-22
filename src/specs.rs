//! The embedded Fig spec bundle: extraction, index and name → file resolution (SPEC §9.1, §6.3).
//! OWNER: host agent.
use crate::settings::Settings;
use std::collections::HashSet;
use std::path::PathBuf;

/// Version of the bundled `@withfig/autocomplete` package.
pub fn version() -> &'static str {
    include_str!("../assets/SPECS_VERSION").trim()
}

/// `~/.tabsmith/specs/<ver>/`, extracting the embedded archive first if `index.json` is missing (atomic rename;
/// removes other versions afterwards).
pub fn ensure_extracted() -> anyhow::Result<PathBuf> {
    todo!("host agent")
}

/// `~/.tabsmith/specs/user/` (user overrides, same layout as the npm build/ dir).
pub fn user_dir() -> anyhow::Result<PathBuf> {
    Ok(crate::settings::home()?.join("specs").join("user"))
}

/// `index.json` of the bundled set ∪ `*.js` stems in the user dir.
#[derive(Clone, Debug, Default)]
pub struct SpecIndex {
    /// Root spec names (e.g. `git`, `aws`, `@withfig/autocomplete-tools`).
    pub completions: Vec<String>,
    /// Diff-versioned spec names (`az`, `heroku`, …) → load `<name>/index.js`.
    pub diff_versioned: HashSet<String>,
}

impl SpecIndex {
    pub fn load() -> anyhow::Result<SpecIndex> {
        todo!("host agent")
    }
}

/// Resolve a global spec name to a file: dev folder (when `autocomplete.developerMode`), then the user dir, then
/// the extracted bundle; `<name>.js`, else `<name>/index.js`; diff-versioned names go to `<name>/index.js`.
pub fn resolve(name: &str, settings: &Settings, index: &SpecIndex) -> Option<PathBuf> {
    let _ = (name, settings, index);
    todo!("host agent")
}
