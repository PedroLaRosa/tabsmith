/// Cleaning (SPEC §8.5): first non-empty non-fence line, surrounding backticks/quotes, history number, echoed
/// buffer, trailing whitespace.
pub(crate) fn clean(text: &str, buffer: &str) -> Option<String> {
    let line = text.lines().find(|l| {
        let t = l.trim();
        !t.is_empty() && !t.starts_with("```")
    })?;
    let t = line.trim();
    let mut s = ['`', '"', '\'']
        .into_iter()
        .find_map(|q| {
            t.strip_prefix(q)?
                .strip_suffix(q)
                .filter(|inner| !inner.contains(q))
        })
        .unwrap_or(line);
    // Number first so a fully echoed `    3  git status` loses both its number and the buffer.
    let digits = s.trim_start();
    let rest = digits.trim_start_matches(|c: char| c.is_ascii_digit());
    if rest.len() < digits.len() {
        s = rest.strip_prefix("  ").unwrap_or(s);
    }
    if !buffer.is_empty() {
        s = s.strip_prefix(buffer).unwrap_or(s);
    }
    let s = s.trim_end();
    // Ghost text is typed into the shell as-is: an embedded CR/ESC/C1 byte would run or rewrite commands.
    (!s.is_empty() && !s.chars().any(char::is_control)).then(|| s.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cleaning() {
        assert_eq!(clean("tus\nmore", "git sta").as_deref(), Some("tus"));
        assert_eq!(
            clean("```bash\n status\n```", "git").as_deref(),
            Some(" status")
        );
        assert_eq!(clean("`status`", "git ").as_deref(), Some("status"));
        assert_eq!(clean("\"status\"", "git ").as_deref(), Some("status"));
        assert_eq!(
            clean("\"a\" \"b\"", "echo ").as_deref(),
            Some("\"a\" \"b\"")
        );
        assert_eq!(clean("git status", "git st").as_deref(), Some("atus"));
        assert_eq!(
            clean("    3  git status", "git st").as_deref(),
            Some("atus")
        );
        assert_eq!(clean("50  ls -la", "ls").as_deref(), Some(" -la"));
        assert_eq!(clean(" -la   ", "ls").as_deref(), Some(" -la"));
        assert_eq!(clean("\n  \n", "ls"), None);
        assert_eq!(clean("git st", "git st"), None);
        assert_eq!(clean("", "ls"), None);
        // Control characters (C0, DEL, C1) are never offered as ghost text.
        assert_eq!(clean("tus\rtouch x", "git sta"), None);
        assert_eq!(clean(" status\rtouch PWNED #", "git"), None);
        assert_eq!(clean(" -la\x1b[2J", "ls"), None);
        assert_eq!(clean(" -l\ta", "ls"), None);
        assert_eq!(clean(" -l\u{9b}2J", "ls"), None);
        assert_eq!(clean(" -l\x7f", "ls"), None);
    }
}
