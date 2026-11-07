use crate::modules::shared::domain::value_objects::shell::Shell;

/// The integrations a command acts on: the dotfiles of one shell, or of every shell.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum IntegrationTarget {
    Dotfiles(Option<Shell>),
    All,
}

impl IntegrationTarget {
    /// The shell named, else `[bash, zsh, fish]`, plus pwsh when it is on PATH (SPEC §2.6).
    pub(crate) fn shells(&self, pwsh_on_path: impl FnOnce() -> bool) -> Vec<Shell> {
        match self {
            IntegrationTarget::Dotfiles(Some(s)) => vec![*s],
            IntegrationTarget::Dotfiles(None) | IntegrationTarget::All => {
                let mut v = vec![Shell::Bash, Shell::Zsh, Shell::Fish];
                if pwsh_on_path() {
                    v.push(Shell::Pwsh);
                }
                v
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;

    #[test]
    fn should_give_only_the_shell_named_without_looking_for_pwsh() {
        let target = IntegrationTarget::Dotfiles(Some(Shell::Zsh));
        let looked_for_pwsh = Cell::new(false);

        let shells = target.shells(|| {
            looked_for_pwsh.set(true);
            true
        });

        assert_eq!((shells, looked_for_pwsh.get()), (vec![Shell::Zsh], false));
    }

    #[test]
    fn should_give_bash_zsh_and_fish_for_every_dotfile_when_pwsh_is_not_on_path() {
        let target = IntegrationTarget::Dotfiles(None);

        let shells = target.shells(|| false);

        assert_eq!(shells, [Shell::Bash, Shell::Zsh, Shell::Fish]);
    }

    #[test]
    fn should_add_pwsh_for_every_dotfile_when_it_is_on_path() {
        let target = IntegrationTarget::Dotfiles(None);

        let shells = target.shells(|| true);

        assert_eq!(shells, [Shell::Bash, Shell::Zsh, Shell::Fish, Shell::Pwsh]);
    }

    /// `integrations <cmd> all` and `integrations <cmd> dotfiles` without a shell act on the same shells.
    #[test]
    fn should_give_every_shell_for_the_all_target_as_for_every_dotfile() {
        let target = IntegrationTarget::All;

        let shells = [target.shells(|| false), target.shells(|| true)];

        assert_eq!(
            shells,
            [
                vec![Shell::Bash, Shell::Zsh, Shell::Fish],
                vec![Shell::Bash, Shell::Zsh, Shell::Fish, Shell::Pwsh],
            ]
        );
    }
}
