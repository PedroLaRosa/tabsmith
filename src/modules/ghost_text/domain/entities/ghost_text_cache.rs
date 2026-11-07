/// LLM completion cache (SPEC §8.4), cleared on every new command.
#[derive(Default)]
pub(crate) struct GhostTextCache {
    entries: Vec<(String, f64)>,
}

impl GhostTextCache {
    pub(crate) fn clear(&mut self) {
        self.entries.clear();
    }

    /// Store full texts (`buffer + completion`); the first gets priority 0.0, others 1.0.
    pub(crate) fn insert(&mut self, full_texts: &[String]) {
        for (i, t) in full_texts.iter().enumerate() {
            self.entries
                .push((t.clone(), if i == 0 { 0.0 } else { 1.0 }));
        }
        // ponytail: linear scan instead of a radix trie, so keep it small (oldest dropped).
        self.entries.drain(..self.entries.len().saturating_sub(50));
    }

    /// Lowest-priority cached text extending `buffer` (first inserted wins ties) → the remaining suffix.
    pub(crate) fn lookup(&self, buffer: &str) -> Option<String> {
        self.entries
            .iter()
            .filter(|(full, _)| full.len() > buffer.len() && full.starts_with(buffer))
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .map(|(full, _)| full[buffer.len()..].to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cache_lookup_vector() {
        let mut c = GhostTextCache::default();
        for (t, p) in [
            ("echo hello, world", 0.5),
            ("echo hello there", 1.0),
            ("ls", 0.5),
            ("ls -l", 1.0),
            ("ls -al", 1.0),
            ("cat -n", 0.5),
            ("cat", 1.0),
        ] {
            c.entries.push((t.into(), p));
        }
        assert_eq!(c.lookup("e").as_deref(), Some("cho hello, world"));
        assert_eq!(c.lookup("echo hello ").as_deref(), Some("there"));
        assert_eq!(c.lookup("ls -a").as_deref(), Some("l"));
        assert_eq!(c.lookup("cat -").as_deref(), Some("n"));
        assert_eq!(c.lookup("other"), None);
        assert_eq!(
            c.lookup("ls -al"),
            None,
            "an exact match is not a completion"
        );
        c.clear();
        assert_eq!(c.lookup("e"), None);
    }

    #[test]
    fn cache_insert_priorities() {
        let mut c = GhostTextCache::default();
        c.insert(&["git status".into(), "git stash".into()]);
        assert_eq!(c.lookup("git sta").as_deref(), Some("tus"));
        assert_eq!(c.lookup("git stas").as_deref(), Some("h"));
        c.insert(&(0..60).map(|i| format!("x{i}")).collect::<Vec<_>>());
        assert_eq!(c.entries.len(), 50);
    }
}
