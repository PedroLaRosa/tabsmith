#[cfg(test)]
use crate::diagnostics::application::test_fixtures::EventLog;
use crate::diagnostics::domain::value_objects::check_outcome::CheckOutcome;

/// Everything doctor tells the user; every method is one line (or one pair of lines) of output.
pub(crate) trait DoctorReporter {
    fn start_progress(&self);
    /// Stopping a progress indicator that is not running does nothing.
    fn stop_progress(&self);
    fn group_header(&self, header: &str);
    fn outcome(&self, name: &str, outcome: &CheckOutcome);
    /// "Attempting to fix automatically..."
    fn fixing(&self);
    /// "Failed to fix: {reason}"
    fn fix_failed(&self, reason: &str);
    /// "Re-running check..."
    fn rechecking(&self);
    fn blank_line(&self);
    /// "Doctor found errors. Please fix them and try again."
    fn doctor_found_errors(&self);
    /// "Everything looks good!"
    fn everything_looks_good(&self);
    /// Where to look when tabsmith still does not work.
    fn support_hint(&self);
    /// Existing terminal sessions need a restart to pick the integration up.
    fn restart_notice(&self);
}

/// Writes each thing doctor tells the user as a line into the event log, so a test sees the order.
#[cfg(test)]
#[derive(Default)]
pub(crate) struct RecordingDoctorReporter {
    log: EventLog,
}

#[cfg(test)]
impl RecordingDoctorReporter {
    pub(crate) fn logging_to(mut self, log: EventLog) -> Self {
        self.log = log;
        self
    }

    pub(crate) fn events(&self) -> Vec<String> {
        self.log.entries()
    }

    fn record(&self, event: &str) {
        self.log.push(format!("reporter.{event}"));
    }
}

#[cfg(test)]
impl DoctorReporter for RecordingDoctorReporter {
    fn start_progress(&self) {
        self.record("start_progress");
    }

    fn stop_progress(&self) {
        self.record("stop_progress");
    }

    fn group_header(&self, header: &str) {
        self.record(&format!("group_header({header})"));
    }

    fn outcome(&self, name: &str, outcome: &CheckOutcome) {
        self.record(&format!("outcome({name}, {outcome:?})"));
    }

    fn fixing(&self) {
        self.record("fixing");
    }

    fn fix_failed(&self, reason: &str) {
        self.record(&format!("fix_failed({reason})"));
    }

    fn rechecking(&self) {
        self.record("rechecking");
    }

    fn blank_line(&self) {
        self.record("blank_line");
    }

    fn doctor_found_errors(&self) {
        self.record("doctor_found_errors");
    }

    fn everything_looks_good(&self) {
        self.record("everything_looks_good");
    }

    fn support_hint(&self) {
        self.record("support_hint");
    }

    fn restart_notice(&self) {
        self.record("restart_notice");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diagnostics::application::test_fixtures::EventLog;

    #[test]
    fn should_record_the_events_without_arguments_in_the_order_they_happen() {
        let reporter = RecordingDoctorReporter::default();

        reporter.start_progress();
        reporter.stop_progress();
        reporter.fixing();
        reporter.rechecking();
        reporter.blank_line();
        reporter.doctor_found_errors();
        reporter.everything_looks_good();
        reporter.support_hint();
        reporter.restart_notice();

        assert_eq!(
            reporter.events(),
            [
                "reporter.start_progress",
                "reporter.stop_progress",
                "reporter.fixing",
                "reporter.rechecking",
                "reporter.blank_line",
                "reporter.doctor_found_errors",
                "reporter.everything_looks_good",
                "reporter.support_hint",
                "reporter.restart_notice",
            ]
        );
    }

    #[test]
    fn should_record_the_header_of_a_group() {
        let reporter = RecordingDoctorReporter::default();

        reporter.group_header("Checking dotfiles");

        assert_eq!(
            reporter.events(),
            ["reporter.group_header(Checking dotfiles)"]
        );
    }

    #[test]
    fn should_record_the_name_and_the_outcome_of_a_check() {
        let reporter = RecordingDoctorReporter::default();

        reporter.outcome("data directory", &CheckOutcome::Warn("late".into()));

        assert_eq!(
            reporter.events(),
            ["reporter.outcome(data directory, Warn(\"late\"))"]
        );
    }

    #[test]
    fn should_record_the_reason_a_fix_failed() {
        let reporter = RecordingDoctorReporter::default();

        reporter.fix_failed("disk full");

        assert_eq!(reporter.events(), ["reporter.fix_failed(disk full)"]);
    }

    #[test]
    fn should_write_its_events_into_the_log_it_shares() {
        let log = EventLog::default();
        let reporter = RecordingDoctorReporter::default().logging_to(log.clone());

        reporter.blank_line();

        assert_eq!(log.entries(), ["reporter.blank_line"]);
    }
}
