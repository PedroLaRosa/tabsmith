/// One row of `integrations status`: whether the integration is installed, its description, its shell's name and its
/// dotfile's name. Why a check failed is a `ShellIntegrationError`, never shown here.
#[derive(Debug, PartialEq)]
pub(crate) struct IntegrationStatus {
    pub(crate) installed: bool,
    pub(crate) description: String,
    pub(crate) shell: String,
    pub(crate) file_name: String,
}
