//! Composition root for the command line: the clap root, the dispatch to each command handler, and the
//! commands that only need the root (`version`, `completion`) plus the error printer.

use crate::bootstrap::bridges::{GeneratorProcessBridge, SpecBundleBridge};
use crate::bootstrap::channel_completion_engine::start_completion_engine;
use crate::dropdown::infrastructure::cli::theme_command::{run_theme_command, ThemeArgs};
use crate::dropdown::infrastructure::dropdown_module::{build_dropdown_module, DropdownModule};
use crate::ghost_text::infrastructure::cli::inline_command::{run_inline_command, InlineCmd};
use crate::ghost_text::infrastructure::inline_command_module::{
    build_inline_command_module, InlineCommandModule,
};
use crate::settings::infrastructure::adapters::json_file_settings_repository::JsonFileSettingsRepository;
use crate::settings::infrastructure::cli::settings_command::{run_settings_command, SettingsArgs};
use crate::settings::infrastructure::settings_module::build_settings_module;
use crate::shared::domain::value_objects::shell::Shell;
use crate::shared::infrastructure::data_directory_from_env::data_directory_from_env;
use crate::shared::infrastructure::shared_dependencies::SharedDependencies;
use crate::shared::infrastructure::terminal_style::epaint;
use crate::shell_integration::application::render_init_script_use_case::RenderInitScriptUseCase;
use crate::shell_integration::domain::value_objects::integration_phase::IntegrationPhase;
use crate::shell_integration::infrastructure::adapters::embedded_shell_scripts::EmbeddedShellScripts;
use crate::shell_integration::infrastructure::adapters::os_shell_environment::OsShellEnvironment;
use crate::shell_integration::infrastructure::cli::get_shell_command::run_get_shell_command;
use crate::shell_integration::infrastructure::cli::init_command::{init_script, run_init_command, InitArgs, ShellArg};
use crate::shell_integration::infrastructure::cli::integrations_command::{run_integrations_command, IntegrationsCmd};
use crate::shell_integration::infrastructure::cli::setup_command::{run_setup_command, SetupArgs};
use crate::shell_integration::infrastructure::cli::should_launch_command::run_should_launch_command;
use crate::shell_integration::infrastructure::cli::uninstall_command::{run_uninstall_command, UninstallArgs};
use crate::shell_integration::infrastructure::shell_integration_module::{
    build_shell_integration_module, ShellIntegrationModule,
};
use crate::specs::infrastructure::specs_module::build_specs_module;
use crate::terminal_session::infrastructure::cli::debug_shell_command::run_debug_shell_command;
use crate::terminal_session::infrastructure::cli::key_tester_command::run_key_tester_command;
use crate::terminal_session::infrastructure::cli::pre_cmd_command::run_pre_cmd_command;
use crate::terminal_session::infrastructure::cli::pty_command::run_pty_command;
use crate::terminal_session::infrastructure::terminal_session_module::{
    build_terminal_session_module, TerminalSessionModule,
};
use crate::{cli, doctor};
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
    Theme(ThemeArgs),
    /// Inline shell completions
    #[command(subcommand)]
    Inline(InlineCmd),
    /// Manage system integrations
    #[command(subcommand)]
    Integrations(IntegrationsCmd),
    /// Generate the dotfiles for the given shell
    Init(InitArgs),
    /// Fix and diagnose common issues
    Doctor(doctor::DoctorArgs),
    /// Setup cli components
    #[command(alias = "install")]
    Setup(SetupArgs),
    /// Uninstall tabsmith
    Uninstall(UninstallArgs),
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
    Debug(DebugCmd),
    /// Internal subcommands used by the shell integrations
    #[command(name = "_", alias = "internal", hide = true, subcommand)]
    Internal(InternalCmd),
}

#[derive(Subcommand)]
enum DebugCmd {
    /// Print the bytes of every key press (Ctrl-D exits)
    KeyTester,
    /// Start a shell with only the tabsmith integration loaded
    Shell {
        shell: Option<ShellArg>,
    },
    /// Show log file paths and the end of pty.log
    Logs,
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
        Some(Command::Theme(a)) => run_theme_command(a, &dropdown_module()),
        Some(Command::Inline(c)) => run_inline_command(c, &inline_command_module()),
        Some(Command::Integrations(c)) => run_integrations_command(c, &shell_integration_module()),
        Some(Command::Init(a)) => run_init_command(a, &shell_integration_module()),
        Some(Command::Doctor(a)) => doctor::run(a),
        Some(Command::Setup(a)) => run_setup_command(a, &shell_integration_module()),
        Some(Command::Uninstall(a)) => run_uninstall_command(a, &shell_integration_module()),
        Some(Command::Version) => {
            println!("tabsmith {}", env!("CARGO_PKG_VERSION"));
            Ok(0)
        }
        Some(Command::Diagnostic(a)) => cli::diagnostic(a),
        Some(Command::Completion(a)) => completion(a),
        Some(Command::Debug(c)) => match c {
            DebugCmd::KeyTester => run_key_tester_command(),
            DebugCmd::Shell { shell } => {
                run_debug_shell_command(shell.map(Into::into), &terminal_session_module(), debug_shell_post_script)
            }
            DebugCmd::Logs => cli::debug_logs(),
        },
        Some(Command::Internal(c)) => match c {
            InternalCmd::Pty { shell, cmd } => run_pty_command(shell, cmd, &terminal_session_module()),
            InternalCmd::ShouldLaunch => run_should_launch_command(&shell_integration_module()),
            InternalCmd::GetShell => run_get_shell_command(),
            InternalCmd::PreCmd { alias } => run_pre_cmd_command(alias, &terminal_session_module()),
        },
    }
}

/// The module of the terminal session (the PTY wrapper, the debug shell and `_ pre-cmd`), over the settings file and
/// the themes folder the environment names (resolved when a session needs them; building reads no file).
pub(crate) fn terminal_session_module() -> TerminalSessionModule {
    let dependencies = SharedDependencies::from_env();
    let settings = Rc::new(JsonFileSettingsRepository::new(dependencies.data_directory.clone()));
    let themes = build_dropdown_module(&dependencies, settings.clone()).themes;
    build_terminal_session_module(
        &dependencies,
        settings,
        themes,
        Rc::new(GeneratorProcessBridge),
        start_completion_engine,
    )
}

/// The debug shell's post-integration script, rendered for the shell it starts.
pub(crate) fn debug_shell_post_script(shell: Shell) -> Result<String> {
    init_script(
        &RenderInitScriptUseCase::new(Rc::new(OsShellEnvironment), Rc::new(EmbeddedShellScripts)),
        shell,
        IntegrationPhase::Post,
    )
    .map_err(Into::into)
}

/// The inline command's module over the settings file the environment names (resolved when a command needs it).
fn inline_command_module() -> InlineCommandModule {
    build_inline_command_module(Rc::new(JsonFileSettingsRepository::new(data_directory_from_env())))
}

/// The theme command's module over the settings file the environment names (resolved when a command needs it).
fn dropdown_module() -> DropdownModule {
    let dependencies = SharedDependencies::from_env();
    let settings = Rc::new(JsonFileSettingsRepository::new(dependencies.data_directory.clone()));
    build_dropdown_module(&dependencies, settings)
}

/// The shell integration commands' module, built for the one command that runs.
fn shell_integration_module() -> ShellIntegrationModule {
    let dependencies = SharedDependencies::from_env();
    let spec_bundle = SpecBundleBridge::new(build_specs_module(&dependencies).install_spec_bundle);
    build_shell_integration_module(&dependencies, Rc::new(spec_bundle))
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
