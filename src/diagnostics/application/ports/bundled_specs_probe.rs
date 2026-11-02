#[cfg(test)]
use crate::diagnostics::application::test_fixtures::{logged_path, EventLog};
#[cfg(test)]
use std::cell::Cell;
use std::path::Path;

/// Asks whether the completion specs tabsmith ships with are on disk.
pub(crate) trait BundledSpecsProbe {
    /// The bundle of this version is extracted below `data_root`.
    fn is_extracted(&self, data_root: &Path) -> bool;
}

/// Answers whether the bundle is extracted from a value a test can flip, and logs each question.
#[cfg(test)]
pub(crate) struct InMemoryBundledSpecsProbe {
    extracted: Cell<bool>,
    log: EventLog,
}

#[cfg(test)]
impl InMemoryBundledSpecsProbe {
    pub(crate) fn extracted(extracted: bool) -> Self {
        InMemoryBundledSpecsProbe {
            extracted: Cell::new(extracted),
            log: EventLog::default(),
        }
    }

    pub(crate) fn logging_to(mut self, log: EventLog) -> Self {
        self.log = log;
        self
    }

    pub(crate) fn set_extracted(&self, extracted: bool) {
        self.extracted.set(extracted);
    }
}

#[cfg(test)]
impl BundledSpecsProbe for InMemoryBundledSpecsProbe {
    fn is_extracted(&self, data_root: &Path) -> bool {
        self.log.push(format!(
            "bundled_specs.is_extracted({})",
            logged_path(data_root)
        ));
        self.extracted.get()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diagnostics::application::test_fixtures::EventLog;

    #[test]
    fn should_answer_the_scripted_value() {
        let probe = InMemoryBundledSpecsProbe::extracted(true);

        let extracted = probe.is_extracted(Path::new("/data"));

        assert!(extracted);
    }

    #[test]
    fn should_answer_the_new_value_after_the_script_flips() {
        let probe = InMemoryBundledSpecsProbe::extracted(false);

        probe.set_extracted(true);

        assert!(probe.is_extracted(Path::new("/data")));
    }

    #[test]
    fn should_log_each_question_with_the_data_root() {
        let log = EventLog::default();
        let probe = InMemoryBundledSpecsProbe::extracted(true).logging_to(log.clone());

        probe.is_extracted(Path::new("/data"));

        assert_eq!(
            log.entries(),
            vec!["bundled_specs.is_extracted(/data)".to_string()]
        );
    }
}
