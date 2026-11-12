use crate::self_update::application::ports::installed_executable::InstalledExecutable;
use crate::self_update::application::ports::release_channel::ReleaseChannel;
use crate::self_update::application::update_trigger::UpdateTrigger;
use crate::self_update::domain::value_objects::release_asset::ReleaseAsset;
use crate::self_update::domain::value_objects::update_outcome::UpdateOutcome;
use std::rc::Rc;

/// Brings the native install to the latest release (`tabsmith update`, and the background check at `_ pty` start).
pub(crate) struct UpdateTabsmithUseCase {
    current_version: String,
    asset: ReleaseAsset,
    channel: Rc<dyn ReleaseChannel>,
    executable: Rc<dyn InstalledExecutable>,
}

/// What [`UpdateTabsmithUseCase`] is built from.
pub(crate) struct UpdateTabsmithParts {
    /// The version of the running binary.
    pub(crate) current_version: String,
    /// The release file that holds this platform's binary.
    pub(crate) asset: ReleaseAsset,
    pub(crate) channel: Rc<dyn ReleaseChannel>,
    pub(crate) executable: Rc<dyn InstalledExecutable>,
}

impl UpdateTabsmithUseCase {
    pub(crate) fn new(parts: UpdateTabsmithParts) -> Self {
        UpdateTabsmithUseCase {
            current_version: parts.current_version,
            asset: parts.asset,
            channel: parts.channel,
            executable: parts.executable,
        }
    }

    pub(crate) fn execute(&self, _trigger: UpdateTrigger) -> UpdateOutcome {
        let latest_version = self.channel.latest_version();
        if latest_version == self.current_version {
            return UpdateOutcome::UpToDate(self.current_version.clone());
        }
        let binary = self.channel.download(&latest_version, self.asset.name());
        self.executable.replace(&binary, &latest_version);
        UpdateOutcome::Updated {
            from: self.current_version.clone(),
            to: latest_version,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::self_update::application::ports::installed_executable::InMemoryExecutable;
    use crate::self_update::application::ports::release_channel::InMemoryReleaseChannel;
    use crate::self_update::domain::value_objects::release_asset::ReleaseAsset;

    const CURRENT: &str = "0.1.0";
    const NEWER: &str = "0.2.0";
    const ASSET: &str = "tabsmith-linux-x64";

    /// The newer release, holding `binary` and the `sums` listing it, signed.
    fn signed_release(sums: &str, binary: &[u8]) -> Rc<InMemoryReleaseChannel> {
        Rc::new(
            InMemoryReleaseChannel::new(NEWER)
                .with_file("SHA256SUMS", sums.as_bytes())
                .with_file("SHA256SUMS.sig", format!("sig:{sums}").as_bytes())
                .with_file(ASSET, binary),
        )
    }

    fn use_case(channel: &Rc<InMemoryReleaseChannel>, executable: &Rc<InMemoryExecutable>) -> UpdateTabsmithUseCase {
        UpdateTabsmithUseCase::new(UpdateTabsmithParts {
            current_version: CURRENT.to_string(),
            asset: ReleaseAsset::for_platform("linux", "x86_64").unwrap(),
            channel: channel.clone(),
            executable: executable.clone(),
        })
    }

    #[test]
    fn should_be_up_to_date_without_downloading_when_the_latest_release_is_the_running_version() {
        let channel = Rc::new(InMemoryReleaseChannel::new(CURRENT));
        let use_case = use_case(&channel, &Rc::new(InMemoryExecutable::default()));

        let outcome = use_case.execute(UpdateTrigger::Manual);

        assert_eq!(
            (outcome, channel.downloads()),
            (UpdateOutcome::UpToDate(CURRENT.to_string()), Vec::<String>::new())
        );
    }

    #[test]
    fn should_replace_the_executable_with_the_binary_of_a_newer_release() {
        let channel = signed_release(&format!("6f6b  {ASSET}\n"), b"ok");
        let executable = Rc::new(InMemoryExecutable::default());
        let use_case = use_case(&channel, &executable);

        let outcome = use_case.execute(UpdateTrigger::Manual);

        assert_eq!(
            (outcome, executable.installed_bytes()),
            (
                UpdateOutcome::Updated { from: CURRENT.to_string(), to: NEWER.to_string() },
                Some(b"ok".to_vec())
            )
        );
    }
}
