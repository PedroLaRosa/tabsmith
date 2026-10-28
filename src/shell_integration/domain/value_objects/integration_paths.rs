use std::path::{Path, PathBuf};

/// Everything an integration operation reads, explicit so tests use a temp HOME.
#[derive(Clone, Debug)]
pub(crate) struct IntegrationPaths {
    /// The user's home directory.
    home: PathBuf,
    /// `~/.tabsmith`.
    data_root: PathBuf,
    /// Absolute path of the tabsmith binary written into the script files.
    binary: String,
    /// `$ZDOTDIR`, if set.
    zdotdir: Option<PathBuf>,
    /// fish config dir: `$XDG_CONFIG_HOME/fish` or `~/.config/fish`.
    fish_config_dir: PathBuf,
    /// pwsh `$PROFILE`; `None` = ask `pwsh`.
    pwsh_profile: Option<PathBuf>,
}

impl IntegrationPaths {
    pub(crate) fn new(
        home: PathBuf,
        data_root: PathBuf,
        binary: String,
        zdotdir: Option<PathBuf>,
        fish_config_dir: PathBuf,
        pwsh_profile: Option<PathBuf>,
    ) -> Self {
        IntegrationPaths {
            home,
            data_root,
            binary,
            zdotdir,
            fish_config_dir,
            pwsh_profile,
        }
    }

    /// The same paths with pwsh's `$PROFILE` known, so listing the pwsh integration does not ask `pwsh`.
    pub(crate) fn with_pwsh_profile(self, pwsh_profile: PathBuf) -> Self {
        IntegrationPaths {
            pwsh_profile: Some(pwsh_profile),
            ..self
        }
    }

    pub(crate) fn home(&self) -> &Path {
        &self.home
    }

    pub(crate) fn data_root(&self) -> &Path {
        &self.data_root
    }

    pub(crate) fn binary(&self) -> &str {
        &self.binary
    }

    pub(crate) fn zdotdir(&self) -> Option<&Path> {
        self.zdotdir.as_deref()
    }

    pub(crate) fn fish_config_dir(&self) -> &Path {
        &self.fish_config_dir
    }

    pub(crate) fn pwsh_profile(&self) -> Option<&Path> {
        self.pwsh_profile.as_deref()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn paths() -> IntegrationPaths {
        IntegrationPaths::new(
            PathBuf::from("/home/u"),
            PathBuf::from("/home/u/.tabsmith"),
            "/usr/local/bin/tabsmith".to_string(),
            Some(PathBuf::from("/home/u/.zsh")),
            PathBuf::from("/home/u/.config/fish"),
            None,
        )
    }

    /// Doctor passes the `$PROFILE` it already knows, even an empty one, so listing pwsh never asks `pwsh` again.
    #[test]
    fn should_know_the_pwsh_profile_it_is_given_and_keep_every_other_path() {
        let unknown_profile = paths();

        let known = unknown_profile.with_pwsh_profile(PathBuf::new());

        assert_eq!(
            (
                known.home(),
                known.data_root(),
                known.binary(),
                known.zdotdir(),
                known.fish_config_dir(),
                known.pwsh_profile(),
            ),
            (
                Path::new("/home/u"),
                Path::new("/home/u/.tabsmith"),
                "/usr/local/bin/tabsmith",
                Some(Path::new("/home/u/.zsh")),
                Path::new("/home/u/.config/fish"),
                Some(Path::new("")),
            )
        );
    }
}
