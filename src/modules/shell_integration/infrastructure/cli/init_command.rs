use crate::modules::shared::domain::value_objects::shell::Shell;
use crate::modules::shared::infrastructure::logger;
use crate::modules::shell_integration::application::render_init_script_use_case::RenderInitScriptUseCase;
use crate::modules::shell_integration::domain::shell_integration_error::ShellIntegrationError;
use crate::modules::shell_integration::domain::value_objects::init_script::InitScript;
use crate::modules::shell_integration::domain::value_objects::integration_phase::IntegrationPhase;
use crate::modules::shell_integration::infrastructure::shell_integration_module::ShellIntegrationModule;
use clap::{Args, ValueEnum};
use std::io::Write;

#[derive(Clone, Copy, Debug, PartialEq, Eq, ValueEnum)]
pub(crate) enum ShellArg {
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
pub(crate) enum IntegrationPhaseArg {
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
pub(crate) struct InitArgs {
    pub(crate) shell: ShellArg,
    pub(crate) when: IntegrationPhaseArg,
    #[arg(long)]
    pub(crate) rcfile: Option<String>,
}

pub(crate) fn run_init_command(args: InitArgs, module: &ShellIntegrationModule) -> anyhow::Result<i32> {
    if let Some(rc) = &args.rcfile {
        logger::debug(&format!(
            "init {:?} {} --rcfile {rc}",
            args.shell,
            IntegrationPhase::from(args.when).name()
        ));
    }
    let out = match init_script(&module.render_init_script, args.shell.into(), args.when.into()) {
        Ok(s) => s,
        Err(e) => format!("# Could not load source: {e:#}"),
    };
    let _ = writeln!(std::io::stdout(), "{out}");
    Ok(0)
}

/// The integration code `init <shell> <phase>` prints (SPEC §5.3), without the trailing newline; the deterministic
/// snapshot when `TABSMITH_INIT_SNAPSHOT_TEST` is set at the moment it is rendered.
pub(crate) fn init_script(
    render: &RenderInitScriptUseCase,
    shell: Shell,
    phase: IntegrationPhase,
) -> Result<String, ShellIntegrationError> {
    render
        .execute(
            shell,
            phase,
            std::env::var_os("TABSMITH_INIT_SNAPSHOT_TEST").is_some(),
        )
        .map(InitScript::into_string)
}
