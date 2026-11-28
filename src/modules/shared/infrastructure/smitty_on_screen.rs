use crate::modules::shared::infrastructure::terminal_style::{sgr, stdout_tty};

/// Smitty's orange fur, from the xterm 256-colour palette.
const FUR: &str = "38;5;208";

/// Smitty's `drawing` as setup and uninstall print it: painted in his fur under a blank line on a terminal, nothing
/// when stdout is not a terminal, so scripts and logs stay clean.
pub(crate) fn smitty_on_screen(drawing: &str, stdout_is_terminal: bool) -> Option<String> {
    if !stdout_is_terminal {
        return None;
    }
    Some(format!("\n{}\n", sgr(drawing.trim_end_matches('\n'), FUR, true)))
}

/// Prints Smitty's `drawing` on stdout when it is a terminal.
pub(crate) fn show_smitty(drawing: &str) {
    if let Some(shown) = smitty_on_screen(drawing, stdout_tty()) {
        print!("{shown}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::shared::domain::services::smitty;

    /// Piped into a script or a log, setup and uninstall print their lines without Smitty.
    #[test]
    fn should_keep_smitty_off_screen_when_stdout_is_not_a_terminal() {
        let stdout_is_terminal = false;

        let shown = smitty_on_screen(smitty::welcome(), stdout_is_terminal);

        assert_eq!(shown, None);
    }

    /// On a terminal Smitty comes in his orange fur, a blank line below the last step, with the colour reset before
    /// the final newline so it never leaks into the prompt.
    #[test]
    fn should_paint_smitty_orange_on_a_terminal() {
        let stdout_is_terminal = true;

        let shown = smitty_on_screen("=^w^=\n", stdout_is_terminal);

        assert_eq!(shown, Some("\n\x1b[38;5;208m=^w^=\x1b[0m\n".to_string()));
    }
}
