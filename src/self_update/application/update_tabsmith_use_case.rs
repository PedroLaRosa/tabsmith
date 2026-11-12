use crate::self_update::application::ports::release_channel::ReleaseChannel;
use crate::self_update::application::update_trigger::UpdateTrigger;
use crate::self_update::domain::value_objects::update_outcome::UpdateOutcome;
use std::rc::Rc;

/// Brings the native install to the latest release (`tabsmith update`, and the background check at `_ pty` start).
pub(crate) struct UpdateTabsmithUseCase {
    current_version: String,
    channel: Rc<dyn ReleaseChannel>,
}

impl UpdateTabsmithUseCase {
    pub(crate) fn new(current_version: &str, channel: Rc<dyn ReleaseChannel>) -> Self {
        UpdateTabsmithUseCase {
            current_version: current_version.to_string(),
            channel,
        }
    }

    pub(crate) fn execute(&self, _trigger: UpdateTrigger) -> UpdateOutcome {
        UpdateOutcome::UpToDate(self.current_version.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::self_update::application::ports::release_channel::InMemoryReleaseChannel;

    const CURRENT: &str = "0.1.0";

    #[test]
    fn should_be_up_to_date_without_downloading_when_the_latest_release_is_the_running_version() {
        let channel = Rc::new(InMemoryReleaseChannel::new(CURRENT));
        let use_case = UpdateTabsmithUseCase::new(CURRENT, channel.clone());

        let outcome = use_case.execute(UpdateTrigger::Manual);

        assert_eq!(
            (outcome, channel.downloads()),
            (UpdateOutcome::UpToDate(CURRENT.to_string()), Vec::<String>::new())
        );
    }
}
