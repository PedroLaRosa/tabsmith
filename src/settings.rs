//! Paths, the flat-JSON settings/state store, typed getters and the settings catalog (SPEC §3).
use anyhow::{Context, Result};
use serde_json::{Map, Value};
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// `TABSMITH_HOME`, else `$HOME/.tabsmith` (Unix) / `%USERPROFILE%\.tabsmith` (Windows).
pub fn home() -> Result<PathBuf> {
    if let Some(h) = std::env::var_os("TABSMITH_HOME").filter(|h| !h.is_empty()) {
        return Ok(PathBuf::from(h));
    }
    let base = std::env::var_os(if cfg!(windows) { "USERPROFILE" } else { "HOME" })
        .or_else(|| std::env::var_os("HOME"))
        .filter(|h| !h.is_empty())
        .context("could not determine the home directory (set HOME or TABSMITH_HOME)")?;
    Ok(PathBuf::from(base).join(".tabsmith"))
}

/// The user's home directory (not the tabsmith dir).
pub fn user_home() -> Result<PathBuf> {
    std::env::var_os(if cfg!(windows) { "USERPROFILE" } else { "HOME" })
        .or_else(|| std::env::var_os("HOME"))
        .filter(|h| !h.is_empty())
        .map(PathBuf::from)
        .context("could not determine the home directory")
}

pub fn settings_path() -> Result<PathBuf> {
    if let Some(p) = std::env::var_os("TABSMITH_SETTINGS_PATH").filter(|p| !p.is_empty()) {
        return Ok(PathBuf::from(p));
    }
    Ok(home()?.join("settings.json"))
}

pub fn state_path() -> Result<PathBuf> {
    Ok(home()?.join("state.json"))
}

/// A flat JSON object file (`settings.json`, `state.json`): literal dotted keys → arbitrary JSON.
pub struct Store {
    pub path: PathBuf,
    pub map: Map<String, Value>,
}

impl Store {
    /// Missing file → empty map. Parse error → Err.
    pub fn load(path: &Path) -> Result<Store> {
        let map = match std::fs::read_to_string(path) {
            Ok(s) if s.trim().is_empty() => Map::new(),
            Ok(s) => match serde_json::from_str::<Value>(&s)
                .with_context(|| format!("{} is corrupted", path.display()))?
            {
                Value::Object(m) => m,
                _ => anyhow::bail!("{} is corrupted (not a JSON object)", path.display()),
            },
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Map::new(),
            Err(e) => return Err(e).with_context(|| format!("reading {}", path.display())),
        };
        Ok(Store { path: path.to_path_buf(), map })
    }

    pub fn load_settings() -> Result<Store> {
        Store::load(&settings_path()?)
    }

    pub fn load_state() -> Result<Store> {
        Store::load(&state_path()?)
    }

    /// Pretty JSON (keys sorted), written to a temp file then renamed; Unix mode 0600.
    pub fn save(&self) -> Result<()> {
        if let Some(dir) = self.path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let tmp = self.path.with_extension(format!("tmp-{}", std::process::id()));
        let mut body = serde_json::to_string_pretty(&Value::Object(self.map.clone()))?;
        body.push('\n');
        std::fs::write(&tmp, body)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = std::fs::set_permissions(&tmp, std::fs::Permissions::from_mode(0o600));
        }
        std::fs::rename(&tmp, &self.path).with_context(|| format!("writing {}", self.path.display()))?;
        Ok(())
    }
}

/// Immutable settings snapshot shared across threads. Typed getters follow Kiro (wrong type → default).
#[derive(Clone, Default, Debug)]
pub struct Settings(pub Arc<Map<String, Value>>);

impl Settings {
    /// Load settings.json; any error → empty (defaults).
    pub fn load() -> Settings {
        match Store::load_settings() {
            Ok(s) => Settings(Arc::new(s.map)),
            Err(e) => {
                crate::logger::warn(&format!("settings: {e:#}"));
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
            .map(|a| a.iter().filter_map(|v| v.as_str().map(String::from)).collect())
            .unwrap_or_default()
    }

    /// Keys with the given prefix, prefix stripped (e.g. `autocomplete.keybindings.`).
    pub fn with_prefix<'a>(&'a self, prefix: &'a str) -> impl Iterator<Item = (&'a str, &'a Value)> + 'a {
        self.0.iter().filter_map(move |(k, v)| k.strip_prefix(prefix).map(|rest| (rest, v)))
    }
}

/// One row of `settings list --all` (SPEC §3.3).
pub struct CatalogEntry {
    pub key: &'static str,
    pub ty: &'static str,
    pub default: &'static str,
    pub description: &'static str,
}

/// Every documented setting (SPEC §3.3).
pub fn catalog() -> &'static [CatalogEntry] {
    todo!("cli agent: SPEC §3.3 catalog")
}

/// Wildcard matcher for `settings --delete PATTERN`: `*` any run, `?` one char.
pub fn wildcard_match(pattern: &str, s: &str) -> bool {
    let _ = (pattern, s);
    todo!("cli agent: SPEC §2.3")
}
