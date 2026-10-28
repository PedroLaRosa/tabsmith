//! Composition root for the command line: the clap root, the dispatch to each command handler, and the
//! commands that only need the root (`version`, `completion`) plus the error printer.

use crate::settings::infrastructure::cli::settings_command::{run_settings_command, SettingsArgs};
use crate::settings::infrastructure::settings_module::build_settings_module;
use crate::shared::infrastructure::data_directory_from_env::data_directory_from_env;
use crate::shared::infrastructure::shared_dependencies::SharedDependencies;
use crate::shared::infrastructure::system_clock::SystemClock;
use crate::shared::infrastructure::terminal_style::epaint;
use crate::{cli, doctor, integrations, pty};
use anyhow::Result;
use clap::{ArgAction, Args, Parser, Subcommand};
use std::io::Write;
use std::sync::Arc;

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
    Init(integrations::InitArgs),
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
            &build_settings_module(&shared_dependencies()),
            print_settings_help,
        ),
        Some(Command::Theme(a)) => cli::theme(a),
        Some(Command::Inline(c)) => cli::inline(c),
        Some(Command::Integrations(c)) => integrations::integrations_cmd(c),
        Some(Command::Init(a)) => integrations::init_cmd(a),
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

/// What every module is built from. Never fails: an unknown home only fails the paths a command asks for.
pub(crate) fn shared_dependencies() -> SharedDependencies {
    SharedDependencies {
        data_directory: data_directory_from_env(),
        clock: Arc::new(SystemClock),
    }
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
