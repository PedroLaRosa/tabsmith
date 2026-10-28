use crate::shared::domain::value_objects::shell::Shell;
use crate::shell_integration::domain::entities::integration::Integration;
use crate::shell_integration::domain::shell_integration_error::ShellIntegrationError;
use crate::shell_integration::domain::value_objects::integration_paths::IntegrationPaths;
use crate::shell_integration::domain::value_objects::integration_phase::IntegrationPhase;
use std::path::{Path, PathBuf};

/// Integrations of `shell` (SPEC §5.5 table).
pub(crate) fn integrations(
    paths: &IntegrationPaths,
    shell: Shell,
    exists: impl Fn(&Path) -> bool,
    pwsh_profile: impl FnOnce() -> Result<PathBuf, ShellIntegrationError>,
) -> Result<Vec<Integration>, ShellIntegrationError> {
    let dot = |path: PathBuf| Integration::dotfile(shell, path);
    Ok(match shell {
        Shell::Bash => {
            let extra: Vec<PathBuf> = [".profile", ".bash_login", ".bash_profile"]
                .iter()
                .map(|f| paths.home().join(f))
                .filter(|p| exists(p))
                .collect();
            let extra = if extra.is_empty() {
                vec![paths.home().join(".profile")]
            } else {
                extra
            };
            std::iter::once(paths.home().join(".bashrc"))
                .chain(extra)
                .map(dot)
                .collect()
        }
        Shell::Zsh => {
            let dir = paths.zdotdir().unwrap_or(paths.home());
            vec![dot(dir.join(".zshrc")), dot(dir.join(".zprofile"))]
        }
        Shell::Fish => [
            (IntegrationPhase::Pre, "00_tabsmith_pre.fish"),
            (IntegrationPhase::Post, "99_tabsmith_post.fish"),
        ]
        .into_iter()
        .map(|(w, f)| Integration::whole_file(shell, paths.fish_config_dir().join("conf.d").join(f), w))
        .collect(),
        Shell::Pwsh => vec![dot(pwsh_profile()?)],
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;
    use std::collections::HashSet;

    fn paths() -> IntegrationPaths {
        IntegrationPaths::new(
            PathBuf::from("/home/u"),
            PathBuf::from("/home/u/.tabsmith"),
            "/usr/local/bin/tabsmith".into(),
            None,
            PathBuf::from("/home/u/.config/fish"),
            None,
        )
    }

    /// Stands for `pwsh` where listing a shell must not ask it for `$PROFILE`.
    fn unasked_pwsh_profile() -> Result<PathBuf, ShellIntegrationError> {
        panic!("pwsh was asked for its $PROFILE")
    }

    fn listed(integrations: &[Integration]) -> Vec<(Shell, PathBuf, Option<IntegrationPhase>)> {
        integrations
            .iter()
            .map(|i| (i.shell(), i.path().to_path_buf(), i.whole_file_phase()))
            .collect()
    }

    #[test]
    fn should_list_the_bashrc_then_each_existing_login_file_from_profile_to_bash_profile() {
        let paths = paths();
        let existing: HashSet<PathBuf> = ["/home/u/.bash_profile", "/home/u/.profile"].map(PathBuf::from).into();

        let bash = integrations(&paths, Shell::Bash, |p| existing.contains(p), unasked_pwsh_profile).unwrap();

        assert_eq!(
            listed(&bash),
            [
                (Shell::Bash, PathBuf::from("/home/u/.bashrc"), None),
                (Shell::Bash, PathBuf::from("/home/u/.profile"), None),
                (Shell::Bash, PathBuf::from("/home/u/.bash_profile"), None),
            ]
        );
    }

    /// A home without any login file still gets one, `.profile`, so login shells source the integration too.
    #[test]
    fn should_list_the_bashrc_and_the_profile_when_no_login_file_exists() {
        let paths = paths();

        let bash = integrations(&paths, Shell::Bash, |_| false, unasked_pwsh_profile).unwrap();

        assert_eq!(
            listed(&bash),
            [
                (Shell::Bash, PathBuf::from("/home/u/.bashrc"), None),
                (Shell::Bash, PathBuf::from("/home/u/.profile"), None),
            ]
        );
    }

    /// zsh reads its dotfiles from `$ZDOTDIR` when it is set.
    #[test]
    fn should_list_the_zshrc_then_the_zprofile_in_zdotdir() {
        let paths = IntegrationPaths::new(
            PathBuf::from("/home/u"),
            PathBuf::from("/home/u/.tabsmith"),
            "/usr/local/bin/tabsmith".into(),
            Some(PathBuf::from("/home/u/.config/zsh")),
            PathBuf::from("/home/u/.config/fish"),
            None,
        );

        let zsh = integrations(&paths, Shell::Zsh, |_| false, unasked_pwsh_profile).unwrap();

        assert_eq!(
            listed(&zsh),
            [
                (Shell::Zsh, PathBuf::from("/home/u/.config/zsh/.zshrc"), None),
                (Shell::Zsh, PathBuf::from("/home/u/.config/zsh/.zprofile"), None),
            ]
        );
    }

    /// fish has no dotfile blocks: tabsmith owns one `conf.d` file per phase, named so fish sources pre first and
    /// post last.
    #[test]
    fn should_list_one_whole_conf_d_file_per_phase_for_fish() {
        let paths = paths();

        let fish = integrations(&paths, Shell::Fish, |_| false, unasked_pwsh_profile).unwrap();

        assert_eq!(
            listed(&fish),
            [
                (
                    Shell::Fish,
                    PathBuf::from("/home/u/.config/fish/conf.d/00_tabsmith_pre.fish"),
                    Some(IntegrationPhase::Pre)
                ),
                (
                    Shell::Fish,
                    PathBuf::from("/home/u/.config/fish/conf.d/99_tabsmith_post.fish"),
                    Some(IntegrationPhase::Post)
                ),
            ]
        );
    }

    /// Asking `pwsh` spawns it (up to 5 s), so it is asked once per listing, and only when the paths do not know
    /// `$PROFILE` yet.
    #[test]
    fn should_ask_pwsh_once_for_the_profile_the_paths_do_not_know() {
        let paths = paths();
        let asked = Cell::new(0);

        let pwsh = integrations(&paths, Shell::Pwsh, |_| false, || {
            asked.set(asked.get() + 1);
            Ok(PathBuf::from("/home/u/.config/powershell/Microsoft.PowerShell_profile.ps1"))
        })
        .unwrap();

        assert_eq!(
            (listed(&pwsh), asked.get()),
            (
                vec![(
                    Shell::Pwsh,
                    PathBuf::from("/home/u/.config/powershell/Microsoft.PowerShell_profile.ps1"),
                    None
                )],
                1
            )
        );
    }
}
