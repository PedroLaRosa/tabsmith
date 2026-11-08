use crate::modules::diagnostics::domain::value_objects::diagnostic_value::DiagnosticValue;

/// A named group of report rows, kept in the order they were added.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct DiagnosticSection {
    name: String,
    rows: Vec<(String, DiagnosticValue)>,
}

impl DiagnosticSection {
    pub(crate) fn new(name: String, rows: Vec<(String, DiagnosticValue)>) -> Self {
        DiagnosticSection { name, rows }
    }

    pub(crate) fn name(&self) -> &str {
        &self.name
    }

    pub(crate) fn rows(&self) -> &[(String, DiagnosticValue)] {
        &self.rows
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn should_keep_the_rows_of_a_section_in_insertion_order() {
        let rows = vec![
            ("terminal".to_string(), DiagnosticValue::Text("kitty".to_string())),
            ("in-ci".to_string(), DiagnosticValue::Flag(false)),
            ("shell-version".to_string(), DiagnosticValue::Missing),
            ("cwd".to_string(), DiagnosticValue::Text("/tmp".to_string())),
        ];

        let section = DiagnosticSection::new("environment".to_string(), rows.clone());

        assert_eq!((section.name(), section.rows()), ("environment", &rows[..]));
    }
}
