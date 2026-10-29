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
use crate::shell_integration::domain::entities::integration::Integration;
use crate::shell_integration::domain::repositories::restart_reminder_repository::RestartReminderRepository;
use crate::shell_integration::domain::services::init_script_renderer;
use crate::shell_integration::domain::services::launch_policy;
use crate::shell_integration::domain::services::manual_install_instructions;
use crate::shell_integration::domain::shell_integration_error::ShellIntegrationError;
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

/// Integrations of `shell` (SPEC §5.5 table), asking `pwsh` for `$PROFILE` when the paths do not know it.
pub fn integrations(ctx: &IntegrationPaths, shell: Shell) -> Result<Vec<Integration>, ShellIntegrationError> {
    legacy_installer().integrations(ctx, shell)
}

/// The installer over the files on disk, the system clock and the running system's shell facts.
fn legacy_installer() -> IntegrationInstaller {
    IntegrationInstaller::new(
        Rc::new(FileDotfileRepository),
        Rc::new(FileBackupRepository),
        Arc::new(SystemClock),
        Rc::new(OsShellEnvironment),
    )
}

impl Integration {
    /// Write both `~/.tabsmith/shell/` script files of a dotfile integration.
    pub(crate) fn write_scripts(&self, ctx: &IntegrationPaths) -> Result<(), ShellIntegrationError> {
        legacy_installer().write_scripts(ctx, self)
    }

    /// SPEC §5.5 `is_installed()`: the first failing reason.
    pub(crate) fn is_installed(&self, ctx: &IntegrationPaths) -> Result<(), ShellIntegrationError> {
        legacy_installer().check(ctx, self)
    }

    /// SPEC §5.5 install (idempotent); an existing dotfile is first copied to `<data>/backups/<unix-secs>/`.
    pub(crate) fn install(&self, ctx: &IntegrationPaths) -> Result<(), ShellIntegrationError> {
        legacy_installer().install(ctx, self)
    }

    /// Anything uninstall would remove is on disk: also true where `is_installed` fails (a moved binary, an edited
    /// script, lines appended after the post block).
    pub(crate) fn has_traces(&self, ctx: &IntegrationPaths) -> bool {
        legacy_installer().has_traces(ctx, self)
    }

    /// SPEC §5.5 uninstall: drop our exact lines, then delete the script files (missing is fine).
    pub(crate) fn uninstall(&self, ctx: &IntegrationPaths) -> Result<(), ShellIntegrationError> {
        legacy_installer().uninstall(ctx, self)
    }

    /// The exact bytes of the `~/.tabsmith/shell` script file of `phase`.
    pub(crate) fn script_contents(&self, paths: &IntegrationPaths, phase: IntegrationPhase) -> String {
        self.script(paths, phase).contents().to_string()
    }
}

/// `[bash, zsh, fish]`, plus pwsh when it is on PATH (SPEC §2.6).
fn default_shells() -> Vec<Shell> {
    IntegrationTarget::All.shells(|| which("pwsh").is_some())
}

/// Run `op` on every integration of `shells` that `pick` selects. Ok(true) if `op` ran at least once; errors
/// are collected as `"<describe bold>: <message>"` and returned as one (SPEC §2.6).
fn apply(
    ctx: &IntegrationPaths,
    shells: &[Shell],
    pick: impl Fn(&Integration) -> bool,
    op: impl Fn(&Integration, &IntegrationPaths) -> Result<(), ShellIntegrationError>,
) -> Result<bool> {
    legacy_installer()
        .apply(ctx, shells, pick, |i| op(i, ctx))
        .map_err(integration_failures_error)
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
    apply(ctx, shells, |i| i.has_traces(ctx), Integration::uninstall)
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
    let (mut rows, mut errs) = (Vec::new(), Vec::new());
    for &shell in shells {
        match integrations(ctx, shell) {
            Ok(list) => rows.extend(list.into_iter().map(|i| (i.is_installed(ctx).is_ok(), i))),
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
        let r = apply(
            &ctx,
            &shells,
            |i| i.whole_file_phase().is_none(),
            Integration::write_scripts,
        );
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
    let mut failed = apply(ctx, shells, |_| true, Integration::uninstall)
        .map_err(|e| eprintln!("{e:#}"))
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
    use crate::shell_integration::domain::value_objects::dotfile::{POST_MARKER, PRE_MARKER};
    use crate::shell_integration::infrastructure::testing::integration_fixtures::{ctx, syntax_ok};

    #[test]
    fn integrations_dotfile_install_uninstall() {
        let temp_dir = TempDir::new("dotfile");
        let dir = temp_dir.path().to_path_buf();
        let c = ctx(&dir);
        std::fs::create_dir_all(c.home()).unwrap();
        let zshrc = c.home().join(".zshrc");
        let original = "#!/bin/zsh\nexport A=1\nalias ll='ls -l'\n";
        std::fs::write(&zshrc, original).unwrap();

        let rc = Integration::dotfile(Shell::Zsh, zshrc.clone());
        assert!(matches!(
            rc.is_installed(&c),
            Err(ShellIntegrationError::NotInstalled(_))
        ));
        rc.install(&c).unwrap();
        rc.is_installed(&c).unwrap();
        let once = std::fs::read_to_string(&zshrc).unwrap();
        let lines: Vec<&str> = once.lines().collect();
        assert_eq!(lines[0], "#!/bin/zsh");
        assert_eq!(lines[1], PRE_MARKER);
        assert_eq!(
            lines[2],
            r#"[ -f "${HOME}/.tabsmith/shell/zshrc.pre.zsh" ] && . "${HOME}/.tabsmith/shell/zshrc.pre.zsh""#
        );
        assert_eq!(lines[lines.len() - 2], POST_MARKER);
        assert!(lines[lines.len() - 1].ends_with(r#". "${HOME}/.tabsmith/shell/zshrc.post.zsh""#));
        assert_eq!(
            std::fs::read_to_string(c.data_root().join("shell/zshrc.pre.zsh")).unwrap(),
            "[ -x '/opt/tab smith/it'\\''s/tabsmith' ] && eval \"$('/opt/tab smith/it'\\''s/tabsmith' init zsh pre --rcfile zshrc)\"\n"
        );

        // Idempotent: a second install changes nothing.
        rc.install(&c).unwrap();
        assert_eq!(std::fs::read_to_string(&zshrc).unwrap(), once);

        // Backup of the original.
        let backups: Vec<_> = std::fs::read_dir(c.data_root().join("backups"))
            .unwrap()
            .flatten()
            .collect();
        assert_eq!(backups.len(), 1);
        assert_eq!(
            std::fs::read_to_string(backups[0].path().join(".zshrc")).unwrap(),
            original
        );

        // The installed dotfile and script files still parse.
        syntax_ok(Shell::Zsh, &zshrc);

        // Uninstall restores the original and deletes the script files.
        rc.uninstall(&c).unwrap();
        assert_eq!(std::fs::read_to_string(&zshrc).unwrap(), original);
        assert!(!c.data_root().join("shell/zshrc.pre.zsh").exists());
        assert!(!c.data_root().join("shell/zshrc.post.zsh").exists());
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn integrations_dotfile_detection() {
        let temp_dir = TempDir::new("detect");
        let dir = temp_dir.path().to_path_buf();
        let c = ctx(&dir);
        let bashrc = c.home().join(".bashrc");
        let rc = Integration::dotfile(Shell::Bash, bashrc.clone());
        assert!(matches!(
            rc.is_installed(&c),
            Err(ShellIntegrationError::FileDoesNotExist(_))
        ));

        rc.install(&c).unwrap(); // creates the file
        let installed = std::fs::read_to_string(&bashrc).unwrap();
        syntax_ok(Shell::Bash, &bashrc);

        std::fs::write(&bashrc, format!("echo first\n{installed}")).unwrap();
        let e = rc.is_installed(&c).unwrap_err().to_string();
        assert!(e.ends_with("does not source pre integration first"), "{e}");

        std::fs::write(
            &bashrc,
            format!("{installed}echo last\n# trailing comment\n\n"),
        )
        .unwrap();
        let e = rc.is_installed(&c).unwrap_err().to_string();
        assert!(
            e.starts_with("Improper integration installation: ")
                && e.ends_with("post integration last"),
            "{e}"
        );

        // A moved binary: script bytes differ.
        std::fs::write(&bashrc, &installed).unwrap();
        let moved = c.clone().with_binary("/elsewhere/tabsmith".into());
        let e = rc.is_installed(&moved).unwrap_err().to_string();
        assert!(e.contains("bashrc.pre.bash should contain:\n[ -n \"$BASH_VERSION\" ] && [ -x '/elsewhere/tabsmith' ]"), "{e}");
        rc.install(&moved).unwrap();
        rc.is_installed(&moved).unwrap();

        // Opt-out marker wins.
        std::fs::write(&bashrc, "  #  TABSMITH ignore\necho hi\n").unwrap();
        rc.is_installed(&c).unwrap();
        rc.install(&c).unwrap();
        assert_eq!(
            std::fs::read_to_string(&bashrc).unwrap(),
            "  #  TABSMITH ignore\necho hi\n"
        );
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn integrations_enumeration_and_whole_files() {
        let temp_dir = TempDir::new("enum");
        let dir = temp_dir.path().to_path_buf();
        let c = ctx(&dir);
        std::fs::create_dir_all(c.home()).unwrap();
        let names = |s| {
            integrations(&c, s)
                .unwrap()
                .iter()
                .map(Integration::file_name)
                .collect::<Vec<_>>()
        };
        assert_eq!(names(Shell::Bash), [".bashrc", ".profile"]);
        std::fs::write(c.home().join(".bash_profile"), "").unwrap();
        assert_eq!(names(Shell::Bash), [".bashrc", ".bash_profile"]);
        assert_eq!(names(Shell::Zsh), [".zshrc", ".zprofile"]);

        let fish = integrations(&c, Shell::Fish).unwrap();
        assert_eq!(
            fish.iter().map(Integration::describe).collect::<Vec<_>>(),
            ["fish pre", "fish post"]
        );
        for i in &fish {
            i.install(&c).unwrap();
            i.is_installed(&c).unwrap();
            syntax_ok(Shell::Fish, i.path());
        }
        assert_eq!(
            std::fs::read_to_string(c.fish_config_dir().join("conf.d/00_tabsmith_pre.fish")).unwrap(),
            "test -x '/opt/tab smith/it\\'s/tabsmith'; and eval ('/opt/tab smith/it\\'s/tabsmith' init fish pre --rcfile 00_tabsmith_pre | string split0)\n"
        );
        for i in &fish {
            i.uninstall(&c).unwrap();
            assert!(!i.path().exists());
        }

        // pwsh: the profile directory is created; script files are `profile.<when>.ps1`.
        let pwsh = &integrations(&c, Shell::Pwsh).unwrap()[0];
        assert_eq!(
            pwsh.describe(),
            "pwsh pre post into Microsoft.PowerShell_profile.ps1"
        );
        pwsh.install(&c).unwrap();
        pwsh.is_installed(&c).unwrap();
        let text = std::fs::read_to_string(pwsh.path()).unwrap();
        assert!(text.contains(r#"if (Test-Path "${HOME}/.tabsmith/shell/profile.pre.ps1") { . "${HOME}/.tabsmith/shell/profile.pre.ps1" }"#));
        syntax_ok(Shell::Pwsh, pwsh.path());
        let _ = std::fs::remove_dir_all(dir);
    }

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

    /// A UTF-8 BOM (VS Code, ISE and Windows PowerShell save .ps1 files with one) stays the first bytes: install
    /// puts the pre block after it, is_installed sees through it, a reinstall is a no-op, uninstall keeps it.
    #[test]
    fn integrations_dotfile_bom_stays_first() {
        let temp_dir = TempDir::new("bom");
        let dir = temp_dir.path().to_path_buf();
        let c = ctx(&dir);
        let pwsh = &integrations(&c, Shell::Pwsh).unwrap()[0];
        std::fs::create_dir_all(pwsh.path().parent().unwrap()).unwrap();
        let user = "Write-Host USERLINE\r\nSet-PSReadLineOption -PredictionSource History\r\n";
        std::fs::write(pwsh.path(), format!("\u{feff}{user}")).unwrap();
        pwsh.install(&c).unwrap();
        let once = std::fs::read_to_string(pwsh.path()).unwrap();
        assert!(
            once.starts_with(&format!("\u{feff}{PRE_MARKER}\n"))
                && once.matches('\u{feff}').count() == 1,
            "{once:?}"
        );
        assert!(
            once.contains(&format!("\nWrite-Host USERLINE\r\n")),
            "{once:?}"
        );
        pwsh.is_installed(&c).unwrap();
        pwsh.install(&c).unwrap();
        assert_eq!(std::fs::read_to_string(pwsh.path()).unwrap(), once);
        syntax_ok(Shell::Pwsh, pwsh.path());

        // An editor re-saves a plain install with a BOM: still installed; a forced reinstall keeps one block.
        pwsh.uninstall(&c).unwrap();
        let back = std::fs::read_to_string(pwsh.path()).unwrap();
        assert!(
            back.starts_with("\u{feff}Write-Host USERLINE\r\n") && !back.contains("tabsmith"),
            "{back:?}"
        );
        std::fs::write(pwsh.path(), user).unwrap();
        pwsh.install(&c).unwrap();
        let plain = std::fs::read_to_string(pwsh.path()).unwrap();
        std::fs::write(pwsh.path(), format!("\u{feff}{plain}")).unwrap();
        pwsh.is_installed(&c).unwrap();
        pwsh.uninstall(&c).unwrap();
        pwsh.install(&c).unwrap();
        let again = std::fs::read_to_string(pwsh.path()).unwrap();
        assert_eq!(again, format!("\u{feff}{plain}"));
        let _ = std::fs::remove_dir_all(dir);
    }

    /// Ctrl-D (end of input) at a [Y/n] prompt is no, never yes.
    #[test]
    fn integrations_confirm_eof_is_no() {
        assert!(!answer(&b""[..]).unwrap());
        assert!(answer(&b"\n"[..]).unwrap() && answer(&b"Yes\n"[..]).unwrap());
        assert!(!answer(&b"n\n"[..]).unwrap());
    }

    /// Dotfiles are replaced atomically: whoever holds the old file sees it whole, a symlinked dotfile stays a
    /// symlink, permissions stay, no temp file is left, and a read-only file still reports PermissionDenied.
    /// The same scenario against the write itself is shared atomic_file.rs `integrations_dotfile_write_is_atomic`.
    #[cfg(unix)]
    #[test]
    fn integrations_dotfile_write_is_atomic_install() {
        use std::os::unix::fs::PermissionsExt;
        let temp_dir = TempDir::new("atomic");
        let dir = temp_dir.path().to_path_buf();
        let c = ctx(&dir);
        let real = dir.join("dotfiles").join("zshrc");
        std::fs::create_dir_all(real.parent().unwrap()).unwrap();
        std::fs::create_dir_all(c.home()).unwrap();
        std::fs::write(&real, "export A=1\n").unwrap();
        std::fs::set_permissions(&real, std::fs::Permissions::from_mode(0o600)).unwrap();
        let zshrc = c.home().join(".zshrc");
        std::os::unix::fs::symlink(&real, &zshrc).unwrap();
        let old = dir.join("old-zshrc");
        std::fs::hard_link(&real, &old).unwrap(); // the old inode, as a concurrent reader sees it
        let rc = Integration::dotfile(Shell::Zsh, zshrc.clone());
        rc.install(&c).unwrap();
        assert_eq!(
            std::fs::read_to_string(&old).unwrap(),
            "export A=1\n",
            "rewritten in place"
        );
        assert!(std::fs::symlink_metadata(&zshrc)
            .unwrap()
            .file_type()
            .is_symlink());
        assert!(std::fs::read_to_string(&real).unwrap().contains(PRE_MARKER));
        assert_eq!(
            std::fs::metadata(&real).unwrap().permissions().mode() & 0o777,
            0o600
        );
        assert_eq!(
            std::fs::read_dir(real.parent().unwrap()).unwrap().count(),
            1,
            "temp file left behind"
        );
        if !is_root() {
            std::fs::set_permissions(&real, std::fs::Permissions::from_mode(0o444)).unwrap();
            assert!(matches!(
                rc.uninstall(&c),
                Err(ShellIntegrationError::PermissionDenied(_))
            ));
            assert!(std::fs::read_to_string(&real).unwrap().contains(PRE_MARKER));
        }
        // A dangling relative symlink (dotfiles checkout not populated yet): the link stays, its target is created.
        let bashrc = c.home().join(".bashrc");
        std::os::unix::fs::symlink("../dotfiles/bashrc", &bashrc).unwrap();
        let rc = Integration::dotfile(Shell::Bash, bashrc.clone());
        rc.install(&c).unwrap();
        assert!(std::fs::symlink_metadata(&bashrc)
            .unwrap()
            .file_type()
            .is_symlink());
        assert!(std::fs::read_to_string(dir.join("dotfiles/bashrc"))
            .unwrap()
            .contains(PRE_MARKER));
        let _ = std::fs::remove_dir_all(dir);
    }

    /// `integrations uninstall` removes integrations whose scripts name another binary (it moved): is_installed
    /// fails for all of them. With nothing left it reports nothing done.
    #[test]
    fn integrations_uninstall_after_binary_moved() {
        let temp_dir = TempDir::new("moved");
        let dir = temp_dir.path().to_path_buf();
        let c = ctx(&dir);
        let shells = [Shell::Bash, Shell::Fish];
        install_dotfiles(&c, &shells).unwrap();
        let moved = c.clone().with_binary("/elsewhere/tabsmith".into());
        let all: Vec<Integration> = shells
            .iter()
            .flat_map(|s| integrations(&moved, *s).unwrap())
            .collect();
        assert!(all.iter().all(|i| i.is_installed(&moved).is_err()));
        assert!(uninstall_dotfiles(&moved, &shells).unwrap());
        assert!(!std::fs::read_to_string(c.home().join(".bashrc"))
            .unwrap()
            .contains("tabsmith"));
        assert!(all.iter().all(|i| !i.has_traces(&moved)), "{all:?}");
        assert!(!uninstall_dotfiles(&moved, &shells).unwrap());
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

    /// The manual-install lines setup prints are the ones is_installed and uninstall recognise.
    #[test]
    fn integrations_manual_lines_are_recognised() {
        let temp_dir = TempDir::new("manual");
        let dir = temp_dir.path().to_path_buf();
        let c = ctx(&dir);
        std::fs::create_dir_all(c.home()).unwrap();
        let text = manual_text(&c, &[Shell::Bash, Shell::Zsh]);
        let lines: Vec<&str> = text
            .lines()
            .filter_map(|l| l.strip_prefix("zsh:"))
            .map(str::trim)
            .collect();
        assert_eq!(lines.len(), 2, "{text}");
        let zshrc = c.home().join(".zshrc");
        std::fs::write(&zshrc, format!("{}\nexport A=1\n{}\n", lines[0], lines[1])).unwrap();
        let rc = Integration::dotfile(Shell::Zsh, zshrc.clone());
        rc.write_scripts(&c).unwrap(); // what setup's manual path writes
        rc.is_installed(&c).unwrap();
        rc.uninstall(&c).unwrap();
        assert_eq!(std::fs::read_to_string(&zshrc).unwrap(), "export A=1\n");
        assert!(
            !text.contains("pwsh")
                && manual_text(&c, &[Shell::Pwsh]).contains("pwsh ($PROFILE): if (Test-Path")
        );
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
