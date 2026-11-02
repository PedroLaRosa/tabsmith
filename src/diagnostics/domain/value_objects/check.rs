use crate::diagnostics::domain::value_objects::check_severity::CheckSeverity;
use crate::diagnostics::domain::value_objects::check_subject::CheckSubject;

/// One line of a doctor group: its name, how hard a failure counts, and what it evaluates.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Check {
    name: String,
    severity: CheckSeverity,
    subject: CheckSubject,
}

impl Check {
    pub(crate) fn new(name: String, severity: CheckSeverity, subject: CheckSubject) -> Self {
        Check { name, severity, subject }
    }

    pub(crate) fn name(&self) -> &str {
        &self.name
    }

    pub(crate) fn severity(&self) -> CheckSeverity {
        self.severity
    }

    pub(crate) fn subject(&self) -> &CheckSubject {
        &self.subject
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diagnostics::domain::value_objects::environment_check::EnvironmentCheck;
    use crate::diagnostics::domain::value_objects::system_facts::SystemFacts;
    use crate::diagnostics::domain::value_objects::terminal_environment::{TerminalEnvironment, TerminalEnvironmentParts};
    use crate::settings::domain::value_objects::settings::Settings;
    use std::rc::Rc;

    fn data_directory_check() -> Check {
        Check::new("Data directory".to_string(), CheckSeverity::Normal, CheckSubject::DataDirectory)
    }

    #[test]
    fn should_read_back_the_name_severity_and_subject_of_a_check() {
        let check = Check::new("Specs".to_string(), CheckSeverity::Soft, CheckSubject::SpecsExtracted);

        assert_eq!((check.name(), check.severity(), check.subject()), ("Specs", CheckSeverity::Soft, &CheckSubject::SpecsExtracted));
    }

    #[test]
    fn should_treat_checks_with_the_same_parts_as_equal() {
        assert_eq!(data_directory_check(), data_directory_check());
    }

    #[test]
    fn should_tell_checks_with_different_severities_apart() {
        let skipped = Check::new("Data directory".to_string(), CheckSeverity::Skip, CheckSubject::DataDirectory);

        assert_ne!(data_directory_check(), skipped);
    }

    #[test]
    fn should_let_environment_checks_share_one_set_of_facts() {
        let terminal = TerminalEnvironment::new(TerminalEnvironmentParts::default());
        let facts = Rc::new(SystemFacts::new(terminal, None, Settings::default()));
        let checks: Vec<Check> = [EnvironmentCheck::Session, EnvironmentCheck::Inline]
            .into_iter()
            .map(|which| Check::new("env".to_string(), CheckSeverity::Normal, CheckSubject::Environment(which, Rc::clone(&facts))))
            .collect();

        assert_eq!(Rc::strong_count(&facts), 3);
        drop(checks);
        assert_eq!(Rc::strong_count(&facts), 1);
    }
}
