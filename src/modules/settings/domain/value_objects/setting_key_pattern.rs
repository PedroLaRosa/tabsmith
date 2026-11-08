/// Wildcard pattern for `settings --delete PATTERN`: `*` any run, `?` one char (backtracks to the last `*`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct SettingKeyPattern {
    pattern: String,
}

impl SettingKeyPattern {
    pub(crate) fn new(pattern: &str) -> Self {
        SettingKeyPattern {
            pattern: pattern.to_string(),
        }
    }

    pub(crate) fn matches(&self, key: &str) -> bool {
        let (p, s): (Vec<char>, Vec<char>) =
            (self.pattern.chars().collect(), key.chars().collect());
        let (mut pi, mut si, mut star, mut mark) = (0, 0, None, 0);
        while si < s.len() {
            if pi < p.len() && (p[pi] == '?' || p[pi] == s[si]) {
                pi += 1;
                si += 1;
            } else if pi < p.len() && p[pi] == '*' {
                star = Some(pi);
                mark = si;
                pi += 1;
            } else if let Some(st) = star {
                pi = st + 1;
                mark += 1;
                si = mark;
            } else {
                return false;
            }
        }
        p[pi..].iter().all(|&c| c == '*')
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wildcard() {
        assert!(SettingKeyPattern::new("autocomplete.theme").matches("autocomplete.theme"));
        assert!(!SettingKeyPattern::new("autocomplete.theme").matches("autocomplete.themes"));
        assert!(
            SettingKeyPattern::new("autocomplete.*").matches("autocomplete.keybindings.control+r")
        );
        assert!(!SettingKeyPattern::new("autocomplete.*").matches("inline.enabled"));
        assert!(SettingKeyPattern::new("*").matches(""));
        assert!(SettingKeyPattern::new("inline.?nabled").matches("inline.enabled"));
        assert!(!SettingKeyPattern::new("inline.?").matches("inline."));
        assert!(SettingKeyPattern::new("*.width").matches("autocomplete.width"));
        assert!(SettingKeyPattern::new("a*b*c").matches("a-x-b-y-b-c"));
        assert!(!SettingKeyPattern::new("a*b*c").matches("a-x-b-y-c-d"));
    }
}
