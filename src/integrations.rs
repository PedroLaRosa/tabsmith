//! Shell integration: `init` scripts, launch decision, dotfile install/uninstall/status, setup/uninstall flows,
//! and the internal `_ should-launch|get-shell|pre-cmd` commands (SPEC §2.6, §2.7, §2.9, §2.11, §5).
//! OWNER: integrations agent. Shell scripts live in `shell/` and are embedded with `include_str!`.
use crate::shared::domain::value_objects::shell::Shell;
use crate::shared::infrastructure::atomic_file::AtomicFile;
use crate::shared::infrastructure::data_directory_from_env::data_directory_from_env;
use crate::shared::infrastructure::logger;
use crate::shared::infrastructure::os::executable_lookup::which;
use crate::shared::infrastructure::os::process_info::{is_root, parent_exe};
use crate::shared::infrastructure::output_format::OutputFormat;
use crate::shared::infrastructure::system_clock::SystemClock;
use crate::shared::infrastructure::terminal_style::{epaint, paint};
use crate::shell_integration::application::integration_installer::IntegrationInstaller;
use crate::shell_integration::application::ports::init_script_templates::InitScriptTemplates;
use crate::shell_integration::application::ports::shell_environment::ShellEnvironment;
use crate::shell_integration::domain::repositories::restart_reminder_repository::RestartReminderRepository;
use crate::shell_integration::domain::services::init_script_renderer;
use crate::shell_integration::domain::services::launch_policy;
use crate::shell_integration::domain::services::manual_install_instructions;
use crate::shell_integration::domain::value_objects::integration_paths::IntegrationPaths;
use crate::shell_integration::domain::value_objects::integration_phase::IntegrationPhase;
use crate::shell_integration::domain::value_objects::integration_target::IntegrationTarget;
use crate::shell_integration::infrastructure::adapters::embedded_shell_scripts::EmbeddedShellScripts;
use crate::shell_integration::infrastructure::adapters::file_backup_repository::FileBackupRepository;
use crate::shell_integration::infrastructure::adapters::file_dotfile_repository::FileDotfileRepository;
use crate::shell_integration::infrastructure::adapters::os_shell_environment::{
    integration_paths_from, OsShellEnvironment,
};
use crate::shell_integration::infrastructure::adapters::state_restart_reminder_repository::StateRestartReminderRepository;
use crate::shell_integration::infrastructure::cli::integrations_command::integration_failures_error;
use crate::types::{now_ms, SessionCtx};
use anyhow::{bail, Context, Result};
use clap::{Args, Subcommand, ValueEnum};
use std::io::{IsTerminal, Write};
use std::path::Path;
use std::rc::Rc;
use std::sync::Arc;
use std::time::Duration;

#[derive(Clone, Copy, Debug, PartialEq, Eq, ValueEnum)]
pub enum ShellArg {
    Bash,
    Zsh,
    Fish,
    Pwsh,
}

impl From<ShellArg> for Shell {
    fn from(s: ShellArg) -> Shell {
        match s {
            ShellArg::Bash => Shell::Bash,
            ShellArg::Zsh => Shell::Zsh,
            ShellArg::Fish => Shell::Fish,
            ShellArg::Pwsh => Shell::Pwsh,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, ValueEnum)]
pub enum IntegrationPhaseArg {
    Pre,
    Post,
}

impl From<IntegrationPhaseArg> for IntegrationPhase {
    fn from(w: IntegrationPhaseArg) -> IntegrationPhase {
        match w {
            IntegrationPhaseArg::Pre => IntegrationPhase::Pre,
            IntegrationPhaseArg::Post => IntegrationPhase::Post,
        }
    }
}

#[derive(Args)]
pub struct InitArgs {
    pub shell: ShellArg,
    pub when: IntegrationPhaseArg,
    #[arg(long)]
    pub rcfile: Option<String>,
}

#[derive(Subcommand, Clone, Debug)]
pub enum IntegrationTargetArg {
    /// Shell dotfile integrations
    Dotfiles { shell: Option<ShellArg> },
    /// All integrations
    All,
}

impl From<IntegrationTargetArg> for IntegrationTarget {
    fn from(t: IntegrationTargetArg) -> IntegrationTarget {
        match t {
            IntegrationTargetArg::Dotfiles { shell } => IntegrationTarget::Dotfiles(shell.map(Shell::from)),
            IntegrationTargetArg::All => IntegrationTarget::All,
        }
    }
}

#[derive(Subcommand)]
pub enum IntegrationsCmd {
    /// Install an integration
    Install {
        #[command(subcommand)]
        target: IntegrationTargetArg,
        /// Suppress status messages
        #[arg(long, short)]
        silent: bool,
    },
    /// Uninstall an integration
    Uninstall {
        #[command(subcommand)]
        target: IntegrationTargetArg,
        #[arg(long, short)]
        silent: bool,
    },
    /// Reinstall an integration
    Reinstall {
        #[command(subcommand)]
        target: IntegrationTargetArg,
        #[arg(long, short)]
        silent: bool,
    },
    /// Check the status of an integration
    Status {
        #[command(subcommand)]
        target: IntegrationTargetArg,
        #[arg(long, short, value_enum, default_value_t)]
        format: OutputFormat,
    },
}

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

// ---------------------------------------------------------------------------------------------------------------
// init (SPEC §2.7, §5.3)

pub fn init_cmd(args: InitArgs) -> anyhow::Result<i32> {
    if let Some(rc) = &args.rcfile {
        logger::debug(&format!(
            "init {:?} {} --rcfile {rc}",
            args.shell,
            IntegrationPhase::from(args.when).name()
        ));
    }
    let out = match init_script(args.shell.into(), args.when.into()) {
        Ok(s) => s,
        Err(e) => format!("# Could not load source: {e:#}"),
    };
    let _ = writeln!(std::io::stdout(), "{out}");
    Ok(0)
}

/// The integration code `init <shell> <when>` prints (SPEC §5.3), without the trailing newline.
pub fn init_script(shell: Shell, when: IntegrationPhase) -> anyhow::Result<String> {
    init_output(
        shell,
        when,
        std::env::var_os("TABSMITH_INIT_SNAPSHOT_TEST").is_some(),
    )
}

/// `snapshot` = deterministic output (SPEC §2.7): shell `/bin/zsh`, launch 0, binary `tabsmith`.
pub(crate) fn init_output(shell: Shell, when: IntegrationPhase, snapshot: bool) -> Result<String> {
    let template = EmbeddedShellScripts.template(shell, when);
    let script = if snapshot {
        init_script_renderer::render_snapshot(shell, when, template)
    } else {
        init_script_renderer::render(
            shell,
            when,
            template,
            &OsShellEnvironment.current_executable()?,
            || OsShellEnvironment.parent_shell(),
            || should_launch().0,
        )
    };
    Ok(script.into_string())
}

// ---------------------------------------------------------------------------------------------------------------
// Launch decision (SPEC §5.2)

/// Launch decision (SPEC §5.2): (launch?, reason line).
pub fn should_launch() -> (bool, String) {
    let decision = launch_policy::decide(&OsShellEnvironment.launch_facts());
    (decision.launches(), decision.reason().to_string())
}

pub fn should_launch_cmd() -> anyhow::Result<i32> {
    let (launch, why) = should_launch();
    println!("{why}");
    Ok(if launch { 0 } else { 1 })
}

pub fn get_shell_cmd() -> anyhow::Result<i32> {
    match parent_exe() {
        Some(p) => {
            print!("{p}");
            let _ = std::io::stdout().flush();
            Ok(0)
        }
        None => Ok(1),
    }
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

/// Uninstall every integration of `shells` with something on disk. Not only the installed ones: `is_installed`
/// fails exactly when uninstall is needed (a moved binary, lines appended after the post block). Ok(true) if any.
fn uninstall_dotfiles(ctx: &IntegrationPaths, shells: &[Shell]) -> Result<bool> {
    let installer = legacy_installer();
    installer
        .apply(ctx, shells, |i| installer.has_traces(ctx, i), |i| installer.uninstall(ctx, i))
        .map_err(integration_failures_error)
}

fn target_shells(t: &IntegrationTargetArg) -> Vec<Shell> {
    IntegrationTarget::from(t.clone()).shells(|| which("pwsh").is_some())
}

pub fn integrations_cmd(cmd: IntegrationsCmd) -> anyhow::Result<i32> {
    let ctx = integration_paths_from(&data_directory_from_env())?;
    let install = |target: &IntegrationTargetArg, silent: bool| -> Result<i32> {
        let installed = install_dotfiles(&ctx, &target_shells(target))?;
        if installed {
            mark_restart();
        }
        if !silent {
            println!(
                "{}",
                if installed {
                    "Installed!"
                } else {
                    "Already installed"
                }
            );
        }
        Ok(0)
    };
    let uninstall = |target: &IntegrationTargetArg, silent: bool| -> Result<i32> {
        let done = uninstall_dotfiles(&ctx, &target_shells(target))?;
        if !silent {
            println!(
                "{}",
                if done {
                    "Uninstalled!"
                } else {
                    "Not installed"
                }
            );
        }
        Ok(0)
    };
    match cmd {
        IntegrationsCmd::Install { target, silent } => install(&target, silent),
        IntegrationsCmd::Uninstall { target, silent } => uninstall(&target, silent),
        IntegrationsCmd::Reinstall { target, silent } => {
            uninstall(&target, true)?;
            install(&target, silent)
        }
        IntegrationsCmd::Status { target, format } => status(&ctx, &target_shells(&target), format),
    }
}

/// `integrations status` (SPEC §2.6); honours the SHELL argument.
fn status(ctx: &IntegrationPaths, shells: &[Shell], format: OutputFormat) -> Result<i32> {
    let installer = legacy_installer();
    let (mut rows, mut errs) = (Vec::new(), Vec::new());
    for &shell in shells {
        match installer.integrations(ctx, shell) {
            Ok(list) => rows.extend(list.into_iter().map(|i| (installer.check(ctx, &i).is_ok(), i))),
            Err(e) => errs.push((shell, format!("{e:#}"))),
        }
    }
    if format == OutputFormat::Plain {
        for (ok, i) in &rows {
            println!(
                "{} {}",
                if *ok {
                    paint("32", "✔")
                } else {
                    paint("31", "✘")
                },
                i.describe()
            );
        }
        for (shell, e) in &errs {
            println!("{}: {e}", shell.name());
        }
        return Ok(0);
    }
    let v = serde_json::json!({
        "integrations": rows.iter().map(|(ok, i)| serde_json::json!({
            "installed": ok, "description": i.describe(), "shell": i.shell().name(), "file_name": i.file_name(),
        })).collect::<Vec<_>>(),
        "errors": errs.iter().map(|(s, e)| serde_json::json!({
            "shell": s.name(), "error": {"title": e, "message": null},
        })).collect::<Vec<_>>(),
    });
    let out = if format == OutputFormat::Json {
        v.to_string()
    } else {
        serde_json::to_string_pretty(&v)?
    };
    println!("{out}");
    Ok(0)
}

// ---------------------------------------------------------------------------------------------------------------
// setup / uninstall (SPEC §2.9)

/// `[Y/n]` prompt on a tty.
fn confirm(prompt: &str) -> Result<bool> {
    if !std::io::stdin().is_terminal() {
        bail!("You must run with --no-confirm if unattended");
    }
    print!("{prompt}");
    std::io::stdout().flush()?;
    answer(std::io::stdin().lock())
}

/// The `[Y/n]` answer on `input`: empty, `y` or `yes` = yes; end of input (Ctrl-D) = no.
fn answer(mut input: impl std::io::BufRead) -> Result<bool> {
    let mut line = String::new();
    if input.read_line(&mut line)? == 0 {
        println!();
        return Ok(false);
    }
    Ok(matches!(
        line.trim().to_ascii_lowercase().as_str(),
        "" | "y" | "yes"
    ))
}

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
    use crate::shell_integration::infrastructure::testing::integration_fixtures::{ctx, install_dotfiles};

    #[test]
    fn integrations_install_dotfiles_reports() {
        let temp_dir = TempDir::new("apply");
        let dir = temp_dir.path().to_path_buf();
        let c = ctx(&dir);
        assert!(install_dotfiles(&c, &[Shell::Zsh, Shell::Fish]).unwrap());
        assert!(!install_dotfiles(&c, &[Shell::Zsh, Shell::Fish]).unwrap());
        // A dotfile that cannot be written is reported with its description.
        std::fs::create_dir_all(c.home().join(".bashrc")).unwrap(); // a directory where the file should be
        let e = format!("{:#}", install_dotfiles(&c, &[Shell::Bash]).unwrap_err());
        assert!(
            e.starts_with("\n\n")
                && e.contains("bash pre post into .bashrc")
                && !e.contains(".profile"),
            "{e}"
        );
        let _ = std::fs::remove_dir_all(dir);
    }

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

    /// Ctrl-D (end of input) at a [Y/n] prompt is no, never yes.
    #[test]
    fn integrations_confirm_eof_is_no() {
        assert!(!answer(&b""[..]).unwrap());
        assert!(answer(&b"\n"[..]).unwrap() && answer(&b"Yes\n"[..]).unwrap());
        assert!(!answer(&b"n\n"[..]).unwrap());
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
