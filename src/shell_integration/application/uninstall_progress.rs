use crate::shell_integration::application::integration_failure::IntegrationFailure;

/// What `tabsmith uninstall` could not do, reported as it happens so the CLI prints each line at its moment.
#[derive(Debug, PartialEq)]
pub(crate) enum UninstallProgress {
    /// Not every integration could be taken out; reported before the purge.
    IntegrationsFailed(Vec<IntegrationFailure>),
}
