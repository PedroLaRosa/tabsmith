use crate::shell_integration::application::integration_failure::IntegrationFailure;

/// What `tabsmith setup` has done so far, reported as it happens so the CLI prints each line at its moment.
#[derive(Debug, PartialEq)]
pub(crate) enum SetupProgress {
    /// The automatic install ran; true if something was installed.
    Installed(bool),
    /// The automatic install failed; the manual instructions follow.
    AutomaticInstallFailed(Vec<IntegrationFailure>),
    /// The lines to add to the dotfiles by hand.
    ManualInstructions(String),
    /// Not every script file the manual lines source could be written.
    ScriptFilesFailed(Vec<IntegrationFailure>),
}
