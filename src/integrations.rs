//! Shell integration: `init` scripts, launch decision, dotfile install/uninstall/status, setup/uninstall flows,
//! and the internal `_ should-launch|get-shell|pre-cmd` commands (SPEC §2.6, §2.7, §2.9, §2.11, §5).
//! OWNER: integrations agent. Shell scripts live in `shell/` and are embedded with `include_str!`.
use crate::types::Shell;
use clap::{Args, Subcommand, ValueEnum};

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
pub enum When {
    Pre,
    Post,
}

#[derive(Args)]
pub struct InitArgs {
    pub shell: ShellArg,
    pub when: When,
    #[arg(long)]
    pub rcfile: Option<String>,
}

#[derive(Subcommand, Clone, Debug)]
pub enum IntegrationTarget {
    /// Shell dotfile integrations
    Dotfiles {
        shell: Option<ShellArg>,
    },
    /// All integrations
    All,
}

#[derive(Subcommand)]
pub enum IntegrationsCmd {
    /// Install an integration
    Install {
        #[command(subcommand)]
        target: IntegrationTarget,
        /// Suppress status messages
        #[arg(long, short)]
        silent: bool,
    },
    /// Uninstall an integration
    Uninstall {
        #[command(subcommand)]
        target: IntegrationTarget,
        #[arg(long, short)]
        silent: bool,
    },
    /// Reinstall an integration
    Reinstall {
        #[command(subcommand)]
        target: IntegrationTarget,
        #[arg(long, short)]
        silent: bool,
    },
    /// Check the status of an integration
    Status {
        #[command(subcommand)]
        target: IntegrationTarget,
        #[arg(long, short, value_enum, default_value_t)]
        format: crate::cli::OutputFormat,
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

pub fn integrations_cmd(cmd: IntegrationsCmd) -> anyhow::Result<i32> {
    let _ = cmd;
    todo!("integrations agent")
}

pub fn init_cmd(args: InitArgs) -> anyhow::Result<i32> {
    let _ = args;
    todo!("integrations agent")
}

/// The integration code `init <shell> <when>` prints (SPEC §5.3), without the trailing newline.
pub fn init_script(shell: Shell, when: When) -> anyhow::Result<String> {
    let _ = (shell, when);
    todo!("integrations agent")
}

pub fn setup(args: SetupArgs) -> anyhow::Result<i32> {
    let _ = args;
    todo!("integrations agent")
}

pub fn uninstall(args: UninstallArgs) -> anyhow::Result<i32> {
    let _ = args;
    todo!("integrations agent")
}

/// Launch decision (SPEC §5.2): (launch?, reason line).
pub fn should_launch() -> (bool, String) {
    todo!("integrations agent")
}

pub fn should_launch_cmd() -> anyhow::Result<i32> {
    todo!("integrations agent")
}

pub fn get_shell_cmd() -> anyhow::Result<i32> {
    todo!("integrations agent")
}

pub fn pre_cmd(alias: Option<String>) -> anyhow::Result<i32> {
    let _ = alias;
    todo!("integrations agent")
}
