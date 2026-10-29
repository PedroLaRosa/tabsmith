use crate::shared::domain::value_objects::shell::Shell;
use crate::shared::infrastructure::os::executable_lookup::which;
use crate::shell_integration::application::uninstall_progress::UninstallProgress;
use crate::shell_integration::application::uninstall_tabsmith_use_case::UninstallTabsmithUseCase;
use crate::shell_integration::domain::value_objects::integration_paths::IntegrationPaths;
use crate::shell_integration::domain::value_objects::integration_target::IntegrationTarget;
use crate::shell_integration::infrastructure::cli::integrations_command::integration_failures_error;
use crate::shell_integration::infrastructure::cli::user_confirmation::confirm;
use crate::shell_integration::infrastructure::shell_integration_module::ShellIntegrationModule;
use clap::Args;

#[derive(Args)]
pub(crate) struct UninstallArgs {
    /// Do not prompt for confirmation
    #[arg(long, short = 'y')]
    pub(crate) no_confirm: bool,
    /// Also remove ~/.tabsmith
    #[arg(long)]
    pub(crate) purge: bool,
}

/// `tabsmith uninstall` (SPEC §2.9): asks first unless `--no-confirm`, and locates the paths only once confirmed.
pub(crate) fn run_uninstall_command(args: UninstallArgs, module: &ShellIntegrationModule) -> anyhow::Result<i32> {
    if !args.no_confirm && !confirm("Are you sure want to continue uninstalling tabsmith? [Y/n] ")?
    {
        println!("Cancelled");
        return Ok(1);
    }
    Ok(uninstall_and_report(
        &module.uninstall_tabsmith,
        &module.locate_integration_paths()?,
        &IntegrationTarget::All.shells(|| which("pwsh").is_some()),
        args.purge,
    ))
}

/// Remove every integration of `shells` (and the data root on `purge`), carrying on past errors (SPEC §2.9): 1 if any.
pub(crate) fn uninstall_and_report(
    uninstall_tabsmith: &UninstallTabsmithUseCase,
    paths: &IntegrationPaths,
    shells: &[Shell],
    purge: bool,
) -> i32 {
    let failed = uninstall_tabsmith.execute(paths, shells, purge, |progress| match progress {
        UninstallProgress::IntegrationsFailed(failures) => {
            eprintln!("{:#}", integration_failures_error(failures))
        }
        UninstallProgress::PurgeFailed(reason) => {
            eprintln!("Could not remove {}: {reason}", paths.data_root().display())
        }
    });
    if failed {
        println!("Uninstalled tabsmith with errors (see above).");
        return 1;
    }
    println!("Uninstalled tabsmith.");
    if purge {
        println!("Restart open terminals to stop tabsmith in them.");
    }
    println!(
        "Remove the binary with your package manager, or delete {}.",
        paths.binary()
    );
    0
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use crate::shared::infrastructure::os::process_info::is_root;
    use crate::shared::infrastructure::testing::temp_dir::TempDir;
    use crate::shell_integration::domain::value_objects::dotfile::PRE_MARKER;
    use crate::shell_integration::infrastructure::testing::integration_fixtures::{ctx, install_dotfiles, uninstall_all};

    /// A dotfile uninstall cannot clean (read-only) makes `uninstall` fail with 1, not claim success.
    #[cfg(unix)]
    #[test]
    fn integrations_uninstall_reports_failure() {
        use std::os::unix::fs::PermissionsExt;
        if is_root() {
            return;
        }
        let temp_dir = TempDir::new("uninstfail");
        let dir = temp_dir.path().to_path_buf();
        let c = ctx(&dir);
        install_dotfiles(&c, &[Shell::Zsh]).unwrap();
        let zshrc = c.home().join(".zshrc");
        std::fs::set_permissions(&zshrc, std::fs::Permissions::from_mode(0o444)).unwrap();
        assert_eq!(uninstall_all(&c, &[Shell::Zsh], false), 1);
        assert!(std::fs::read_to_string(&zshrc)
            .unwrap()
            .contains(PRE_MARKER));
        std::fs::set_permissions(&zshrc, std::fs::Permissions::from_mode(0o644)).unwrap();
        assert_eq!(uninstall_all(&c, &[Shell::Zsh], true), 0);
        assert!(!c.data_root().exists());
        let _ = std::fs::remove_dir_all(dir);
    }
}
