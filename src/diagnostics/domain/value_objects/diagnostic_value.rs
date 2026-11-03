/// One value of a diagnostic report row.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum DiagnosticValue {
    Text(String),
    Flag(bool),
    Missing,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn should_tell_the_three_kinds_of_value_apart() {
        let values = [
            DiagnosticValue::Text("zsh".to_string()),
            DiagnosticValue::Flag(true),
            DiagnosticValue::Missing,
        ];

        assert_ne!(values[0], values[1]);
        assert_ne!(values[1], values[2]);
        assert_ne!(DiagnosticValue::Flag(true), DiagnosticValue::Flag(false));
        assert_eq!(values[0].clone(), DiagnosticValue::Text("zsh".to_string()));
    }
}
