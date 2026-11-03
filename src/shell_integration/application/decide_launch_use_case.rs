use crate::shell_integration::application::ports::shell_environment::ShellEnvironment;
use crate::shell_integration::domain::services::launch_policy::decide;
use crate::shell_integration::domain::value_objects::launch_decision::LaunchDecision;
use std::rc::Rc;

/// `tabsmith _ should-launch`: whether tabsmith launches in this shell (SPEC §5.2), and why.
pub(crate) struct DecideLaunchUseCase {
    environment: Rc<dyn ShellEnvironment>,
}

impl DecideLaunchUseCase {
    pub(crate) fn new(environment: Rc<dyn ShellEnvironment>) -> Self {
        DecideLaunchUseCase { environment }
    }

    pub(crate) fn execute(&self) -> LaunchDecision {
        decide(&self.environment.launch_facts())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shell_integration::application::ports::shell_environment::RecordingShellEnvironment;
    use std::collections::HashMap;

    #[test]
    fn should_decide_on_the_launch_facts_the_shell_environment_gives() {
        let environment = RecordingShellEnvironment::default()
            .with_launch_facts(HashMap::from([("CI".into(), "true".into())]), true);
        let use_case = DecideLaunchUseCase::new(Rc::new(environment));

        let decision = use_case.execute();

        assert_eq!(
            (decision.launches(), decision.reason()),
            (false, "❌ In CI")
        );
    }
}
