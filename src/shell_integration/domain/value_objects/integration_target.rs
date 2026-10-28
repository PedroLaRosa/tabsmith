use crate::shared::domain::value_objects::shell::Shell;

/// The integrations a command acts on: the dotfiles of one shell, or of every shell.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum IntegrationTarget {
    Dotfiles(Option<Shell>),
    All,
}

impl IntegrationTarget {
    pub(crate) fn shells(&self, _pwsh_on_path: impl FnOnce() -> bool) -> Vec<Shell> {
        match self {
            IntegrationTarget::Dotfiles(Some(s)) => vec![*s],
            IntegrationTarget::Dotfiles(None) | IntegrationTarget::All => vec![Shell::Bash, Shell::Zsh, Shell::Fish],
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
}
