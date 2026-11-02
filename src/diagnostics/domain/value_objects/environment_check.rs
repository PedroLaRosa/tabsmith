/// A check of the environment group.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum EnvironmentCheck {
    TerminalIntegrations,
    Session,
    AutocompleteEnabled,
    DeveloperMode,
    Inline,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn should_tell_the_environment_checks_apart() {
        let checks = [
            EnvironmentCheck::TerminalIntegrations,
            EnvironmentCheck::Session,
            EnvironmentCheck::AutocompleteEnabled,
            EnvironmentCheck::DeveloperMode,
            EnvironmentCheck::Inline,
        ];

        let distinct = checks.iter().enumerate().all(|(i, a)| checks.iter().skip(i + 1).all(|b| a != b));

        assert!(distinct);
    }

    #[test]
    fn should_copy_an_environment_check() {
        let check = EnvironmentCheck::Inline;

        let copy = check;

        assert_eq!((check, copy), (EnvironmentCheck::Inline, EnvironmentCheck::Inline));
    }
}
