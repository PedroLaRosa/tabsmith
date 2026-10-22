//! Small user-facing commands: settings, theme, inline, diagnostic, completion, debug (SPEC §2.3–2.5, §2.10).
//! OWNER: cli agent.
use clap::{Args, Subcommand, ValueEnum};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, ValueEnum)]
pub enum OutputFormat {
    #[default]
    Plain,
    Json,
    JsonPretty,
}

#[derive(Args)]
pub struct SettingsArgs {
    #[command(subcommand)]
    pub cmd: Option<SettingsCmd>,
    /// Key of the setting
    pub key: Option<String>,
    /// Value of the setting (JSON, or a plain string)
    pub value: Option<String>,
    /// Delete a setting (wildcards * and ? allowed)
    #[arg(long, short)]
    pub delete: bool,
    /// Output format
    #[arg(long, short, value_enum, default_value_t)]
    pub format: OutputFormat,
}

#[derive(Subcommand)]
pub enum SettingsCmd {
    /// Open the settings file
    Open,
    /// List settings
    List {
        /// List all available settings with descriptions
        #[arg(long)]
        all: bool,
        #[arg(long, short, value_enum, default_value_t)]
        format: OutputFormat,
    },
    #[command(hide = true)]
    All {
        #[arg(long, short, value_enum, default_value_t)]
        format: OutputFormat,
    },
}

#[derive(Args)]
pub struct ThemeArgs {
    /// List all available themes
    #[arg(long, conflicts_with_all = ["folder", "theme"])]
    pub list: bool,
    /// Print the theme directory path
    #[arg(long, conflicts_with = "theme")]
    pub folder: bool,
    /// Theme name: dark, light, system, or a custom theme
    pub theme: Option<String>,
}

#[derive(Subcommand)]
pub enum InlineCmd {
    /// Enable inline suggestions
    Enable,
    /// Disable inline suggestions
    Disable,
    /// Show current status
    Status,
    /// Select the inline suggestion source (history, specs, history+specs or an LLM profile)
    #[command(alias = "set-source")]
    SetCustomization { name: Option<String> },
    /// Show the available inline sources
    #[command(alias = "show-sources")]
    ShowCustomizations {
        #[arg(long, short, value_enum, default_value_t)]
        format: OutputFormat,
    },
}

#[derive(Args)]
pub struct DiagnosticArgs {
    #[arg(long, short, value_enum, default_value_t)]
    pub format: OutputFormat,
}

#[derive(Args)]
pub struct CompletionArgs {
    #[arg(value_enum, default_value_t = clap_complete::Shell::Zsh)]
    pub shell: clap_complete::Shell,
}

#[derive(Subcommand)]
pub enum DebugCmd {
    /// Print the bytes of every key press (Ctrl-D exits)
    KeyTester,
    /// Start a shell with only the tabsmith integration loaded
    Shell { shell: Option<crate::integrations::ShellArg> },
    /// Show log file paths and the end of pty.log
    Logs,
}

pub fn settings(args: SettingsArgs) -> anyhow::Result<i32> {
    let _ = args;
    todo!("cli agent")
}

pub fn theme(args: ThemeArgs) -> anyhow::Result<i32> {
    let _ = args;
    todo!("cli agent")
}

pub fn inline(cmd: InlineCmd) -> anyhow::Result<i32> {
    let _ = cmd;
    todo!("cli agent")
}

pub fn diagnostic(args: DiagnosticArgs) -> anyhow::Result<i32> {
    let _ = args;
    todo!("cli agent")
}

pub fn completion(args: CompletionArgs) -> anyhow::Result<i32> {
    let _ = args;
    todo!("cli agent")
}

pub fn debug(cmd: DebugCmd) -> anyhow::Result<i32> {
    let _ = cmd;
    todo!("cli agent")
}

/// `error: <msg>` on stderr (`error:` bold red when stderr is a tty).
pub fn print_error(err: &anyhow::Error) {
    use std::io::IsTerminal;
    if std::io::stderr().is_terminal() {
        eprintln!("\x1b[1;31merror:\x1b[0m {err:#}");
    } else {
        eprintln!("error: {err:#}");
    }
}
