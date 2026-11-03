use crate::shared::domain::value_objects::shell::Shell;
use crate::shell_integration::domain::entities::integration::Integration;
use crate::shell_integration::domain::value_objects::integration_paths::IntegrationPaths;
use crate::shell_integration::domain::value_objects::integration_phase::IntegrationPhase;

/// Manual install instructions: the exact lines install writes, so doctor and
/// uninstall recognise them, and their `[ -f … ]` guard keeps a later uninstall from breaking shell startup.
pub(crate) fn text(paths: &IntegrationPaths, shells: &[Shell]) -> String {
    let rc = |shell, file: &str| Integration::dotfile(shell, paths.home().join(file));
    let mut rcs = vec![
        ("bash:   ", rc(Shell::Bash, ".bashrc")),
        ("zsh:    ", rc(Shell::Zsh, ".zshrc")),
    ];
    if shells.contains(&Shell::Pwsh) {
        rcs.push(("pwsh ($PROFILE):", rc(Shell::Pwsh, "profile")));
    }
    let mut s = String::from("\nTo install the integrations manually, you will have to add the following to your rc files\n");
    s += "This step is required for the application to function properly\n";
    for (phase, at) in [(IntegrationPhase::Pre, "top"), (IntegrationPhase::Post, "bottom")] {
        s += &format!("\nAt the {at} of your .bashrc or .zshrc file:\n");
        for (label, i) in &rcs {
            s += &format!("{label} {}\n", i.source_line(paths, phase));
        }
    }
    s + "\n"
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shared::infrastructure::testing::temp_dir::TempDir;
    use crate::shell_integration::infrastructure::testing::integration_fixtures::ctx;

    /// What setup prints when it does not edit the dotfiles: bash and zsh lines (never fish), then a blank line.
    #[test]
    fn should_print_the_manual_install_text_for_bash_and_zsh() {
        let temp_dir = TempDir::new("manual");
        let c = ctx(temp_dir.path());

        let text = text(&c, &[Shell::Bash, Shell::Zsh]);

        assert_eq!(
            text,
            "\nTo install the integrations manually, you will have to add the following to your rc files\nThis step is required for the application to function properly\n\nAt the top of your .bashrc or .zshrc file:\nbash:    [ -f \"${HOME}/.tabsmith/shell/bashrc.pre.bash\" ] && . \"${HOME}/.tabsmith/shell/bashrc.pre.bash\"\nzsh:     [ -f \"${HOME}/.tabsmith/shell/zshrc.pre.zsh\" ] && . \"${HOME}/.tabsmith/shell/zshrc.pre.zsh\"\n\nAt the bottom of your .bashrc or .zshrc file:\nbash:    [ -f \"${HOME}/.tabsmith/shell/bashrc.post.bash\" ] && . \"${HOME}/.tabsmith/shell/bashrc.post.bash\"\nzsh:     [ -f \"${HOME}/.tabsmith/shell/zshrc.post.zsh\" ] && . \"${HOME}/.tabsmith/shell/zshrc.post.zsh\"\n\n"
        );
    }

    /// With pwsh on PATH the pwsh lines follow the zsh ones, for the pwsh profile.
    #[test]
    fn should_print_the_manual_install_text_with_pwsh() {
        let temp_dir = TempDir::new("manualpwsh");
        let c = ctx(temp_dir.path());

        let text = text(&c, &[Shell::Bash, Shell::Zsh, Shell::Pwsh]);

        assert_eq!(
            text,
            "\nTo install the integrations manually, you will have to add the following to your rc files\nThis step is required for the application to function properly\n\nAt the top of your .bashrc or .zshrc file:\nbash:    [ -f \"${HOME}/.tabsmith/shell/bashrc.pre.bash\" ] && . \"${HOME}/.tabsmith/shell/bashrc.pre.bash\"\nzsh:     [ -f \"${HOME}/.tabsmith/shell/zshrc.pre.zsh\" ] && . \"${HOME}/.tabsmith/shell/zshrc.pre.zsh\"\npwsh ($PROFILE): if (Test-Path \"${HOME}/.tabsmith/shell/profile.pre.ps1\") { . \"${HOME}/.tabsmith/shell/profile.pre.ps1\" }\n\nAt the bottom of your .bashrc or .zshrc file:\nbash:    [ -f \"${HOME}/.tabsmith/shell/bashrc.post.bash\" ] && . \"${HOME}/.tabsmith/shell/bashrc.post.bash\"\nzsh:     [ -f \"${HOME}/.tabsmith/shell/zshrc.post.zsh\" ] && . \"${HOME}/.tabsmith/shell/zshrc.post.zsh\"\npwsh ($PROFILE): if (Test-Path \"${HOME}/.tabsmith/shell/profile.post.ps1\") { . \"${HOME}/.tabsmith/shell/profile.post.ps1\" }\n\n"
        );
    }
}
