//! tabsmith — IDE-style autocomplete for your terminal (dropdown menu + inline ghost text),
//! driven by the Fig completion specs. See docs/SPEC.md.
#![allow(dead_code)]

mod cli;
mod doctor;
mod engine;
mod icons;
mod inline;
mod integrations;
mod js;
mod keys;
mod logger;
mod menu;
mod parser;
mod pty;
mod render;
mod settings;
mod shell_parser;
mod spec;
mod specs;
mod suggest;
mod tracker;
mod types;

use clap::{ArgAction, Parser, Subcommand};

#[derive(Parser)]
#[command(
    name = "tabsmith",
    version,
    about = "IDE-style autocomplete for your terminal"
)]
pub struct Cli {
    /// Increase log verbosity (-v warn, -vv info, -vvv debug, -vvvv trace)
    #[arg(short, long, action = ArgAction::Count, global = true)]
    verbose: u8,
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand)]
pub enum Command {
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
    Completion(cli::CompletionArgs),
    /// Debug tabsmith
    #[command(subcommand)]
    Debug(cli::DebugCmd),
    /// Internal subcommands used by the shell integrations
    #[command(name = "_", alias = "internal", hide = true, subcommand)]
    Internal(InternalCmd),
}

#[derive(Subcommand)]
pub enum InternalCmd {
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

fn main() {
    let cli = Cli::parse();
    let is_pty = matches!(
        cli.command,
        Some(Command::Internal(InternalCmd::Pty { .. }))
    );
    logger::init(if is_pty { "pty" } else { "cli" }, cli.verbose, !is_pty);

    let result = match cli.command {
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
        Some(Command::Completion(a)) => cli::completion(a),
        Some(Command::Debug(c)) => cli::debug(c),
        Some(Command::Internal(c)) => match c {
            InternalCmd::Pty { shell, cmd } => Ok(pty::run(shell, cmd)),
            InternalCmd::ShouldLaunch => integrations::should_launch_cmd(),
            InternalCmd::GetShell => integrations::get_shell_cmd(),
            InternalCmd::PreCmd { alias } => integrations::pre_cmd(alias),
        },
    };

    match result {
        Ok(code) => std::process::exit(code),
        Err(err) => {
            logger::error(&format!("{err:#}"));
            cli::print_error(&err);
            std::process::exit(1);
        }
    }
}
