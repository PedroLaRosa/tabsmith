use std::collections::HashMap;

/// What the launch rules (SPEC §5.2) look at: the environment (non-UTF-8 names dropped, values lossy) and whether stdin
/// is a tty (Unix) / a console (Windows).
#[derive(Debug)]
pub(crate) struct LaunchFacts {
    env: HashMap<String, String>,
    stdin_is_terminal: bool,
}

impl LaunchFacts {
    pub(crate) fn new(env: HashMap<String, String>, stdin_is_terminal: bool) -> Self {
        LaunchFacts {
            env,
            stdin_is_terminal,
        }
    }

    /// Set, whatever its value (empty included).
    pub(crate) fn has(&self, name: &str) -> bool {
        self.env.contains_key(name)
    }

    /// Set to exactly `value` (case-sensitive).
    pub(crate) fn is(&self, name: &str, value: &str) -> bool {
        self.env.get(name).is_some_and(|x| x == value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn facts(env: &[(&str, &str)], stdin_is_terminal: bool) -> LaunchFacts {
        LaunchFacts::new(
            env.iter()
                .map(|(k, v)| (k.to_string(), v.to_string()))
                .collect(),
            stdin_is_terminal,
        )
    }

    /// `CI=` (set but empty) still means "In CI".
    #[test]
    fn should_count_a_variable_as_set_even_when_it_is_empty() {
        let facts = facts(&[("CI", "")], true);

        let set = [facts.has("CI"), facts.has("TMUX")];

        assert_eq!(set, [true, false]);
    }
    /// Warp is told apart by its exact `TERM_PROGRAM`, case included.
    #[test]
    fn should_match_a_variable_only_on_its_exact_value() {
        let facts = facts(&[("TERM_PROGRAM", "WarpTerminal")], true);

        let matches = [
            facts.is("TERM_PROGRAM", "WarpTerminal"),
            facts.is("TERM_PROGRAM", "warpterminal"),
            facts.is("TERM_PROGRAM", "Warp"),
            facts.is("__CFBundleIdentifier", "WarpTerminal"),
        ];

        assert_eq!(matches, [true, false, false, false]);
    }
}
