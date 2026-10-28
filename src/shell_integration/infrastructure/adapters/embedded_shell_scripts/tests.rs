//! Embedded shell scripts tests: the `shell/` files each shell and phase gets, the `init` snapshots, and the
//! scripts run in real shells (each skipped when its shell is not installed).
use super::EmbeddedShellScripts;
use crate::shared::domain::value_objects::shell::Shell;
use crate::shell_integration::application::ports::init_script_templates::InitScriptTemplates;
use crate::shell_integration::domain::value_objects::integration_phase::IntegrationPhase;
use std::path::Path;

/// Bash and zsh share `pre.sh`; every other shell and phase has a `shell/` file of its own.
#[test]
fn should_embed_the_shell_file_of_each_shell_and_phase() {
    let cases = [
        (Shell::Bash, IntegrationPhase::Pre, "pre.sh"),
        (Shell::Bash, IntegrationPhase::Post, "post.bash"),
        (Shell::Zsh, IntegrationPhase::Pre, "pre.sh"),
        (Shell::Zsh, IntegrationPhase::Post, "post.zsh"),
        (Shell::Fish, IntegrationPhase::Pre, "pre.fish"),
        (Shell::Fish, IntegrationPhase::Post, "post.fish"),
        (Shell::Pwsh, IntegrationPhase::Pre, "pre.ps1"),
        (Shell::Pwsh, IntegrationPhase::Post, "post.ps1"),
    ];
    let shell_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("shell");
    let files: Vec<String> = cases
        .iter()
        .map(|(_, _, file)| std::fs::read_to_string(shell_dir.join(file)).unwrap())
        .collect();

    let templates: Vec<&str> = cases
        .iter()
        .map(|(shell, phase, _)| EmbeddedShellScripts.template(*shell, *phase))
        .collect();

    assert_eq!(templates, files);
}
