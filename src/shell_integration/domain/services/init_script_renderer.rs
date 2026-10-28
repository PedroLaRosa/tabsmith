use crate::shared::domain::value_objects::shell::Shell;
use crate::shell_integration::domain::value_objects::init_script::InitScript;
use crate::shell_integration::domain::value_objects::integration_phase::IntegrationPhase;

/// The output of `init <shell> <phase>` (SPEC §5.3), without the trailing newline.
pub(crate) fn render(
    _shell: Shell,
    _phase: IntegrationPhase,
    template: &str,
    _binary: &str,
    _parent_shell: impl FnOnce() -> Option<String>,
    _launches: impl FnOnce() -> bool,
) -> InitScript {
    InitScript::new(template.trim_end_matches('\n').to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn should_render_the_post_template_without_its_trailing_newlines_and_add_none() {
        let template = "body\n\n";

        let script = render(
            Shell::Bash,
            IntegrationPhase::Post,
            template,
            "tabsmith",
            || None,
            || true,
        );

        assert_eq!(script.text(), "body");
    }
}
