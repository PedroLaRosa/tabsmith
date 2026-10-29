use crate::specs::domain::value_objects::exec_request::ExecRequest;
use serde::Deserialize;
use std::collections::HashMap;

/// `executeCommand` input `{command, args, cwd?, env?, timeout?}` as spec code sends it.
///
/// The identifier MUST stay `ExecInput`: serde_derive writes "expected struct ExecInput" and
/// "struct ExecInput with 5 elements" from it, and spec code reaches those texts through `__ts_exec`.
#[derive(Deserialize)]
pub(crate) struct ExecInput {
    command: String,
    #[serde(default)]
    args: Vec<String>,
    #[serde(default)]
    cwd: Option<String>,
    /// `null` value → remove the variable.
    #[serde(default)]
    env: HashMap<String, Option<String>>,
    #[serde(default)]
    timeout: Option<u64>,
}

impl From<ExecInput> for ExecRequest {
    fn from(input: ExecInput) -> Self {
        ExecRequest::new(input.command, input.args, input.cwd, input.env, input.timeout)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn should_read_an_exec_request_with_its_optional_fields_defaulted() {
        let json = r#"{"command":"ls"}"#;

        let request = ExecRequest::from(serde_json::from_str::<ExecInput>(json).unwrap());

        assert_eq!(
            request,
            ExecRequest::new("ls".into(), vec![], None, HashMap::new(), None)
        );
    }

    #[test]
    fn should_read_a_null_env_value_as_a_variable_to_remove() {
        let json = r#"{"command":"x","args":["a"],"cwd":"/w","env":{"A":null,"B":"1"},"timeout":7,"extra":0}"#;

        let request = ExecRequest::from(serde_json::from_str::<ExecInput>(json).unwrap());

        assert_eq!(
            request,
            ExecRequest::new(
                "x".into(),
                vec!["a".into()],
                Some("/w".into()),
                HashMap::from([("A".to_string(), None), ("B".to_string(), Some("1".to_string()))]),
                Some(7),
            )
        );
    }

    #[test]
    fn should_name_the_exec_input_record_in_its_parse_errors() {
        let read = |json: &str| serde_json::from_str::<ExecInput>(json).err().unwrap().to_string();

        let errors = [read("[]"), read("1"), read("{}")];

        assert_eq!(
            errors,
            [
                "invalid length 0, expected struct ExecInput with 5 elements at line 1 column 2",
                "invalid type: integer `1`, expected struct ExecInput at line 1 column 1",
                "missing field `command` at line 1 column 2",
            ]
        );
    }
}
