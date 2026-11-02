use crate::diagnostics::domain::value_objects::check::Check;

/// A titled run of checks that doctor prints and evaluates together.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct CheckGroup {
    header: String,
    checks: Vec<Check>,
}

impl CheckGroup {
    pub(crate) fn new(header: String, checks: Vec<Check>) -> Self {
        CheckGroup { header, checks }
    }

    pub(crate) fn header(&self) -> &str {
        &self.header
    }

    pub(crate) fn checks(&self) -> &[Check] {
        &self.checks
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diagnostics::domain::value_objects::check_severity::CheckSeverity;
    use crate::diagnostics::domain::value_objects::check_subject::CheckSubject;

    fn check(name: &str, subject: CheckSubject) -> Check {
        Check::new(name.to_string(), CheckSeverity::Normal, subject)
    }

    #[test]
    fn should_read_back_the_header_and_the_checks_of_a_group_in_order() {
        let first = check("Data directory", CheckSubject::DataDirectory);
        let second = check("Specs", CheckSubject::SpecsExtracted);
        let group = CheckGroup::new("Setup".to_string(), vec![first.clone(), second.clone()]);

        assert_eq!((group.header(), group.checks()), ("Setup", &[first, second][..]));
    }

    #[test]
    fn should_hold_a_group_without_checks() {
        let group = CheckGroup::new("Empty".to_string(), Vec::new());

        assert!(group.checks().is_empty());
    }

    #[test]
    fn should_compare_groups_by_header() {
        let checks = vec![check("Specs", CheckSubject::SpecsExtracted)];
        let group = CheckGroup::new("Setup".to_string(), checks.clone());

        assert_eq!((group == CheckGroup::new("Setup".to_string(), checks.clone()), group == CheckGroup::new("Other".to_string(), checks)), (true, false));
    }
}
