//! Composition root for the command line: the clap root, the dispatch to each command handler, and the
//! commands that only need the root (`version`, `completion`) plus the error printer.

use crate::bootstrap::bridges::{GeneratorProcessBridge, SpecBundleBridge};
use crate::bootstrap::channel_completion_engine::start_completion_engine;
use crate::bootstrap::diagnostics_bridges::{BundledSpecsBridge, CheckFixerBridge, IntegrationProbeBridge};
use crate::modules::diagnostics::infrastructure::adapters::file_repairs::FileRepairs;
use crate::modules::diagnostics::infrastructure::cli::debug_logs_command::run_debug_logs_command;
use crate::modules::diagnostics::infrastructure::cli::diagnostic_command::{run_diagnostic_command, DiagnosticArgs};
use crate::modules::diagnostics::infrastructure::cli::doctor_command::{run_doctor_command, DoctorArgs};
use crate::modules::diagnostics::infrastructure::diagnostics_module::{build_diagnostics_module, DiagnosticsModule};
use crate::modules::dropdown::infrastructure::cli::theme_command::{run_theme_command, ThemeArgs};
use crate::modules::dropdown::infrastructure::dropdown_module::{build_dropdown_module, DropdownModule};
use crate::modules::ghost_text::infrastructure::cli::inline_command::{run_inline_command, InlineCmd};
use crate::modules::ghost_text::infrastructure::inline_command_module::{
    build_inline_command_module, InlineCommandModule,
};
use crate::modules::self_update::infrastructure::cli::background_update::start_background_update;
use crate::modules::self_update::infrastructure::cli::update_command::run_update_command;
use crate::modules::self_update::infrastructure::self_update_module::{build_self_update_module, SelfUpdateModule};
use crate::modules::settings::infrastructure::adapters::json_file_settings_repository::JsonFileSettingsRepository;
use crate::modules::settings::infrastructure::cli::settings_command::{run_settings_command, SettingsArgs};
use crate::modules::settings::infrastructure::settings_module::build_settings_module;
use crate::modules::shared::domain::value_objects::shell::Shell;
use crate::modules::shared::infrastructure::data_directory_from_env::data_directory_from_env;
use crate::modules::shared::infrastructure::shared_dependencies::SharedDependencies;
use crate::modules::shared::infrastructure::terminal_style::epaint;
use crate::modules::shell_integration::application::integration_installer::IntegrationInstaller;
use crate::modules::shell_integration::application::render_init_script_use_case::RenderInitScriptUseCase;
use crate::modules::shell_integration::domain::repositories::restart_reminder_repository::RestartReminderRepository;
use crate::modules::shell_integration::domain::value_objects::integration_phase::IntegrationPhase;
use crate::modules::shell_integration::infrastructure::adapters::embedded_shell_scripts::EmbeddedShellScripts;
use crate::modules::shell_integration::infrastructure::adapters::file_backup_repository::FileBackupRepository;
use crate::modules::shell_integration::infrastructure::adapters::file_dotfile_repository::FileDotfileRepository;
use crate::modules::shell_integration::infrastructure::adapters::os_shell_environment::OsShellEnvironment;
use crate::modules::shell_integration::infrastructure::adapters::state_restart_reminder_repository::StateRestartReminderRepository;
use crate::modules::shell_integration::infrastructure::cli::get_shell_command::run_get_shell_command;
use crate::modules::shell_integration::infrastructure::cli::init_command::{init_script, run_init_command, InitArgs, ShellArg};
use crate::modules::shell_integration::infrastructure::cli::integrations_command::{run_integrations_command, IntegrationsCmd};
use crate::modules::shell_integration::infrastructure::cli::setup_command::{run_setup_command, SetupArgs};
use crate::modules::shell_integration::infrastructure::cli::should_launch_command::run_should_launch_command;
use crate::modules::shell_integration::infrastructure::cli::uninstall_command::{run_uninstall_command, UninstallArgs};
use crate::modules::shell_integration::infrastructure::shell_integration_module::{
    build_shell_integration_module, ShellIntegrationModule,
};
use crate::modules::specs::infrastructure::specs_module::build_specs_module;
use crate::modules::terminal_session::infrastructure::cli::debug_shell_command::run_debug_shell_command;
use crate::modules::terminal_session::infrastructure::cli::key_tester_command::run_key_tester_command;
use crate::modules::terminal_session::infrastructure::cli::pre_cmd_command::run_pre_cmd_command;
use crate::modules::terminal_session::infrastructure::cli::pty_command::run_pty_command;
use crate::modules::terminal_session::infrastructure::terminal_session_module::{
    build_terminal_session_module, TerminalSessionModule,
};
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
    Doctor(DoctorArgs),
    /// Setup cli components
    #[command(alias = "install")]
    Setup(SetupArgs),
    /// Uninstall tabsmith
    Uninstall(UninstallArgs),
    /// Update tabsmith to the latest release
    Update,
    /// Show the version
    #[command(hide = true)]
    Version,
    /// Run diagnostic tests
    #[command(alias = "diagnostics")]
    Diagnostic(DiagnosticArgs),
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
        Some(Command::Doctor(a)) => run_doctor_command(a, diagnostics_module),
        Some(Command::Setup(a)) => run_setup_command(a, &shell_integration_module()),
        Some(Command::Uninstall(a)) => run_uninstall_command(a, &shell_integration_module()),
        Some(Command::Update) => run_update_command(self_update_module().as_ref()),
        Some(Command::Version) => {
            println!("tabsmith {}", env!("CARGO_PKG_VERSION"));
            Ok(0)
        }
        Some(Command::Diagnostic(a)) => run_diagnostic_command(a, &diagnostics_module()),
        Some(Command::Completion(a)) => completion(a),
        Some(Command::Debug(c)) => match c {
            DebugCmd::KeyTester => run_key_tester_command(),
            DebugCmd::Shell { shell } => {
                run_debug_shell_command(shell.map(Into::into), &terminal_session_module(), debug_shell_post_script)
            }
            DebugCmd::Logs => run_debug_logs_command(&diagnostics_module()),
        },
        Some(Command::Internal(c)) => match c {
            InternalCmd::Pty { shell, cmd } => {
                start_background_update(self_update_module);
                run_pty_command(shell, cmd, &terminal_session_module())
            }
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

/// The self-update module over the settings file the environment names; `None` on a platform without release
/// binaries. Built on the thread that runs the update.
fn self_update_module() -> Option<SelfUpdateModule> {
    let dependencies = SharedDependencies::from_env();
    let settings = Rc::new(JsonFileSettingsRepository::new(dependencies.data_directory.clone()));
    build_self_update_module(&dependencies, settings)
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

/// The diagnostics module (doctor, diagnostic, debug logs), built for the one command that runs. Building only
/// constructs: nothing is read, looked up or run until a command needs it.
fn diagnostics_module() -> DiagnosticsModule {
    let dependencies = SharedDependencies::from_env();
    let settings = Rc::new(JsonFileSettingsRepository::new(dependencies.data_directory.clone()));
    let restart_reminder: Rc<dyn RestartReminderRepository> =
        Rc::new(StateRestartReminderRepository::new(dependencies.data_directory.clone()));
    let installer = Rc::new(IntegrationInstaller::new(
        Rc::new(FileDotfileRepository),
        Rc::new(FileBackupRepository),
        dependencies.clock.clone(),
        Rc::new(OsShellEnvironment),
    ));
    let spec_bundle = build_specs_module(&dependencies).install_spec_bundle;
    build_diagnostics_module(
        &dependencies,
        settings,
        restart_reminder.clone(),
        Rc::new(IntegrationProbeBridge::new(installer.clone(), dependencies.data_directory.clone())),
        Rc::new(BundledSpecsBridge),
        Rc::new(CheckFixerBridge::new(
            installer,
            restart_reminder,
            spec_bundle,
            FileRepairs::new(dependencies.clock.clone()),
        )),
    )
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
