/// The gray continuation computed for one buffer: the buffer it was computed for and its text.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct GhostText {
    computed_for: String,
    text: String,
}

impl GhostText {
    pub(crate) fn new(computed_for: String, text: String) -> Self {
        Self { computed_for, text }
    }

    /// What is left of ghost `g` (computed for buffer `base`) once `buffer` is typed. Never with control characters:
    /// accepting writes the text to the shell, where a CR (e.g. from an LLM reply) would run commands.
    pub(crate) fn rest_after(&self, buffer: &str) -> Option<String> {
        let rest = self.text.strip_prefix(buffer.strip_prefix(self.computed_for.as_str())?)?;
        (!rest.is_empty() && !rest.chars().any(char::is_control)).then(|| rest.to_string())
    }

    /// Leading whitespace + the next run of non-whitespace chars.
    pub(crate) fn next_word(g: &str) -> &str {
        let start = g.len() - g.trim_start().len();
        let end = g[start..]
            .find(char::is_whitespace)
            .map_or(g.len(), |i| start + i);
        &g[..end]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ghost_keys_and_words() {
        assert_eq!(GhostText::next_word(" -la /tmp"), " -la");
        assert_eq!(GhostText::next_word("ho hi"), "ho");
        assert_eq!(GhostText::next_word("x"), "x");
    }

    #[test]
    fn ghost_with_control_chars_is_never_shown_or_accepted() {
        // An LLM reply with bare CRs would run `touch /x` when accepted.
        assert_eq!(
            GhostText::new("git sta".into(), "tus\rtouch /x\recho".into()).rest_after("git sta"),
            None
        );
        assert_eq!(GhostText::new("git st".into(), "atus\x1b[2J".into()).rest_after("git sta"), None);
        assert_eq!(GhostText::new("git st".into(), "atus\u{9b}31m".into()).rest_after("git sta"), None);
        assert_eq!(GhostText::new("git st".into(), "atus\x7f".into()).rest_after("git sta"), None);
        assert_eq!(GhostText::new("echo".into(), " a\nb".into()).rest_after("echo"), None);
        assert_eq!(
            GhostText::new("git st".into(), "atus -s".into()).rest_after("git sta"),
            Some("tus -s".into())
        );
        assert_eq!(GhostText::new("git".into(), "x".into()).rest_after("ls"), None);
    }
}
