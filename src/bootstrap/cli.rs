//! Composition root for the command line: the clap root, the dispatch to each command handler, and the
//! commands that only need the root (`version`, `completion`) plus the error printer.

use crate::bootstrap::bridges::SpecBundleBridge;
use crate::settings::infrastructure::cli::settings_command::{run_settings_command, SettingsArgs};
use crate::settings::infrastructure::settings_module::build_settings_module;
use crate::shared::infrastructure::shared_dependencies::SharedDependencies;
use crate::shared::infrastructure::terminal_style::epaint;
use crate::shell_integration::infrastructure::cli::init_command::{run_init_command, InitArgs};
use crate::shell_integration::infrastructure::shell_integration_module::{
    build_shell_integration_module, ShellIntegrationModule,
};
use crate::{cli, doctor, integrations, pty};
use anyhow::Result;
use clap::{ArgAction, Args, Parser, Subcommand};
use std::io::Write;
use std::rc::Rc;

#[derive(Parser)]
#[command(
    name = "tabsmith",
    version,
    about = "IDE-style autocomplete for your terminal"
)]
pub(crate) struct Cli {
    /// Increase log verbosity (-v warn, -vv info, -vvv debug, -vvvv trace)
    #[arg(short, long, action = ArgAction::Count, global = true)]
    verbose: u8,
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand)]
enum Command {
    /// Customize appearance & behavior
    Settings(SettingsArgs),
    /// Get or set theme
    Theme(cli::ThemeArgs),
    /// Inline shell completions
    #[command(subcommand)]
    Inline(cli::InlineCmd),
    /// Manage system integrations
    #[command(subcommand)]
    Integrations(integrations::IntegrationsCmd),
    /// Generate the dotfiles for the given shell
    Init(InitArgs),
    /// Fix and diagnose common issues
    Doctor(doctor::DoctorArgs),
    /// Setup cli components
    #[command(alias = "install")]
    Setup(integrations::SetupArgs),
    /// Uninstall tabsmith
    Uninstall(integrations::UninstallArgs),
    /// Show the version
    #[command(hide = true)]
    Version,
    /// Run diagnostic tests
    #[command(alias = "diagnostics")]
    Diagnostic(cli::DiagnosticArgs),
    /// Generate shell completions for tabsmith itself
    #[command(hide = true)]
    Completion(CompletionArgs),
    /// Debug tabsmith
    #[command(subcommand)]
    Debug(cli::DebugCmd),
    /// Internal subcommands used by the shell integrations
    #[command(name = "_", alias = "internal", hide = true, subcommand)]
    Internal(InternalCmd),
}

#[derive(Subcommand)]
enum InternalCmd {
    /// Run the PTY wrapper (the autocomplete session)
    Pty {
        /// Shell executable to spawn
        #[arg(long)]
        shell: Option<String>,
        /// Explicit command to run instead of the shell
        #[arg(last = true)]
        cmd: Vec<String>,
    },
    /// Decide whether the wrapper should launch in this environment (exit 0 = launch)
    ShouldLaunch,
    /// Print the parent process' executable path
    GetShell,
    /// Report the shell environment and aliases for the current session
    PreCmd {
        #[arg(long, allow_hyphen_values = true)]
        alias: Option<String>,
    },
}

impl Cli {
    pub(crate) fn verbose(&self) -> u8 {
        self.verbose
    }

    /// True when this invocation runs the PTY wrapper (the autocomplete session).
    pub(crate) fn runs_pty(&self) -> bool {
        matches!(
            self.command,
            Some(Command::Internal(InternalCmd::Pty { .. }))
        )
    }
}

pub(crate) fn run(cli: Cli) -> Result<i32> {
    match cli.command {
        None => {
            use clap::CommandFactory;
            Cli::command().print_help().map(|_| 0).map_err(Into::into)
        }
        Some(Command::Settings(a)) => run_settings_command(
            a,
            &build_settings_module(&SharedDependencies::from_env()),
            print_settings_help,
        ),
        Some(Command::Theme(a)) => cli::theme(a),
        Some(Command::Inline(c)) => cli::inline(c),
        Some(Command::Integrations(c)) => integrations::integrations_cmd(c),
        Some(Command::Init(a)) => run_init_command(a, &shell_integration_module()),
        Some(Command::Doctor(a)) => doctor::run(a),
        Some(Command::Setup(a)) => integrations::setup(a),
        Some(Command::Uninstall(a)) => integrations::uninstall(a),
        Some(Command::Version) => {
            println!("tabsmith {}", env!("CARGO_PKG_VERSION"));
            Ok(0)
        }
        Some(Command::Diagnostic(a)) => cli::diagnostic(a),
        Some(Command::Completion(a)) => completion(a),
        Some(Command::Debug(c)) => cli::debug(c),
        Some(Command::Internal(c)) => match c {
            InternalCmd::Pty { shell, cmd } => Ok(pty::run(shell, cmd)),
            InternalCmd::ShouldLaunch => integrations::should_launch_cmd(),
            InternalCmd::GetShell => integrations::get_shell_cmd(),
            InternalCmd::PreCmd { alias } => integrations::pre_cmd(alias),
        },
    }
}

/// The shell integration commands' module, built for the one command that runs.
fn shell_integration_module() -> ShellIntegrationModule {
    build_shell_integration_module(&SharedDependencies::from_env(), Rc::new(SpecBundleBridge))
}

/// `tabsmith settings --help` as the root command prints it, with the global `-v`.
fn print_settings_help() -> Result<()> {
    if let Err(help) = Cli::try_parse_from(["tabsmith", "settings", "--help"]) {
        help.print()?;
    }
    Ok(())
}

#[derive(Args)]
struct CompletionArgs {
    #[arg(value_enum, default_value_t = clap_complete::Shell::Zsh)]
    shell: clap_complete::Shell,
}

fn completion(args: CompletionArgs) -> Result<i32> {
    // Buffered: clap_complete panics on a write error (e.g. `| head`).
    let mut buf = Vec::new();
    clap_complete::generate(
        args.shell,
        &mut <Cli as clap::CommandFactory>::command(),
        "tabsmith",
        &mut buf,
    );
    std::io::stdout().write_all(&buf)?;
    Ok(0)
}

/// `error: <msg>` on stderr (`error:` bold red when stderr is a tty).
pub(crate) fn print_error(err: &anyhow::Error) {
    eprintln!("{} {err:#}", epaint("1;31", "error:"));
}
