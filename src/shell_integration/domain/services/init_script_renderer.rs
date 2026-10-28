use crate::shared::domain::services::shell_quoting::quote;
use crate::shared::domain::value_objects::shell::Shell;
use crate::shell_integration::domain::value_objects::init_script::InitScript;
use crate::shell_integration::domain::value_objects::integration_phase::IntegrationPhase;

/// The output of `init <shell> <phase>` (SPEC §5.3), without the trailing newline.
pub(crate) fn render(
    shell: Shell,
    _phase: IntegrationPhase,
    template: &str,
    binary: &str,
    _parent_shell: impl FnOnce() -> Option<String>,
    _launches: impl FnOnce() -> bool,
) -> InitScript {
    InitScript::new(
        template
            .trim_end_matches('\n')
            .replace("{{BIN}}", &quote(shell, binary)),
    )
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

    #[test]
    fn should_replace_every_binary_placeholder_with_the_binary_quoted_for_the_shell() {
        let template = "body {{BIN}} {{BIN}}\n\n";

        let script = render(
            Shell::Fish,
            IntegrationPhase::Post,
            template,
            "/opt/tab smith/it's/tabsmith",
            || None,
            || true,
        );

        assert_eq!(
            script.text(),
            r"body '/opt/tab smith/it\'s/tabsmith' '/opt/tab smith/it\'s/tabsmith'"
        );
    }
}
