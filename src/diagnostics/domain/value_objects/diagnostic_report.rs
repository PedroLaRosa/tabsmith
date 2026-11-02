use crate::diagnostics::domain::value_objects::diagnostic_section::DiagnosticSection;

/// The facts about the installation that `debug diagnostic` prints, section by section.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct DiagnosticReport {
    sections: Vec<DiagnosticSection>,
}

impl DiagnosticReport {
    pub(crate) fn new(sections: Vec<DiagnosticSection>) -> Self {
        DiagnosticReport { sections }
    }

    pub(crate) fn sections(&self) -> &[DiagnosticSection] {
        &self.sections
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diagnostics::domain::value_objects::diagnostic_value::DiagnosticValue;

    fn section(name: &str) -> DiagnosticSection {
        let row = ("version".to_string(), DiagnosticValue::Text("1".to_string()));
        DiagnosticSection::new(name.to_string(), vec![row])
    }

    #[test]
    fn should_keep_the_sections_of_a_report_in_insertion_order() {
        let sections = vec![section("tabsmith"), section("environment"), section("env-vars")];

        let report = DiagnosticReport::new(sections.clone());

        assert_eq!(report.sections(), &sections[..]);
    }
}
