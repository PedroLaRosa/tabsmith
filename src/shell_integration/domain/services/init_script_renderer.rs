use crate::shared::domain::services::shell_quoting::quote;
use crate::shared::domain::value_objects::shell::Shell;
use crate::shell_integration::domain::value_objects::init_script::InitScript;
use crate::shell_integration::domain::value_objects::integration_phase::IntegrationPhase;

/// The output of `init <shell> <phase>` (SPEC §5.3), without the trailing newline.
pub(crate) fn render(
    shell: Shell,
    phase: IntegrationPhase,
    template: &str,
    binary: &str,
    _parent_shell: impl FnOnce() -> Option<String>,
    launches: impl FnOnce() -> bool,
) -> InitScript {
    let launch = || if launches() { 0 } else { 1 };
    let mut lines = Vec::new();
    if phase == IntegrationPhase::Pre {
        lines.push(format!("set -g SHOULD_TABSMITH_LAUNCH {}", launch()));
    }
    lines.push(
        template
            .trim_end_matches('\n')
            .replace("{{BIN}}", &quote(shell, binary)),
    );
    InitScript::new(lines.join("\n"))
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

    /// SHOULD_TABSMITH_LAUNCH=0 means launch.
    #[test]
    fn should_open_the_fish_pre_script_with_a_zero_launch_flag_when_tabsmith_launches() {
        let template = "body\n";

        let script = render(
            Shell::Fish,
            IntegrationPhase::Pre,
            template,
            "tabsmith",
            || None,
            || true,
        );

        assert_eq!(script.text(), "set -g SHOULD_TABSMITH_LAUNCH 0\nbody");
    }

    #[test]
    fn should_open_the_fish_pre_script_with_a_one_launch_flag_when_tabsmith_does_not_launch() {
        let template = "body\n";

        let script = render(
            Shell::Fish,
            IntegrationPhase::Pre,
            template,
            "tabsmith",
            || None,
            || false,
        );

        assert_eq!(script.text(), "set -g SHOULD_TABSMITH_LAUNCH 1\nbody");
    }
}
