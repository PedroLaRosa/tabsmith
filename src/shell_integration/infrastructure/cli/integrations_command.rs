use crate::shared::infrastructure::terminal_style::sgr;
use crate::shell_integration::application::integration_failure::IntegrationFailure;

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
}
