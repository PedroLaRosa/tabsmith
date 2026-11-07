use crate::modules::specs::domain::value_objects::filter_strategy::FilterStrategy;
use std::collections::HashMap;

/// SPEC §6.1.5 mid-word guard: the cursor (already clamped to the buffer) sits before a character of a word, or
/// inside a character. The `||` keeps the slice off a non-boundary.
pub(crate) fn is_mid_word(buffer: &str, cursor: usize) -> bool {
    !buffer.is_char_boundary(cursor)
        || buffer[cursor..]
            .chars()
            .next()
            .is_some_and(|c| !c.is_whitespace())
}

/// The argument's filter strategy decides the fuzzy search; the default one follows the user's fuzzy search.
pub(crate) fn uses_fuzzy_search(strategy: FilterStrategy, user_fuzzy: bool) -> bool {
    match strategy {
        FilterStrategy::Prefix => false,
        FilterStrategy::Fuzzy => true,
        FilterStrategy::Default => user_fuzzy,
    }
}

/// The text before the last token, as typed: what a history row must start with.
pub(crate) fn history_prefix(before: &str, raw_last_token: &str) -> String {
    before
        .strip_suffix(raw_last_token)
        .unwrap_or(before)
        .to_string()
}

/// The value of `key` in `env`; on Windows a key differing only in case matches too.
pub(crate) fn env_var(env: &HashMap<String, String>, key: &str) -> Option<String> {
    env.get(key)
        .or_else(|| {
            if cfg!(windows) {
                env.iter()
                    .find(|(k, _)| k.eq_ignore_ascii_case(key))
                    .map(|(_, v)| v)
            } else {
                None
            }
        })
        .cloned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn should_guard_a_cursor_inside_a_word() {
        let buffer = "git cömmit";

        let guarded = (is_mid_word(buffer, 4), is_mid_word(buffer, 6));

        assert_eq!(guarded, (true, true));
    }

    #[test]
    fn should_never_guard_a_cursor_at_the_end_of_the_buffer() {
        let buffer = "git co";

        let guarded = (is_mid_word(buffer, buffer.len()), is_mid_word("", 0));

        assert_eq!(guarded, (false, false));
    }
}
