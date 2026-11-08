use crate::modules::shared::domain::value_objects::shell::Shell;
use crate::modules::shell_integration::domain::value_objects::integration_phase::IntegrationPhase;

/// The shell code `init` prints for a shell and phase, still holding `{{BIN}}` (bash and zsh share one pre script).
pub(crate) trait InitScriptTemplates {
    fn template(&self, shell: Shell, phase: IntegrationPhase) -> &'static str;
}

/// Answers every shell and phase with the one template it was given, for tests.
#[cfg(test)]
pub(crate) struct FixedInitScriptTemplates {
    template: &'static str,
}

#[cfg(test)]
impl FixedInitScriptTemplates {
    pub(crate) fn new(template: &'static str) -> Self {
        FixedInitScriptTemplates { template }
    }
}

#[cfg(test)]
impl InitScriptTemplates for FixedInitScriptTemplates {
    fn template(&self, _shell: Shell, _phase: IntegrationPhase) -> &'static str {
        self.template
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn should_answer_every_shell_and_phase_with_the_template_it_was_given() {
        let templates = FixedInitScriptTemplates::new("echo {{BIN}}");

        let answers = [
            templates.template(Shell::Bash, IntegrationPhase::Pre),
            templates.template(Shell::Pwsh, IntegrationPhase::Post),
        ];

        assert_eq!(answers, ["echo {{BIN}}", "echo {{BIN}}"]);
    }
}
