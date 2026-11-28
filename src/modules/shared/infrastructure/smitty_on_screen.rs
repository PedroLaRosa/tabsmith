/// Smitty's `drawing` as setup and uninstall print it: nothing when stdout is not a terminal, so scripts and logs
/// stay clean.
pub(crate) fn smitty_on_screen(_drawing: &str, _stdout_is_terminal: bool) -> Option<String> {
    None
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
}
