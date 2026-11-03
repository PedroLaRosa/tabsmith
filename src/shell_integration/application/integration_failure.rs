use crate::shared::domain::value_objects::shell::Shell;
use crate::shell_integration::domain::shell_integration_error::ShellIntegrationError;

/// One failure of a run over integrations: a shell whose integrations could not be listed, or one integration
/// (its description) whose operation failed. `reason` is the error's text.
#[derive(Debug, PartialEq)]
pub(crate) enum IntegrationFailure {
    Listing { shell: String, reason: String },
    Operation { description: String, reason: String },
}

impl IntegrationFailure {
    /// `shell`'s integrations could not be listed. One builder, so every run words a shell it cannot list alike.
    pub(crate) fn listing(shell: Shell, error: &ShellIntegrationError) -> Self {
        IntegrationFailure::Listing {
            shell: shell.name().to_string(),
            reason: error.to_string(),
        }
    }
}
