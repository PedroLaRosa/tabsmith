use std::io::IsTerminal;

/// `s` wrapped in SGR `code` when stdout is a terminal.
pub(crate) fn paint(code: &str, s: &str) -> String {
    if std::io::stdout().is_terminal() {
        format!("\x1b[{code}m{s}\x1b[0m")
    } else {
        s.to_string()
    }
}

/// `s` wrapped in SGR `code` when stderr is a terminal.
pub(crate) fn epaint(code: &str, s: &str) -> String {
    if std::io::stderr().is_terminal() {
        format!("\x1b[{code}m{s}\x1b[0m")
    } else {
        s.to_string()
    }
}

pub(crate) fn stdout_tty() -> bool {
    std::io::stdout().is_terminal()
}

pub(crate) fn sgr(s: &str, code: &str, on: bool) -> String {
    if on {
        format!("\x1b[{code}m{s}\x1b[0m")
    } else {
        s.to_string()
    }
}

pub(crate) fn bold(s: &str, on: bool) -> String {
    sgr(s, "1", on)
}
