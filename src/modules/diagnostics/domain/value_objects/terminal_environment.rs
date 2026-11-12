/// The environment facts the terminal rule reads, taken once when the environment group is built.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct TerminalEnvironment {
    term_program: Option<String>,
    bundle_identifier: Option<String>,
    tabsmith_term: Option<String>,
    inside_emacs: bool,
    launched_by_tabsmith: bool,
    in_ci: bool,
    parent_is_cmd: bool,
    tabsmith_version: String,
}

/// The facts of a `TerminalEnvironment`. `inside_emacs`, `launched_by_tabsmith` and `in_ci` are presence facts:
/// true when the variable is set, even to an empty value.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct TerminalEnvironmentParts {
    pub(crate) term_program: Option<String>,
    /// The `__CFBundleIdentifier` variable.
    pub(crate) bundle_identifier: Option<String>,
    pub(crate) tabsmith_term: Option<String>,
    pub(crate) inside_emacs: bool,
    pub(crate) launched_by_tabsmith: bool,
    pub(crate) in_ci: bool,
    pub(crate) parent_is_cmd: bool,
    /// The version the running binary reports, supplied by the probe so tests can fix it.
    pub(crate) tabsmith_version: String,
}

impl TerminalEnvironment {
    pub(crate) fn new(parts: TerminalEnvironmentParts) -> Self {
        TerminalEnvironment {
            term_program: parts.term_program,
            bundle_identifier: parts.bundle_identifier,
            tabsmith_term: parts.tabsmith_term,
            inside_emacs: parts.inside_emacs,
            launched_by_tabsmith: parts.launched_by_tabsmith,
            in_ci: parts.in_ci,
            parent_is_cmd: parts.parent_is_cmd,
            tabsmith_version: parts.tabsmith_version,
        }
    }

    pub(crate) fn term_program(&self) -> Option<&str> {
        self.term_program.as_deref()
    }

    pub(crate) fn bundle_identifier(&self) -> Option<&str> {
        self.bundle_identifier.as_deref()
    }

    pub(crate) fn tabsmith_term(&self) -> Option<&str> {
        self.tabsmith_term.as_deref()
    }

    pub(crate) fn inside_emacs(&self) -> bool {
        self.inside_emacs
    }

    pub(crate) fn launched_by_tabsmith(&self) -> bool {
        self.launched_by_tabsmith
    }

    pub(crate) fn in_ci(&self) -> bool {
        self.in_ci
    }

    pub(crate) fn parent_is_cmd(&self) -> bool {
        self.parent_is_cmd
    }

    pub(crate) fn tabsmith_version(&self) -> &str {
        &self.tabsmith_version
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn every_fact_set() -> TerminalEnvironmentParts {
        TerminalEnvironmentParts {
            term_program: Some("iTerm.app".to_string()),
            bundle_identifier: Some("com.googlecode.iterm2".to_string()),
            tabsmith_term: Some("xterm-256color".to_string()),
            inside_emacs: true,
            launched_by_tabsmith: true,
            in_ci: true,
            parent_is_cmd: true,
            tabsmith_version: "1.2.3".to_string(),
        }
    }

    #[test]
    fn should_read_back_the_text_facts_of_the_terminal_environment() {
        let environment = TerminalEnvironment::new(every_fact_set());

        assert_eq!(
            (
                environment.term_program(),
                environment.bundle_identifier(),
                environment.tabsmith_term(),
                environment.tabsmith_version()
            ),
            (Some("iTerm.app"), Some("com.googlecode.iterm2"), Some("xterm-256color"), "1.2.3")
        );
    }

    #[test]
    fn should_read_back_the_flag_facts_of_the_terminal_environment() {
        let environment = TerminalEnvironment::new(every_fact_set());

        assert_eq!(
            (
                environment.inside_emacs(),
                environment.launched_by_tabsmith(),
                environment.in_ci(),
                environment.parent_is_cmd()
            ),
            (true, true, true, true)
        );
    }

    #[test]
    fn should_report_absent_text_facts_as_none() {
        let environment = TerminalEnvironment::new(TerminalEnvironmentParts::default());

        assert_eq!(
            (environment.term_program(), environment.bundle_identifier(), environment.tabsmith_term()),
            (None, None, None)
        );
    }

    #[test]
    fn should_keep_an_empty_text_fact_apart_from_an_absent_one() {
        let environment = TerminalEnvironment::new(TerminalEnvironmentParts {
            term_program: Some(String::new()),
            ..TerminalEnvironmentParts::default()
        });

        assert_eq!(environment.term_program(), Some(""));
    }

    #[test]
    fn should_tell_terminal_environments_apart_by_in_ci() {
        let environment = TerminalEnvironment::new(every_fact_set());
        let other = TerminalEnvironment::new(TerminalEnvironmentParts { in_ci: false, ..every_fact_set() });

        assert_ne!(environment, other);
    }
}
