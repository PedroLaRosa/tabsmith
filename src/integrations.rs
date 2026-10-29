//! Shell integration: `init` scripts, launch decision, dotfile install/uninstall/status, setup/uninstall flows,
//! and the internal `_ should-launch|get-shell|pre-cmd` commands (SPEC §2.6, §2.7, §2.9, §2.11, §5).
//! OWNER: integrations agent. Shell scripts live in `shell/` and are embedded with `include_str!`.
use crate::shared::domain::value_objects::shell::Shell;
use crate::shared::infrastructure::atomic_file::AtomicFile;
use crate::shared::infrastructure::data_directory_from_env::data_directory_from_env;
use crate::shared::infrastructure::os::executable_lookup::which;
use crate::shared::infrastructure::os::process_info::is_root;
use crate::shared::infrastructure::system_clock::SystemClock;
use crate::shared::infrastructure::terminal_style::{epaint, paint};
use crate::shell_integration::application::integration_installer::IntegrationInstaller;
use crate::shell_integration::domain::repositories::restart_reminder_repository::RestartReminderRepository;
use crate::shell_integration::domain::services::manual_install_instructions;
use crate::shell_integration::domain::value_objects::integration_paths::IntegrationPaths;
use crate::shell_integration::domain::value_objects::integration_target::IntegrationTarget;
use crate::shell_integration::infrastructure::adapters::file_backup_repository::FileBackupRepository;
use crate::shell_integration::infrastructure::adapters::file_dotfile_repository::FileDotfileRepository;
use crate::shell_integration::infrastructure::adapters::os_shell_environment::{
    integration_paths_from, OsShellEnvironment,
};
use crate::shell_integration::infrastructure::adapters::state_restart_reminder_repository::StateRestartReminderRepository;
use crate::shell_integration::infrastructure::cli::integrations_command::integration_failures_error;
use crate::shell_integration::infrastructure::cli::user_confirmation::confirm;
use crate::types::{now_ms, SessionCtx};
use anyhow::{Context, Result};
use clap::Args;
use std::path::Path;
use std::rc::Rc;
use std::sync::Arc;
use std::time::Duration;

#[derive(Args)]
pub struct SetupArgs {
    /// Do not prompt for confirmation
    #[arg(long)]
    pub no_confirm: bool,
    /// Force installation (e.g. as root)
    #[arg(long)]
    pub force: bool,
}

#[derive(Args)]
pub struct UninstallArgs {
    /// Do not prompt for confirmation
    #[arg(long, short = 'y')]
    pub no_confirm: bool,
    /// Also remove ~/.tabsmith
    #[arg(long)]
    pub purge: bool,
}

// ---------------------------------------------------------------------------------------------------------------
// Small shared helpers (also used by doctor)

/// Mark that existing terminals need a restart (state `doctor.prompt-restart-terminal`, printed by doctor).
pub fn mark_restart() {
    StateRestartReminderRepository::new(data_directory_from_env()).save_pending();
}

pub fn pre_cmd(alias: Option<String>) -> anyhow::Result<i32> {
    // The id becomes a file name: accept only what the wrapper generates (hex), never a path.
    let Some(sid) = std::env::var("TABSMITH_SESSION_ID").ok().filter(|s| {
        !s.is_empty()
            && s.bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
    }) else {
        return Ok(1);
    };
    let env = std::env::vars_os()
        .filter_map(|(k, v)| Some((k.into_string().ok()?, v.into_string().ok()?)))
        .collect();
    let ctx = SessionCtx {
        env,
        alias,
        ts: now_ms(),
    };
    let path = SessionCtx::path(&sid)?;
    AtomicFile::new(&path).write_private(serde_json::to_string(&ctx)?.as_bytes())?;
    prune_run_files(path.parent().context("no parent directory")?);
    Ok(0)
}

/// Delete run files untouched for a day. A shell that exits right after a prompt leaves one behind: its background
/// `_ pre-cmd` writes the file again after the wrapper removed it. A live session rewrites its file at every prompt.
fn prune_run_files(dir: &Path) {
    for e in std::fs::read_dir(dir).into_iter().flatten().flatten() {
        let stale = e.metadata().is_ok_and(|m| {
            m.is_file()
                && m.modified()
                    .is_ok_and(|t| t.elapsed().unwrap_or_default() > Duration::from_secs(24 * 3600))
        });
        if stale {
            let _ = std::fs::remove_file(e.path());
        }
    }
}

// ---------------------------------------------------------------------------------------------------------------
// Dotfile integrations (SPEC §5.5)

/// The installer over the files on disk, the system clock and the running system's shell facts.
fn legacy_installer() -> IntegrationInstaller {
    IntegrationInstaller::new(
        Rc::new(FileDotfileRepository),
        Rc::new(FileBackupRepository),
        Arc::new(SystemClock),
        Rc::new(OsShellEnvironment),
    )
}

/// `[bash, zsh, fish]`, plus pwsh when it is on PATH (SPEC §2.6).
fn default_shells() -> Vec<Shell> {
    IntegrationTarget::All.shells(|| which("pwsh").is_some())
}

/// Install every integration of `shells` that is not installed. Ok(true) if something was installed.
pub fn install_dotfiles(ctx: &IntegrationPaths, shells: &[Shell]) -> Result<bool> {
    legacy_installer()
        .install_missing(ctx, shells)
        .map_err(integration_failures_error)
}

// ---------------------------------------------------------------------------------------------------------------
// setup / uninstall (SPEC §2.9)

pub fn setup(args: SetupArgs) -> anyhow::Result<i32> {
    if is_root() && !args.force {
        eprintln!("{}", epaint("1;31", "Installing as root is not supported."));
        eprintln!(
            "{}",
            epaint(
                "31",
                "If you know what you're doing, run the command again with --force."
            )
        );
        return Ok(1);
    }
    let auto = args.no_confirm
        || confirm("Do you want tabsmith to modify your shell config (you will have to manually do this otherwise)? [Y/n] ")?;
    let ctx = integration_paths_from(&data_directory_from_env())?;
    let shells: Vec<Shell> = Shell::ALL
        .into_iter()
        .filter(|s| which(s.name()).is_some())
        .collect();
    let mut manual = !auto;
    if auto {
        match install_dotfiles(&ctx, &shells) {
            Ok(installed) => {
                if installed {
                    mark_restart();
                }
                println!(
                    "{}",
                    if installed {
                        "Installed!"
                    } else {
                        "Already installed"
                    }
                );
            }
            Err(e) => {
                println!("{}", paint("1", "Could not automatically install:"));
                println!("{e:#}");
                manual = true;
            }
        }
    }
    if manual {
        print!("{}", manual_text(&ctx, &shells));
        // Script files only: the user declined (or we failed) to edit the dotfiles themselves.
        let installer = legacy_installer();
        let r = installer
            .apply(
                &ctx,
                &shells,
                |i| i.whole_file_phase().is_none(),
                |i| installer.write_scripts(&ctx, i),
            )
            .map_err(integration_failures_error);
        if let Err(e) = r {
            println!("Could not install required files:");
            println!("{e:#}");
        }
    }
    crate::specs::ensure_extracted().context("could not extract the completion specs")?;
    if !manual {
        println!("Restart your terminal (or open a new tab) to start using tabsmith.");
    }
    Ok(0)
}

fn manual_text(ctx: &IntegrationPaths, shells: &[Shell]) -> String {
    manual_install_instructions::text(ctx, shells)
}

pub fn uninstall(args: UninstallArgs) -> anyhow::Result<i32> {
    if !args.no_confirm && !confirm("Are you sure want to continue uninstalling tabsmith? [Y/n] ")?
    {
        println!("Cancelled");
        return Ok(1);
    }
    Ok(uninstall_all(
        &integration_paths_from(&data_directory_from_env())?,
        &default_shells(),
        args.purge,
    ))
}

/// Remove every integration of `shells` (and the data root on `purge`), carrying on past errors (SPEC §2.9): 1 if any.
fn uninstall_all(ctx: &IntegrationPaths, shells: &[Shell], purge: bool) -> i32 {
    let installer = legacy_installer();
    let mut failed = installer
        .apply(ctx, shells, |_| true, |i| installer.uninstall(ctx, i))
        .map_err(|failures| eprintln!("{:#}", integration_failures_error(failures)))
        .is_err();
    if purge {
        match std::fs::remove_dir_all(ctx.data_root()) {
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => {
                eprintln!("Could not remove {}: {e}", ctx.data_root().display());
                failed = true;
            }
            _ => {}
        }
    }
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
        ctx.binary()
    );
    0
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shared::infrastructure::testing::temp_dir::TempDir;
    #[cfg(unix)]
    use crate::shell_integration::domain::value_objects::dotfile::PRE_MARKER;
    #[cfg(unix)]
    use crate::shell_integration::infrastructure::testing::integration_fixtures::{ctx, install_dotfiles};

    /// The file modes and the purge guard of this scenario are asserted in shared atomic_file.rs
    /// (`integrations_pre_cmd_file`); this half keeps the SessionCtx round trip.
    #[cfg(unix)]
    #[test]
    fn integrations_pre_cmd_file_session_ctx() {
        let temp_dir = TempDir::new("precmd");
        let dir = temp_dir.path().to_path_buf();
        let path = dir.join("run").join("abc123.json");
        let ctx = SessionCtx {
            env: [("A".to_string(), "1".to_string())].into(),
            alias: Some("ll='ls -l'".into()),
            ts: 7,
        };
        AtomicFile::new(&path).write_private(serde_json::to_string(&ctx).unwrap().as_bytes()).unwrap();
        let back = SessionCtx::load(&path).unwrap();
        assert_eq!(
            (back.env["A"].as_str(), back.alias.as_deref(), back.ts),
            ("1", Some("ll='ls -l'"), 7)
        );
        let _ = std::fs::remove_dir_all(dir);
    }

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

    /// Run files untouched for a day go (a quick exit can leave one behind); fresh files and directories stay.
    #[test]
    fn integrations_prune_run_files() {
        let temp_dir = TempDir::new("prune");
        let dir = temp_dir.path().to_path_buf();
        let (old, new, sub) = (
            dir.join("old.json"),
            dir.join("new.json"),
            dir.join("debug-shell-1"),
        );
        std::fs::write(&old, "{}").unwrap();
        std::fs::write(&new, "{}").unwrap();
        std::fs::create_dir(&sub).unwrap();
        let day_ago = std::time::SystemTime::now() - Duration::from_secs(2 * 24 * 3600);
        std::fs::File::options()
            .write(true)
            .open(&old)
            .unwrap()
            .set_modified(day_ago)
            .unwrap();
        prune_run_files(&dir);
        assert!(!old.exists() && new.exists() && sub.exists());
        let _ = std::fs::remove_dir_all(dir);
    }
}
