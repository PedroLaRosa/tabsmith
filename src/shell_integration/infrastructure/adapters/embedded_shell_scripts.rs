use crate::shared::domain::value_objects::shell::Shell;
use crate::shell_integration::application::ports::init_script_templates::InitScriptTemplates;
use crate::shell_integration::domain::value_objects::integration_phase::IntegrationPhase;

const PRE_SH: &str = include_str!("../../../../shell/pre.sh");
const POST_ZSH: &str = include_str!("../../../../shell/post.zsh");
const POST_BASH: &str = include_str!("../../../../shell/post.bash");
const PRE_FISH: &str = include_str!("../../../../shell/pre.fish");
const POST_FISH: &str = include_str!("../../../../shell/post.fish");
const PRE_PS1: &str = include_str!("../../../../shell/pre.ps1");
const POST_PS1: &str = include_str!("../../../../shell/post.ps1");

/// The `shell/` scripts, embedded in the binary at build time.
pub(crate) struct EmbeddedShellScripts;

impl InitScriptTemplates for EmbeddedShellScripts {
    fn template(&self, shell: Shell, phase: IntegrationPhase) -> &'static str {
        match (shell, phase) {
            (Shell::Bash | Shell::Zsh, IntegrationPhase::Pre) => PRE_SH,
            (Shell::Bash, IntegrationPhase::Post) => POST_BASH,
            (Shell::Zsh, IntegrationPhase::Post) => POST_ZSH,
            (Shell::Fish, IntegrationPhase::Pre) => PRE_FISH,
            (Shell::Fish, IntegrationPhase::Post) => POST_FISH,
            (Shell::Pwsh, IntegrationPhase::Pre) => PRE_PS1,
            (Shell::Pwsh, IntegrationPhase::Post) => POST_PS1,
        }
    }
}

#[cfg(test)]
mod tests;
