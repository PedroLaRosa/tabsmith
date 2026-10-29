use std::collections::HashMap;

/// When each suggestion of each command was last inserted: command → suggestion name → unix ms (SPEC §3.2
/// `state.json`). Ranking boosts the recent ones.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct SuggestionRecency {
    times: HashMap<String, HashMap<String, u64>>,
}

impl SuggestionRecency {
    /// The recency as plain values: what it is persisted as.
    pub(crate) fn to_primitives(&self) -> HashMap<String, HashMap<String, u64>> {
        self.times.clone()
    }

    /// Records that `name` was inserted for `command` at `at_ms`; `true` when the recency changed. The
    /// auto-execute row (`↪`) is never recorded.
    pub(crate) fn record(&mut self, command: String, name: String, at_ms: u64) -> bool {
        if name.contains('↪') {
            return false;
        }
        self.times.entry(command).or_default().insert(name, at_ms);
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn times(command: &str, name: &str, at_ms: u64) -> HashMap<String, HashMap<String, u64>> {
        [(command.to_string(), [(name.to_string(), at_ms)].into())].into()
    }

    #[test]
    fn should_record_the_insertion_time_under_the_command_and_name() {
        let mut recency = SuggestionRecency::default();

        let recorded = recency.record("git".into(), "checkout".into(), 1_700_000_000_000);

        assert_eq!(
            (recorded, recency.to_primitives()),
            (true, times("git", "checkout", 1_700_000_000_000))
        );
    }

    #[test]
    fn should_not_record_an_auto_execute_row() {
        let mut recency = SuggestionRecency::default();

        let recorded = recency.record("git".into(), "↪".into(), 1_700_000_000_000);

        assert_eq!((recorded, recency), (false, SuggestionRecency::default()));
    }
}
