use crate::shared::domain::services::shell_quoting::quote;
use crate::shared::domain::value_objects::shell::Shell;
use crate::shell_integration::domain::value_objects::init_script::InitScript;
use crate::shell_integration::domain::value_objects::integration_phase::IntegrationPhase;

/// The output of `init <shell> <phase>` (SPEC §5.3), without the trailing newline. Both facts are I/O, so they are
/// asked only when needed: `parent_shell` for a bash or zsh pre script, then `launches` for any pre script.
pub(crate) fn render(
    shell: Shell,
    phase: IntegrationPhase,
    template: &str,
    binary: &str,
    parent_shell: impl FnOnce() -> Option<String>,
    launches: impl FnOnce() -> bool,
) -> InitScript {
    let launch = || if launches() { 0 } else { 1 };
    let mut lines = Vec::new();
    if phase == IntegrationPhase::Pre {
        match shell {
            Shell::Bash | Shell::Zsh => {
                let parent = parent_shell();
                if let Some(p) = parent {
                    lines.push(format!("TABSMITH_SHELL={}", quote(shell, &p)));
                }
                lines.push(format!("SHOULD_TABSMITH_LAUNCH={}", launch()));
            }
            Shell::Fish => lines.push(format!(
                "set -g SHOULD_TABSMITH_LAUNCH {}",
                launch()
            )),
            Shell::Pwsh => lines.push(format!("$TABSMITH_SHOULD_LAUNCH = {}", launch())),
        }
    }
    lines.push(
        template.trim_end_matches('\n')
            .replace("{{BIN}}", &quote(shell, binary)),
    );
    InitScript::new(lines.join("\n"))
}

/// The deterministic output (SPEC §2.7): binary `tabsmith`, parent shell `/bin/zsh` (bash too), launching.
pub(crate) fn render_snapshot(shell: Shell, phase: IntegrationPhase, template: &str) -> InitScript {
    render(
        shell,
        phase,
        template,
        "tabsmith",
        || Some("/bin/zsh".to_string()),
        || true,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;

    /// The facts `render` asked for, in order (each one is I/O for `init`).
    fn facts_asked_while_rendering(shell: Shell, phase: IntegrationPhase) -> Vec<&'static str> {
        let asked = RefCell::new(Vec::new());
        render(
            shell,
            phase,
            "body\n",
            "tabsmith",
            || {
                asked.borrow_mut().push("parent shell");
                Some("/bin/zsh".to_string())
            },
            || {
                asked.borrow_mut().push("launch decision");
                true
            },
        );
        asked.into_inner()
    }

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

    #[test]
    fn should_open_the_pwsh_pre_script_with_its_own_launch_variable() {
        let template = "body\n";

        let script = render(
            Shell::Pwsh,
            IntegrationPhase::Pre,
            template,
            "tabsmith",
            || None,
            || false,
        );

        assert_eq!(script.text(), "$TABSMITH_SHOULD_LAUNCH = 1\nbody");
    }

    #[test]
    fn should_open_the_bash_pre_script_with_the_parent_shell_quoted_then_the_launch_flag() {
        let template = "body\n";

        let script = render(
            Shell::Bash,
            IntegrationPhase::Pre,
            template,
            "tabsmith",
            || Some("/opt/it's/bash".to_string()),
            || true,
        );

        assert_eq!(
            script.text(),
            "TABSMITH_SHELL='/opt/it'\\''s/bash'\nSHOULD_TABSMITH_LAUNCH=0\nbody"
        );
    }

    #[test]
    fn should_leave_the_parent_shell_out_of_the_zsh_pre_script_when_it_is_unknown() {
        let template = "body\n";

        let script = render(
            Shell::Zsh,
            IntegrationPhase::Pre,
            template,
            "tabsmith",
            || None,
            || false,
        );

        assert_eq!(script.text(), "SHOULD_TABSMITH_LAUNCH=1\nbody");
    }

    /// The snapshot names zsh as the parent even for bash (SPEC §2.7).
    #[test]
    fn should_render_a_snapshot_running_tabsmith_from_zsh_and_launching() {
        let template = "body {{BIN}}\n";

        let script = render_snapshot(Shell::Bash, IntegrationPhase::Pre, template);

        assert_eq!(
            script.text(),
            "TABSMITH_SHELL='/bin/zsh'\nSHOULD_TABSMITH_LAUNCH=0\nbody 'tabsmith'"
        );
    }

    #[test]
    fn should_ask_neither_the_parent_shell_nor_the_launch_decision_for_a_post_script() {
        let shells = [Shell::Bash, Shell::Zsh, Shell::Fish, Shell::Pwsh];

        let asked = shells.map(|shell| facts_asked_while_rendering(shell, IntegrationPhase::Post));

        assert_eq!(asked, [vec![], vec![], vec![], vec![]] as [Vec<&str>; 4]);
    }

    #[test]
    fn should_ask_only_the_launch_decision_for_the_fish_and_pwsh_pre_scripts() {
        let shells = [Shell::Fish, Shell::Pwsh];

        let asked = shells.map(|shell| facts_asked_while_rendering(shell, IntegrationPhase::Pre));

        assert_eq!(asked, [vec!["launch decision"], vec!["launch decision"]]);
    }

    #[test]
    fn should_ask_the_parent_shell_before_the_launch_decision_for_the_bash_and_zsh_pre_scripts() {
        let shells = [Shell::Bash, Shell::Zsh];

        let asked = shells.map(|shell| facts_asked_while_rendering(shell, IntegrationPhase::Pre));

        assert_eq!(
            asked,
            [
                vec!["parent shell", "launch decision"],
                vec!["parent shell", "launch decision"],
            ]
        );
    }

    /// `trim_end_matches('\n')`: a CR before the last LF and the blank lines inside stay.
    #[test]
    fn should_trim_only_the_line_feeds_at_the_end_of_the_template() {
        let template = "\nfirst\n\nlast\r\n\n";

        let script = render(
            Shell::Zsh,
            IntegrationPhase::Post,
            template,
            "tabsmith",
            || None,
            || true,
        );

        assert_eq!(script.text(), "\nfirst\n\nlast\r");
    }
}
