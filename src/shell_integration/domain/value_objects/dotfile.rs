pub(crate) const PRE_MARKER: &str = "# tabsmith pre block. Keep at the top of this file.";
pub(crate) const POST_MARKER: &str = "# tabsmith post block. Keep at the bottom of this file.";

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

    pub(crate) fn text(&self) -> &str {
        &self.text
    }

    /// The lines after the BOM, trimmed, without blank lines and `#` comments (the markers are such comments).
    pub(crate) fn sourced_lines(&self) -> Vec<&str> {
        split_bom(&self.text)
            .1
            .lines()
            .map(str::trim)
            .filter(|l| !l.is_empty() && !l.starts_with('#'))
            .collect()
    }

    /// `{bom}{shebang}{PRE_MARKER}\n{pre_source}\n{rest}`, then the trailing newlines replaced by
    /// `\n{POST_MARKER}\n{post_source}\n`.
    pub(crate) fn with_blocks(&self, pre_source: &str, post_source: &str) -> Dotfile {
        let (bom, body) = split_bom(&self.text);
        let (shebang, rest) = split_shebang(body);
        let new = format!("{bom}{shebang}{PRE_MARKER}\n{pre_source}\n{rest}");
        let new = format!(
            "{}\n{POST_MARKER}\n{post_source}\n",
            new.trim_end_matches('\n')
        );
        Dotfile::new(new)
    }

    /// The lines after the BOM without the markers and source lines (compared without trailing whitespace), the
    /// rest trimmed and ended with one `\n`.
    pub(crate) fn without_blocks(&self, pre_source: &str, post_source: &str) -> Dotfile {
        let (bom, body) = split_bom(&self.text);
        let kept: String = body
            .split_inclusive('\n')
            .filter(|l| {
                let line = l.trim_end();
                !([PRE_MARKER, POST_MARKER].contains(&line) || [pre_source, post_source].contains(&line))
            })
            .collect();
        Dotfile::new(format!("{bom}{}\n", kept.trim()))
    }

    /// A marker or source line (compared without trailing whitespace) on any line of the text as it is: a BOM
    /// before the first line is not stripped.
    pub(crate) fn has_own_line(&self, pre_source: &str, post_source: &str) -> bool {
        self.text.lines().any(|l| {
            let line = l.trim_end();
            [PRE_MARKER, POST_MARKER].contains(&line) || [pre_source, post_source].contains(&line)
        })
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

/// First line kept on top iff it starts with `#!`.
fn split_shebang(s: &str) -> (String, &str) {
    if !s.starts_with("#!") {
        return (String::new(), s);
    }
    match s.find('\n') {
        Some(i) => (s[..=i].to_string(), &s[i + 1..]),
        None => (format!("{s}\n"), ""),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ZSHRC_PRE_SOURCE: &str =
        r#"[ -f "${HOME}/.tabsmith/shell/zshrc.pre.zsh" ] && . "${HOME}/.tabsmith/shell/zshrc.pre.zsh""#;
    const ZSHRC_POST_SOURCE: &str =
        r#"[ -f "${HOME}/.tabsmith/shell/zshrc.post.zsh" ] && . "${HOME}/.tabsmith/shell/zshrc.post.zsh""#;

    /// Any case and spacing, also right after a BOM.
    #[test]
    fn should_be_opted_out_by_a_tabsmith_ignore_comment() {
        let dotfile = Dotfile::new("\u{feff}  #  TABSMITH ignore\necho hi\n".to_string());

        let opted_out = dotfile.is_opted_out();

        assert!(opted_out);
    }

    /// The lines the shell runs, as the installed check compares them: an indented source line still counts.
    #[test]
    fn should_give_the_trimmed_lines_that_are_not_blank_or_comments() {
        let dotfile = Dotfile::new(
            "\u{feff}# tabsmith pre block. Keep at the top of this file.\n  [ -f \"${HOME}/.tabsmith/shell/bashrc.pre.bash\" ] && . \"${HOME}/.tabsmith/shell/bashrc.pre.bash\"\n\necho hi\r\n# tabsmith post block. Keep at the bottom of this file.\n[ -f \"${HOME}/.tabsmith/shell/bashrc.post.bash\" ] && . \"${HOME}/.tabsmith/shell/bashrc.post.bash\"\n".to_string(),
        );

        let lines = dotfile.sourced_lines();

        assert_eq!(
            lines,
            [
                "[ -f \"${HOME}/.tabsmith/shell/bashrc.pre.bash\" ] && . \"${HOME}/.tabsmith/shell/bashrc.pre.bash\"",
                "echo hi",
                "[ -f \"${HOME}/.tabsmith/shell/bashrc.post.bash\" ] && . \"${HOME}/.tabsmith/shell/bashrc.post.bash\"",
            ]
        );
    }

    #[test]
    fn should_put_the_pre_block_after_the_shebang_and_the_post_block_at_the_bottom() {
        let dotfile = Dotfile::new("#!/bin/zsh\nexport A=1\nalias ll='ls -l'\n".to_string());

        let installed = dotfile.with_blocks(ZSHRC_PRE_SOURCE, ZSHRC_POST_SOURCE);

        assert_eq!(
            installed.text(),
            format!("#!/bin/zsh\n{PRE_MARKER}\n{ZSHRC_PRE_SOURCE}\nexport A=1\nalias ll='ls -l'\n{POST_MARKER}\n{ZSHRC_POST_SOURCE}\n")
        );
    }

    #[test]
    fn should_take_the_blocks_out_and_give_back_the_user_lines() {
        let dotfile = Dotfile::new(format!(
            "#!/bin/zsh\n{PRE_MARKER}\n{ZSHRC_PRE_SOURCE}\nexport A=1\nalias ll='ls -l'\n{POST_MARKER}\n{ZSHRC_POST_SOURCE}\n"
        ));

        let uninstalled = dotfile.without_blocks(ZSHRC_PRE_SOURCE, ZSHRC_POST_SOURCE);

        assert_eq!(
            uninstalled.text(),
            "#!/bin/zsh\nexport A=1\nalias ll='ls -l'\n"
        );
    }

    #[test]
    fn should_tell_a_dotfile_with_an_own_line_from_one_without() {
        let traced = Dotfile::new(format!("export A=1\n{ZSHRC_POST_SOURCE}\n"));
        let untouched = Dotfile::new("export A=1\n".to_string());

        let found = [traced, untouched].map(|d| d.has_own_line(ZSHRC_PRE_SOURCE, ZSHRC_POST_SOURCE));

        assert_eq!(found, [true, false]);
    }
}
