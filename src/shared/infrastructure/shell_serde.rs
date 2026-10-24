//! serde support for the domain `Shell`, which stays std-only.
use crate::shared::domain::value_objects::shell::Shell;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// The derive `Shell` carried before it moved to the domain, applied remotely: same names, order and errors.
#[derive(Serialize, Deserialize)]
#[serde(remote = "Shell", rename = "Shell", rename_all = "lowercase")]
enum ShellDef {
    Bash,
    Zsh,
    Fish,
    Pwsh,
}

impl Serialize for Shell {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        ShellDef::serialize(self, serializer)
    }
}

impl<'de> Deserialize<'de> for Shell {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        ShellDef::deserialize(deserializer)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn should_serialize_every_shell_as_its_lowercase_name() {
        let shells = Shell::ALL;

        let json: Vec<String> = shells.iter().map(|s| serde_json::to_string(s).unwrap()).collect();

        assert_eq!(json, [r#""bash""#, r#""zsh""#, r#""fish""#, r#""pwsh""#]);
    }

    #[test]
    fn should_deserialize_every_lowercase_shell_name() {
        let json = [r#""bash""#, r#""zsh""#, r#""fish""#, r#""pwsh""#];

        let shells: Vec<Shell> = json.iter().map(|j| serde_json::from_str(j).unwrap()).collect();

        assert_eq!(shells, Shell::ALL);
    }

    #[test]
    fn should_reject_any_other_name_with_the_unknown_variant_error() {
        let json = r#""Bash""#;

        let error = serde_json::from_str::<Shell>(json).unwrap_err();

        assert_eq!(
            error.to_string(),
            "unknown variant `Bash`, expected one of `bash`, `zsh`, `fish`, `pwsh` at line 1 column 6"
        );
    }

    #[test]
    fn should_reject_a_value_that_is_not_a_name() {
        let json = "1";

        let error = serde_json::from_str::<Shell>(json).unwrap_err();

        assert_eq!(error.to_string(), "expected value at line 1 column 1");
    }
}
