use crate::shared::domain::value_objects::data_directory::DataDirectory;
use std::ffi::OsString;
use std::path::PathBuf;

fn data_directory_from(env: impl Fn(&str) -> Option<OsString>) -> DataDirectory {
    DataDirectory::new(root_from(&env), settings_file_from(&env))
}

/// `TABSMITH_HOME`, else `$HOME/.tabsmith` (Unix) / `%USERPROFILE%\.tabsmith` (Windows).
fn root_from(env: &impl Fn(&str) -> Option<OsString>) -> Option<PathBuf> {
    if let Some(h) = env("TABSMITH_HOME").filter(|h| !h.is_empty()) {
        return Some(PathBuf::from(h));
    }
    let base = env(if cfg!(windows) { "USERPROFILE" } else { "HOME" })
        .or_else(|| env("HOME"))
        .filter(|h| !h.is_empty())?;
    Some(PathBuf::from(base).join(".tabsmith"))
}

fn settings_file_from(env: &impl Fn(&str) -> Option<OsString>) -> Option<PathBuf> {
    env("TABSMITH_SETTINGS_PATH")
        .filter(|p| !p.is_empty())
        .map(PathBuf::from)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn env_with(vars: &[(&str, &str)]) -> impl Fn(&str) -> Option<OsString> {
        let vars: Vec<(String, OsString)> = vars
            .iter()
            .map(|(name, value)| (name.to_string(), OsString::from(value)))
            .collect();
        move |name| {
            vars.iter()
                .find(|(key, _)| key == name)
                .map(|(_, value)| value.clone())
        }
    }

    #[test]
    fn should_use_tabsmith_home_as_the_root() {
        let env = env_with(&[("TABSMITH_HOME", "/data"), ("HOME", "/home/ana")]);

        let data_directory = data_directory_from(env);

        assert_eq!(data_directory.root(), Ok(PathBuf::from("/data")));
    }

    #[test]
    fn should_move_the_settings_file_to_tabsmith_settings_path_even_without_a_home_directory() {
        let env = env_with(&[("TABSMITH_SETTINGS_PATH", "/elsewhere/s.json")]);

        let data_directory = data_directory_from(env);

        assert_eq!(
            data_directory.settings_file(),
            Ok(PathBuf::from("/elsewhere/s.json"))
        );
    }
}
