use crate::shared::domain::value_objects::shell::Shell;
use crate::shell_integration::application::ports::init_script_templates::InitScriptTemplates;
use crate::shell_integration::application::ports::shell_environment::ShellEnvironment;
use crate::shell_integration::domain::services::init_script_renderer;
use crate::shell_integration::domain::services::launch_policy::decide;
use crate::shell_integration::domain::shell_integration_error::ShellIntegrationError;
use crate::shell_integration::domain::value_objects::init_script::InitScript;
use crate::shell_integration::domain::value_objects::integration_phase::IntegrationPhase;
use std::rc::Rc;

/// `tabsmith init <shell> <phase>`: the integration code a shell sources (SPEC §5.3).
pub(crate) struct RenderInitScriptUseCase {
    environment: Rc<dyn ShellEnvironment>,
    templates: Rc<dyn InitScriptTemplates>,
}

impl RenderInitScriptUseCase {
    pub(crate) fn new(
        environment: Rc<dyn ShellEnvironment>,
        templates: Rc<dyn InitScriptTemplates>,
    ) -> Self {
        RenderInitScriptUseCase {
            environment,
            templates,
        }
    }

    pub(crate) fn execute(
        &self,
        shell: Shell,
        phase: IntegrationPhase,
        snapshot: bool,
    ) -> Result<InitScript, ShellIntegrationError> {
        let template = self.templates.template(shell, phase);
        if snapshot {
            return Ok(init_script_renderer::render_snapshot(shell, phase, template));
        }
        Ok(init_script_renderer::render(
            shell,
            phase,
            template,
            &self.environment.current_executable()?,
            || self.environment.parent_shell(),
            || decide(&self.environment.launch_facts()).launches(),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shell_integration::application::ports::init_script_templates::FixedInitScriptTemplates;
    use crate::shell_integration::application::ports::shell_environment::RecordingShellEnvironment;

    /// The use case over `environment` and one template for every shell and phase.
    fn use_case(environment: &Rc<RecordingShellEnvironment>) -> RenderInitScriptUseCase {
        RenderInitScriptUseCase::new(
            environment.clone(),
            Rc::new(FixedInitScriptTemplates::new("body {{BIN}}\n")),
        )
    }

    #[test]
    fn should_render_the_snapshot_without_asking_the_shell_environment() {
        let environment = Rc::new(RecordingShellEnvironment::default());

        let script = use_case(&environment)
            .execute(Shell::Bash, IntegrationPhase::Pre, true)
            .unwrap();

        assert_eq!(
            (script.text(), environment.asked()),
            (
                "TABSMITH_SHELL='/bin/zsh'\nSHOULD_TABSMITH_LAUNCH=0\nbody 'tabsmith'",
                vec![]
            )
        );
    }

    #[test]
    fn should_render_the_post_script_running_the_current_executable() {
        let environment = Rc::new(RecordingShellEnvironment::default());

        let script = use_case(&environment)
            .execute(Shell::Zsh, IntegrationPhase::Post, false)
            .unwrap();

        assert_eq!(script.text(), "body '/usr/local/bin/tabsmith'");
    }

    /// No variable is set and stdin is no terminal, so tabsmith does not launch: SHOULD_TABSMITH_LAUNCH is 1.
    #[test]
    fn should_open_the_pre_script_with_the_launch_flag_the_launch_facts_decide() {
        let environment = Rc::new(RecordingShellEnvironment::default());

        let script = use_case(&environment)
            .execute(Shell::Fish, IntegrationPhase::Pre, false)
            .unwrap();

        assert_eq!(
            script.text(),
            "set -g SHOULD_TABSMITH_LAUNCH 1\nbody '/usr/local/bin/tabsmith'"
        );
    }

    #[test]
    fn should_open_the_zsh_pre_script_with_the_parent_shell_the_environment_gives() {
        let environment = Rc::new(
            RecordingShellEnvironment::default().with_parent_shell(Some("/usr/bin/zsh".into())),
        );

        let script = use_case(&environment)
            .execute(Shell::Zsh, IntegrationPhase::Pre, false)
            .unwrap();

        assert_eq!(
            script.text(),
            "TABSMITH_SHELL='/usr/bin/zsh'\nSHOULD_TABSMITH_LAUNCH=1\nbody '/usr/local/bin/tabsmith'"
        );
    }
}
