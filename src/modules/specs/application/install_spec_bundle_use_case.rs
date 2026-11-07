use crate::modules::specs::application::ports::spec_bundle::SpecBundle;
use crate::modules::specs::domain::spec_error::SpecError;
use std::rc::Rc;

/// The completion specs are installed before anything completes with them: setup, doctor's fix and the engine.
pub(crate) struct InstallSpecBundleUseCase {
    bundle: Rc<dyn SpecBundle>,
}

impl InstallSpecBundleUseCase {
    pub(crate) fn new(bundle: Rc<dyn SpecBundle>) -> Self {
        InstallSpecBundleUseCase { bundle }
    }

    pub(crate) fn execute(&self) -> Result<(), SpecError> {
        self.bundle.install()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::specs::application::ports::spec_bundle::RecordingSpecBundle;

    #[test]
    fn should_install_the_spec_bundle_once() {
        let bundle = Rc::new(RecordingSpecBundle::answering(Ok(())));
        let use_case = InstallSpecBundleUseCase::new(bundle.clone());

        let installed = use_case.execute();

        assert_eq!((installed, bundle.installs()), (Ok(()), 1));
    }

    #[test]
    fn should_pass_the_bundle_failure_through() {
        let failure =
            SpecError::BundleUnavailable("creating /home/u/.tabsmith/specs: disk full".into());
        let use_case = InstallSpecBundleUseCase::new(Rc::new(RecordingSpecBundle::answering(Err(
            failure.clone(),
        ))));

        let installed = use_case.execute();

        assert_eq!(installed, Err(failure));
    }
}
