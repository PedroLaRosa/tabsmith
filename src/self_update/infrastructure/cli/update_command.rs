//! `tabsmith update` (SPEC §2.10).

use crate::self_update::domain::value_objects::update_outcome::UpdateOutcome;

/// What `tabsmith update` prints once it is done.
fn update_report(outcome: &UpdateOutcome) -> String {
    match outcome {
        UpdateOutcome::Updated { from, to } => {
            format!("Updated tabsmith from {from} to {to}. Open a new terminal to use it.")
        }
        UpdateOutcome::UpToDate(version) => format!("tabsmith is up to date ({version})"),
        // Only the background check skips; `tabsmith update` passes every gate it has.
        UpdateOutcome::Skipped(_) => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn should_tell_the_user_to_open_a_new_terminal_after_an_update() {
        let outcome = UpdateOutcome::Updated { from: "0.1.0".to_string(), to: "0.2.0".to_string() };

        let report = update_report(&outcome);

        assert_eq!(report, "Updated tabsmith from 0.1.0 to 0.2.0. Open a new terminal to use it.");
    }

    #[test]
    fn should_tell_the_user_the_running_version_is_the_latest() {
        let outcome = UpdateOutcome::UpToDate("0.2.0".to_string());

        let report = update_report(&outcome);

        assert_eq!(report, "tabsmith is up to date (0.2.0)");
    }
}
