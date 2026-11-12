use crate::modules::specs::domain::spec_error::SpecError;
#[cfg(test)]
use std::cell::Cell;

/// The completion specs tabsmith ships with, put where the completion engine reads them.
pub(crate) trait SpecBundle {
    /// Installs the bundle unless it is there already. A failure is `BundleUnavailable`.
    fn install(&self) -> Result<(), SpecError>;
}

/// Answers every install with the result it was given, and counts the installs, for tests.
#[cfg(test)]
pub(crate) struct RecordingSpecBundle {
    result: Result<(), SpecError>,
    installs: Cell<usize>,
}

#[cfg(test)]
impl RecordingSpecBundle {
    pub(crate) fn answering(result: Result<(), SpecError>) -> Self {
        RecordingSpecBundle {
            result,
            installs: Cell::new(0),
        }
    }

    pub(crate) fn installs(&self) -> usize {
        self.installs.get()
    }
}

#[cfg(test)]
impl SpecBundle for RecordingSpecBundle {
    fn install(&self) -> Result<(), SpecError> {
        self.installs.set(self.installs.get() + 1);
        self.result.clone()
    }
}
