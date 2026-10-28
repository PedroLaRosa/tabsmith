/// The text of an rc file that gets the pre and post blocks.
#[derive(Debug)]
pub(crate) struct Dotfile {
    text: String,
}

impl Dotfile {
    pub(crate) fn new(text: String) -> Self {
        Dotfile { text }
    }

    /// A `# tabsmith ignore` line anywhere after the BOM: the user opted this file out.
    pub(crate) fn is_opted_out(&self) -> bool {
        split_bom(&self.text).1.lines().any(is_ignore_line)
    }
}

/// `# tabsmith ignore` (case-insensitive): the user opted this file out.
fn is_ignore_line(line: &str) -> bool {
    line.trim_start().strip_prefix('#').is_some_and(|r| {
        r.trim_start()
            .to_ascii_lowercase()
            .starts_with("tabsmith ignore")
    })
}

/// (UTF-8 BOM or "", the text after it): editors save pwsh profiles with a BOM, which must stay the first bytes.
fn split_bom(s: &str) -> (&str, &str) {
    s.strip_prefix('\u{feff}')
        .map_or(("", s), |b| ("\u{feff}", b))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Any case and spacing, also right after a BOM.
    #[test]
    fn should_be_opted_out_by_a_tabsmith_ignore_comment() {
        let dotfile = Dotfile::new("\u{feff}  #  TABSMITH ignore\necho hi\n".to_string());

        let opted_out = dotfile.is_opted_out();

        assert!(opted_out);
    }
}
