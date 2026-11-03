use unicode_width::UnicodeWidthChar;

/// Control chars (newlines, tabs, ESC) would break the frame: show them as spaces.
pub(crate) fn clean(s: &str) -> String {
    s.chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .collect()
}

/// Display width of a char, as the model computes it (per char, `unicode-width`).
pub(crate) fn cw(c: char) -> usize {
    c.width().unwrap_or(0)
}

pub(crate) fn width(s: &str) -> usize {
    s.chars().map(cw).sum()
}

/// Fit into `w` columns; overflow replaced by `…`.
pub(crate) fn fit(s: &str, w: usize) -> String {
    if width(s) <= w {
        return s.to_string();
    }
    let (mut out, mut used) = (String::new(), 0);
    for c in s.chars() {
        if used + cw(c) > w.saturating_sub(1) {
            break;
        }
        out.push(c);
        used += cw(c);
    }
    if w > 0 {
        out.push('…');
    }
    out
}

/// Greedy word wrap by display width (long words hard-broken), at most `max` lines, `…` on overflow.
pub(crate) fn wrap(text: &str, w: usize, max: usize) -> Vec<String> {
    let mut lines = vec![String::new()];
    let mut lw = 0;
    for word in text.split_whitespace() {
        if lw > 0 && lw + 1 + width(word) > w {
            lines.push(String::new());
            lw = 0;
        }
        if lw > 0 {
            lines.last_mut().unwrap().push(' ');
            lw += 1;
        }
        for c in word.chars() {
            if lw + cw(c) > w {
                lines.push(String::new());
                lw = 0;
            }
            lines.last_mut().unwrap().push(c);
            lw += cw(c);
        }
    }
    if lines.len() > max {
        lines.truncate(max);
        if let Some(l) = lines.last_mut() {
            *l = fit(&format!("{l}…"), w);
        }
    }
    lines
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn truncation_and_wrap() {
        assert_eq!(fit("美国人美国人", 10), "美国人美…");
        assert_eq!(width(&fit("美国人美国人", 10)), 9);
        assert_eq!(fit("abc", 3), "abc");
        assert_eq!(fit("abcd", 3), "ab…");
        assert_eq!(fit("abcd", 0), "");
        assert_eq!(wrap("one two three", 7, 5), vec!["one two", "three"]);
        assert_eq!(wrap("abcdefghij", 4, 5), vec!["abcd", "efgh", "ij"]);
        assert_eq!(wrap("aa bb cc dd", 5, 1), vec!["aa b…"]);
        assert_eq!(wrap("aa bb cc dd", 6, 1), vec!["aa bb…"]);
        assert!(cw('…') == 1);
    }
}
