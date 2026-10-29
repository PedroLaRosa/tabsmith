use crate::shared::infrastructure::shared_dependencies::SharedDependencies;
use crate::specs::application::install_spec_bundle_use_case::InstallSpecBundleUseCase;
use crate::specs::infrastructure::adapters::embedded_spec_bundle::EmbeddedSpecBundle;
use std::rc::Rc;

/// The specs use cases, over the spec bundle embedded in the binary.
pub(crate) struct SpecsModule {
    pub(crate) install_spec_bundle: InstallSpecBundleUseCase,
}

/// Only constructs: the data directory is read when the bundle is installed (D3).
pub(crate) fn build_specs_module(dependencies: &SharedDependencies) -> SpecsModule {
    let bundle = Rc::new(EmbeddedSpecBundle::new(dependencies.data_directory.clone()));
    SpecsModule {
        install_spec_bundle: InstallSpecBundleUseCase::new(bundle),
    }
}
