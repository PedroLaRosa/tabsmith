use crate::shared::domain::value_objects::shell::Shell;

/// Single-quote `s` for `shell` (SPEC §5.3): POSIX `'\''`, fish `\'` + `\\`, pwsh `''`.
pub(crate) fn quote(shell: Shell, s: &str) -> String {
    match shell {
        Shell::Pwsh => format!("'{}'", pwsh_sq(s)),
        Shell::Fish => format!("'{}'", s.replace('\\', "\\\\").replace('\'', "\\'")),
        Shell::Bash | Shell::Zsh => format!("'{}'", s.replace('\'', r"'\''")),
    }
}

/// `s` for inside a pwsh '…' string: every quote PowerShell ends such a string with (`'` and U+2018-U+201B) doubled.
fn pwsh_sq(s: &str) -> String {
    s.chars()
        .flat_map(|c| {
            std::iter::repeat_n(
                c,
                1 + usize::from("'\u{2018}\u{2019}\u{201A}\u{201B}".contains(c)),
            )
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn integrations_quote() {
        assert_eq!(quote(Shell::Zsh, "/a b/it's"), r"'/a b/it'\''s'");
        assert_eq!(quote(Shell::Fish, r"/a\it's"), r"'/a\\it\'s'");
        assert_eq!(quote(Shell::Pwsh, r"C:\it's"), r"'C:\it''s'");
    }
}
