//! Composition root for the command line: the clap root, the dispatch to each command handler, and the
//! commands that only need the root (`version`, `completion`) plus the error printer.

use crate::shared::infrastructure::terminal_style::epaint;
use crate::{cli, doctor, integrations, pty};
use anyhow::Result;
use clap::{ArgAction, Args, Parser, Subcommand};
use std::io::Write;

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
pub(crate) enum Command {
    /// Customize appearance & behavior
    Settings(cli::SettingsArgs),
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
pub(crate) enum InternalCmd {
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
        Some(Command::Settings(a)) => cli::settings(a),
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

#[derive(Args)]
pub(crate) struct CompletionArgs {
    #[arg(value_enum, default_value_t = clap_complete::Shell::Zsh)]
    shell: clap_complete::Shell,
}

pub(crate) fn completion(args: CompletionArgs) -> Result<i32> {
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
