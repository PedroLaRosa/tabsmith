use crate::shared::domain::value_objects::data_directory::DataDirectory;
use std::ffi::OsString;
use std::path::PathBuf;

/// The data directory this process's environment names (`TABSMITH_HOME`, `TABSMITH_SETTINGS_PATH`, `HOME`,
/// `USERPROFILE`). Never fails: an unknown home surfaces from the paths that need it.
pub(crate) fn data_directory_from_env() -> DataDirectory {
    data_directory_from(|name| std::env::var_os(name))
}

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
    use crate::shared::domain::shared_error::SharedError;

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

    #[test]
    fn should_ignore_an_empty_tabsmith_home() {
        let env = env_with(&[
            ("TABSMITH_HOME", ""),
            ("HOME", "/home/ana"),
            ("USERPROFILE", "/home/ana"),
        ]);

        let data_directory = data_directory_from(env);

        assert_eq!(
            data_directory.root(),
            Ok(PathBuf::from("/home/ana/.tabsmith"))
        );
    }

    #[test]
    fn should_keep_a_relative_tabsmith_home_as_given() {
        let env = env_with(&[("TABSMITH_HOME", "relative/data")]);

        let data_directory = data_directory_from(env);

        assert_eq!(data_directory.root(), Ok(PathBuf::from("relative/data")));
    }

    #[test]
    fn should_put_the_root_under_home_on_unix_and_under_userprofile_on_windows() {
        let env = env_with(&[("HOME", "/home/ana"), ("USERPROFILE", "/users/ana")]);

        let data_directory = data_directory_from(env);

        let expected = if cfg!(windows) {
            "/users/ana/.tabsmith"
        } else {
            "/home/ana/.tabsmith"
        };
        assert_eq!(data_directory.root(), Ok(PathBuf::from(expected)));
    }

    #[test]
    fn should_fall_back_to_home_when_the_platform_variable_is_unset() {
        let env = env_with(&[("HOME", "/home/ana")]);

        let data_directory = data_directory_from(env);

        assert_eq!(
            data_directory.root(),
            Ok(PathBuf::from("/home/ana/.tabsmith"))
        );
    }

    #[test]
    fn should_not_fall_back_to_home_when_the_platform_variable_is_empty() {
        // On Unix the platform variable is HOME itself, so the first (empty) entry is the one read.
        let platform_variable = if cfg!(windows) { "USERPROFILE" } else { "HOME" };
        let env = env_with(&[(platform_variable, ""), ("HOME", "/home/ana")]);

        let data_directory = data_directory_from(env);

        assert_eq!(
            data_directory.root(),
            Err(SharedError::HomeDirectoryNotFound)
        );
    }

    #[test]
    fn should_leave_the_root_unknown_without_a_home_directory() {
        let env = env_with(&[]);

        let data_directory = data_directory_from(env);

        assert_eq!(
            data_directory.root(),
            Err(SharedError::HomeDirectoryNotFound)
        );
    }

    #[test]
    fn should_ignore_an_empty_tabsmith_settings_path() {
        let env = env_with(&[("TABSMITH_SETTINGS_PATH", ""), ("TABSMITH_HOME", "/data")]);

        let data_directory = data_directory_from(env);

        assert_eq!(
            data_directory.settings_file(),
            Ok(PathBuf::from("/data/settings.json"))
        );
    }
}
