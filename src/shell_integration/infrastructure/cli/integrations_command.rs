use crate::shared::infrastructure::terminal_style::{sgr, stdout_tty};
use crate::shell_integration::application::integration_failure::IntegrationFailure;

/// The failures of a run over integrations as one error, the text main prints after `error: ` (the legacy
/// `bail!`); a failed integration's description is bold when stdout is a terminal, as `paint` does.
pub(crate) fn integration_failures_error(failures: Vec<IntegrationFailure>) -> anyhow::Error {
    anyhow::Error::msg(failures_text(&failures, stdout_tty()))
}

/// The failures each in a paragraph of its own after a blank line, a failed integration's description in bold when
/// `styled` (SPEC §2.6).
fn failures_text(failures: &[IntegrationFailure], styled: bool) -> String {
    let parts: Vec<String> = failures
        .iter()
        .map(|failure| match failure {
            IntegrationFailure::Listing { shell, reason } => format!("{shell}: {reason}"),
            IntegrationFailure::Operation {
                description,
                reason,
            } => format!("{}: {reason}", sgr(description, "1", styled)),
        })
        .collect();
    format!("\n\n{}", parts.join("\n\n"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn should_word_a_shell_it_could_not_list_as_the_shell_and_reason_after_a_blank_line() {
        let failures = [IntegrationFailure::Listing {
            shell: "pwsh".into(),
            reason: "pwsh printed an empty $PROFILE".into(),
        }];

        let text = failures_text(&failures, false);

        assert_eq!(text, "\n\npwsh: pwsh printed an empty $PROFILE");
    }

    fn failed_zshrc() -> IntegrationFailure {
        IntegrationFailure::Operation {
            description: "zsh pre post into .zshrc".into(),
            reason: "Permission denied: /home/u/.zshrc".into(),
        }
    }

    #[test]
    fn should_put_a_blank_line_between_failures() {
        let failures = [
            IntegrationFailure::Listing {
                shell: "pwsh".into(),
                reason: "pwsh printed an empty $PROFILE".into(),
            },
            failed_zshrc(),
        ];

        let text = failures_text(&failures, false);

        assert_eq!(
            text,
            "\n\npwsh: pwsh printed an empty $PROFILE\n\nzsh pre post into .zshrc: Permission denied: /home/u/.zshrc"
        );
    }

    #[test]
    fn should_bold_the_description_of_a_failed_operation_when_styled() {
        let text = failures_text(&[failed_zshrc()], true);

        assert_eq!(text, "\n\n\x1b[1mzsh pre post into .zshrc\x1b[0m: Permission denied: /home/u/.zshrc");
    }

    #[test]
    fn should_word_a_failed_operation_plainly_when_not_styled() {
        let text = failures_text(&[failed_zshrc()], false);

        assert_eq!(text, "\n\nzsh pre post into .zshrc: Permission denied: /home/u/.zshrc");
    }

    /// A listing failure is never styled, so the text does not depend on whether stdout is a terminal.
    #[test]
    fn should_give_the_failures_text_as_the_error_main_prints() {
        let failures = vec![IntegrationFailure::Listing {
            shell: "pwsh".into(),
            reason: "pwsh printed an empty $PROFILE".into(),
        }];

        let error = integration_failures_error(failures);

        assert_eq!(format!("{error:#}"), "\n\npwsh: pwsh printed an empty $PROFILE");
    }
}
