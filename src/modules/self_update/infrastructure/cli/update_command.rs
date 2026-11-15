//! `tabsmith update` (SPEC §2.10).

use crate::modules::self_update::application::update_trigger::UpdateTrigger;
use crate::modules::self_update::domain::value_objects::update_outcome::UpdateOutcome;
use crate::modules::self_update::infrastructure::self_update_module::SelfUpdateModule;
use anyhow::{bail, Result};

/// Runs `update`; `module` is `None` on a platform no release carries a binary for. Every failure is an error that
/// says what went wrong (and, outside `~/.local/bin`, how to update instead), so the command exits 1.
pub(crate) fn run_update_command(module: Option<&SelfUpdateModule>) -> Result<i32> {
    let Some(module) = module else {
        bail!(
            "tabsmith has no release for {}-{}; update it the way you installed it",
            std::env::consts::OS,
            std::env::consts::ARCH
        );
    };
    let outcome = module.update_tabsmith.execute(UpdateTrigger::Manual)?;
    println!("{}", update_report(&outcome));
    Ok(0)
}

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
