use crate::modules::shell_integration::application::integration_failure::IntegrationFailure;
use crate::modules::shell_integration::application::integration_status::IntegrationStatus;

/// What `integrations status` shows: one row per integration, shell by shell, then the shells whose integrations
/// could not be listed, in shell order.
#[derive(Debug)]
pub(crate) struct IntegrationStatusReport {
    pub(crate) integrations: Vec<IntegrationStatus>,
    pub(crate) failures: Vec<IntegrationFailure>,
}
