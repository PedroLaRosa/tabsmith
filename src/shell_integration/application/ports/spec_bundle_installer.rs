use crate::shell_integration::domain::shell_integration_error::ShellIntegrationError;
#[cfg(test)]
use std::cell::Cell;

/// Puts the completion specs tabsmith ships with where the completion engine reads them.
pub(crate) trait SpecBundleInstaller {
    /// A failure is a `SpecBundle` error.
    fn install(&self) -> Result<(), ShellIntegrationError>;
}

/// Succeeds, or fails with the message it was given, and counts how often it was asked, for tests.
#[cfg(test)]
#[derive(Default)]
pub(crate) struct RecordingSpecBundleInstaller {
    failure: Option<String>,
    asked: Cell<usize>,
}

#[cfg(test)]
impl RecordingSpecBundleInstaller {
    pub(crate) fn failing(message: &str) -> Self {
        RecordingSpecBundleInstaller {
            failure: Some(message.into()),
            ..Self::default()
        }
    }

    pub(crate) fn times_asked(&self) -> usize {
        self.asked.get()
    }
}

#[cfg(test)]
impl SpecBundleInstaller for RecordingSpecBundleInstaller {
    fn install(&self) -> Result<(), ShellIntegrationError> {
        self.asked.set(self.asked.get() + 1);
        match &self.failure {
            Some(message) => Err(ShellIntegrationError::SpecBundle(message.clone())),
            None => Ok(()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn should_succeed_and_record_each_install() {
        let installer = RecordingSpecBundleInstaller::default();

        let installs = [installer.install().is_ok(), installer.install().is_ok()];

        assert_eq!((installs, installer.times_asked()), ([true, true], 2));
    }

    #[test]
    fn should_fail_as_a_spec_bundle_error_with_the_message_it_was_given() {
        let installer =
            RecordingSpecBundleInstaller::failing("creating /home/u/.tabsmith/specs: disk full");

        let failure = installer.install();

        assert!(
            matches!(&failure, Err(ShellIntegrationError::SpecBundle(m)) if m == "creating /home/u/.tabsmith/specs: disk full")
                && installer.times_asked() == 1,
            "{failure:?}"
        );
    }
}
