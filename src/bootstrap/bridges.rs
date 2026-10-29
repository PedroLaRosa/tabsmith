//! Ports of one module implemented over another module, wired here because only bootstrap may join modules.
use crate::shell_integration::application::ports::spec_bundle_installer::SpecBundleInstaller;
use crate::shell_integration::domain::shell_integration_error::ShellIntegrationError;
use crate::specs::application::install_spec_bundle_use_case::InstallSpecBundleUseCase;

/// setup's spec bundle, extracted by the specs module (specs' InstallSpecBundleUseCase).
pub(crate) struct SpecBundleBridge {
    install_spec_bundle: InstallSpecBundleUseCase,
}

impl SpecBundleBridge {
    pub(crate) fn new(install_spec_bundle: InstallSpecBundleUseCase) -> Self {
        SpecBundleBridge { install_spec_bundle }
    }
}

impl SpecBundleInstaller for SpecBundleBridge {
    fn install(&self) -> Result<(), ShellIntegrationError> {
        self.install_spec_bundle
            .execute()
            .map_err(|e| spec_bundle_failure(anyhow::Error::from(e)))
    }
}

/// The whole chain, as anyhow's `{:#}` prints it: the error's Display shows no source (P3, T15).
fn spec_bundle_failure(error: anyhow::Error) -> ShellIntegrationError {
    ShellIntegrationError::SpecBundle(format!("{error:#}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::Context;

    /// The bundle's extraction (EmbeddedSpecBundle) fails with a chain that BundleUnavailable holds already
    /// rendered (`creating <dir>: <io>`); main then prints `error: could not extract the
    /// completion specs: ` and that whole chain, as before.
    #[test]
    fn should_keep_the_whole_chain_of_a_failed_extraction() {
        let extraction: anyhow::Result<()> =
            Err(std::io::Error::other("disk full")).context("creating /home/u/.tabsmith/specs");

        let failure = spec_bundle_failure(extraction.unwrap_err());

        assert_eq!(
            format!("{failure:#}"),
            "could not extract the completion specs: creating /home/u/.tabsmith/specs: disk full"
        );
    }
}
