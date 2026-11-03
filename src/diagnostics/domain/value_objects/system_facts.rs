use crate::diagnostics::domain::value_objects::terminal_environment::TerminalEnvironment;
use crate::settings::domain::value_objects::settings::Settings;

/// The immutable snapshot the environment group's checks read, taken when that group is built.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct SystemFacts {
    terminal: TerminalEnvironment,
    session_id: Option<String>,
    settings: Settings,
}

impl SystemFacts {
    pub(crate) fn new(terminal: TerminalEnvironment, session_id: Option<String>, settings: Settings) -> Self {
        SystemFacts { terminal, session_id, settings }
    }

    pub(crate) fn terminal(&self) -> &TerminalEnvironment {
        &self.terminal
    }

    pub(crate) fn session_id(&self) -> Option<&str> {
        self.session_id.as_deref()
    }

    pub(crate) fn settings(&self) -> &Settings {
        &self.settings
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diagnostics::domain::value_objects::terminal_environment::TerminalEnvironmentParts;
    use crate::settings::domain::value_objects::setting_value::SettingValue;

    fn terminal() -> TerminalEnvironment {
        TerminalEnvironment::new(TerminalEnvironmentParts {
            term_program: Some("Apple_Terminal".to_string()),
            ..TerminalEnvironmentParts::default()
        })
    }

    #[test]
    fn should_read_back_the_terminal_the_session_and_the_settings_of_the_facts() {
        let settings = Settings::default().with("autocomplete.enabled", SettingValue::Bool(false));
        let facts = SystemFacts::new(terminal(), Some("s-1".to_string()), settings.clone());

        assert_eq!((facts.terminal(), facts.session_id(), facts.settings()), (&terminal(), Some("s-1"), &settings));
    }

    #[test]
    fn should_report_no_session_id_when_the_facts_have_none() {
        let facts = SystemFacts::new(terminal(), None, Settings::default());

        assert_eq!(facts.session_id(), None);
    }

    #[test]
    fn should_compare_system_facts_by_terminal_session_and_settings() {
        let facts = SystemFacts::new(terminal(), Some("s-1".to_string()), Settings::default());
        let same = SystemFacts::new(terminal(), Some("s-1".to_string()), Settings::default());
        let other_session = SystemFacts::new(terminal(), Some("s-2".to_string()), Settings::default());

        assert_eq!((facts == same, facts == other_session), (true, false));
    }
}
