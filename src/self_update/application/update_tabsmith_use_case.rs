use crate::self_update::application::ports::checksum_signature::ChecksumSignature;
use crate::self_update::application::ports::installed_executable::InstalledExecutable;
use crate::self_update::application::ports::release_channel::ReleaseChannel;
use crate::self_update::application::ports::sha256_digest::Sha256Digest;
use crate::self_update::application::update_trigger::UpdateTrigger;
use crate::self_update::domain::self_update_error::SelfUpdateError;
use crate::self_update::domain::value_objects::checksum_list::ChecksumList;
use crate::self_update::domain::value_objects::release_asset::ReleaseAsset;
use crate::self_update::domain::value_objects::update_outcome::UpdateOutcome;
use crate::specs::domain::value_objects::semantic_version::SemanticVersion;
use std::cmp::Ordering;
use std::rc::Rc;

const CHECKSUMS_FILE: &str = "SHA256SUMS";
const CHECKSUMS_SIGNATURE_FILE: &str = "SHA256SUMS.sig";

/// Brings the native install to the latest release (`tabsmith update`, and the background check at `_ pty` start).
pub(crate) struct UpdateTabsmithUseCase {
    current_version: String,
    asset: ReleaseAsset,
    channel: Rc<dyn ReleaseChannel>,
    digest: Rc<dyn Sha256Digest>,
    signature: Rc<dyn ChecksumSignature>,
    executable: Rc<dyn InstalledExecutable>,
}

/// What [`UpdateTabsmithUseCase`] is built from.
pub(crate) struct UpdateTabsmithParts {
    /// The version of the running binary.
    pub(crate) current_version: String,
    /// The release file that holds this platform's binary.
    pub(crate) asset: ReleaseAsset,
    pub(crate) channel: Rc<dyn ReleaseChannel>,
    pub(crate) digest: Rc<dyn Sha256Digest>,
    pub(crate) signature: Rc<dyn ChecksumSignature>,
    pub(crate) executable: Rc<dyn InstalledExecutable>,
}

impl UpdateTabsmithUseCase {
    pub(crate) fn new(parts: UpdateTabsmithParts) -> Self {
        UpdateTabsmithUseCase {
            current_version: parts.current_version,
            asset: parts.asset,
            channel: parts.channel,
            digest: parts.digest,
            signature: parts.signature,
            executable: parts.executable,
        }
    }

    pub(crate) fn execute(&self, _trigger: UpdateTrigger) -> Result<UpdateOutcome, SelfUpdateError> {
        let Some(latest_version) = self.newer_release()? else {
            return Ok(UpdateOutcome::UpToDate(self.current_version.clone()));
        };
        let binary = self.verified_binary(&latest_version)?;
        self.executable.replace(&binary, &latest_version);
        Ok(UpdateOutcome::Updated {
            from: self.current_version.clone(),
            to: latest_version,
        })
    }

    /// The latest release's version, normalised, when it is newer than the running one; `None` when it is not.
    fn newer_release(&self) -> Result<Option<String>, SelfUpdateError> {
        let latest_version = self.channel.latest_version();
        let Some(latest) = SemanticVersion::parse(&latest_version) else {
            return Err(SelfUpdateError::InvalidVersion { version: latest_version });
        };
        let current = SemanticVersion::parse(&self.current_version).unwrap_or_default();
        let is_newer = latest.compare(&current) == Ordering::Greater;
        Ok(is_newer.then(|| latest.to_string()))
    }

    /// This platform's binary from the release of `version`, once it matches the release's trusted `SHA256SUMS`.
    fn verified_binary(&self, version: &str) -> Result<Vec<u8>, SelfUpdateError> {
        let checksums = self.trusted_checksums(version)?;
        let listed_digest = checksums
            .digest_for(&self.asset)
            .ok_or_else(|| SelfUpdateError::MissingChecksum {
                asset: self.asset.name().to_string(),
            })?;
        let binary = self.channel.download(version, self.asset.name());
        if self.digest.hex_digest(&binary) != listed_digest {
            return Err(SelfUpdateError::ChecksumMismatch {
                asset: self.asset.name().to_string(),
            });
        }
        Ok(binary)
    }

    /// The release's `SHA256SUMS`, once `SHA256SUMS.sig` shows the release signing key signed it. Nothing else of the
    /// release is downloaded before this check.
    fn trusted_checksums(&self, version: &str) -> Result<ChecksumList, SelfUpdateError> {
        let sums = self.channel.download(version, CHECKSUMS_FILE);
        let signature = self.channel.download(version, CHECKSUMS_SIGNATURE_FILE);
        if !self.signature.is_authentic(&sums, &signature) {
            return Err(SelfUpdateError::UntrustedChecksums);
        }
        Ok(ChecksumList::parse(&String::from_utf8_lossy(&sums)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::self_update::application::ports::checksum_signature::PrefixedChecksumSignature;
    use crate::self_update::application::ports::installed_executable::InMemoryExecutable;
    use crate::self_update::application::ports::release_channel::InMemoryReleaseChannel;
    use crate::self_update::application::ports::sha256_digest::HexEncodingDigest;
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
            digest: Rc::new(HexEncodingDigest),
            signature: Rc::new(PrefixedChecksumSignature),
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
            (Ok(UpdateOutcome::UpToDate(CURRENT.to_string())), Vec::<String>::new())
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
                Ok(UpdateOutcome::Updated { from: CURRENT.to_string(), to: NEWER.to_string() }),
                Some(b"ok".to_vec())
            )
        );
    }

    #[test]
    fn should_leave_the_executable_untouched_when_the_binary_does_not_match_its_checksum() {
        let channel = signed_release(&format!("0000  {ASSET}\n"), b"ok");
        let executable = Rc::new(InMemoryExecutable::default());
        let use_case = use_case(&channel, &executable);

        let result = use_case.execute(UpdateTrigger::Manual);

        assert_eq!(
            (result, executable.installed_bytes()),
            (Err(SelfUpdateError::ChecksumMismatch { asset: ASSET.to_string() }), None)
        );
    }

    #[test]
    fn should_leave_the_executable_untouched_when_sha256sums_does_not_list_the_binary() {
        let channel = signed_release("6f6b  tabsmith-darwin-arm64\n", b"ok");
        let executable = Rc::new(InMemoryExecutable::default());
        let use_case = use_case(&channel, &executable);

        let result = use_case.execute(UpdateTrigger::Manual);

        assert_eq!(
            (result, executable.installed_bytes()),
            (Err(SelfUpdateError::MissingChecksum { asset: ASSET.to_string() }), None)
        );
    }

    #[test]
    fn should_not_download_the_binary_when_the_checksums_signature_does_not_verify() {
        let sums = format!("6f6b  {ASSET}\n");
        let channel = Rc::new(
            InMemoryReleaseChannel::new(NEWER)
                .with_file("SHA256SUMS", sums.as_bytes())
                .with_file("SHA256SUMS.sig", b"sig:signed by someone else")
                .with_file(ASSET, b"ok"),
        );
        let executable = Rc::new(InMemoryExecutable::default());
        let use_case = use_case(&channel, &executable);

        let result = use_case.execute(UpdateTrigger::Manual);

        assert_eq!(
            (result, channel.downloads(), executable.installed_bytes()),
            (
                Err(SelfUpdateError::UntrustedChecksums),
                vec!["SHA256SUMS".to_string(), "SHA256SUMS.sig".to_string()],
                None
            )
        );
    }

    #[test]
    fn should_download_nothing_when_the_latest_version_is_not_a_version() {
        let channel = Rc::new(InMemoryReleaseChannel::new("<!DOCTYPE html>"));
        let executable = Rc::new(InMemoryExecutable::default());
        let use_case = use_case(&channel, &executable);

        let result = use_case.execute(UpdateTrigger::Manual);

        assert_eq!(
            (result, channel.downloads(), executable.installed_bytes()),
            (
                Err(SelfUpdateError::InvalidVersion { version: "<!DOCTYPE html>".to_string() }),
                Vec::<String>::new(),
                None
            )
        );
    }

    /// Decision 8: only a newer release installs, so the updater never downgrades.
    #[test]
    fn should_stay_on_the_running_version_when_the_latest_release_is_older() {
        let channel = Rc::new(InMemoryReleaseChannel::new("0.0.9"));
        let use_case = use_case(&channel, &Rc::new(InMemoryExecutable::default()));

        let outcome = use_case.execute(UpdateTrigger::Manual);

        assert_eq!(
            (outcome, channel.downloads()),
            (Ok(UpdateOutcome::UpToDate(CURRENT.to_string())), Vec::<String>::new())
        );
    }
}
