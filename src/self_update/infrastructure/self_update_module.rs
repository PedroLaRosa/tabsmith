use crate::self_update::application::update_tabsmith_use_case::{UpdateTabsmithParts, UpdateTabsmithUseCase};
use crate::self_update::domain::value_objects::release_asset::ReleaseAsset;
use crate::self_update::infrastructure::adapters::file_installed_executable::FileInstalledExecutable;
use crate::self_update::infrastructure::adapters::github_release_channel::GithubReleaseChannel;
use crate::self_update::infrastructure::adapters::marker_file_update_check_repository::MarkerFileUpdateCheckRepository;
use crate::self_update::infrastructure::adapters::ring_ed25519_checksum_signature::{
    RingEd25519ChecksumSignature, RELEASE_SIGNING_KEY_HEX,
};
use crate::self_update::infrastructure::adapters::ring_sha256_digest::RingSha256Digest;
use crate::settings::domain::repositories::settings_repository::SettingsRepository;
use crate::shared::infrastructure::data_directory_from_env::user_home_from_env;
use crate::shared::infrastructure::os::process_info::exe_path;
use crate::shared::infrastructure::shared_dependencies::SharedDependencies;
use std::path::PathBuf;
use std::rc::Rc;

/// The self-update use case over GitHub Releases, the running binary and the `update-check` marker.
pub(crate) struct SelfUpdateModule {
    pub(crate) update_tabsmith: UpdateTabsmithUseCase,
}

/// `None` on a platform no release carries a binary for. Building opens no connection and reads no file; an unknown
/// binary path or home only makes the install read as not native.
pub(crate) fn build_self_update_module(
    dependencies: &SharedDependencies,
    settings: Rc<dyn SettingsRepository>,
) -> Option<SelfUpdateModule> {
    let asset = ReleaseAsset::for_platform(std::env::consts::OS, std::env::consts::ARCH)?;
    let executable = FileInstalledExecutable::new(
        exe_path().map(PathBuf::from).unwrap_or_default(),
        user_home_from_env().unwrap_or_default(),
    );
    let update_tabsmith = UpdateTabsmithUseCase::new(UpdateTabsmithParts {
        current_version: env!("CARGO_PKG_VERSION").to_string(),
        asset,
        channel: Rc::new(GithubReleaseChannel::new()),
        digest: Rc::new(RingSha256Digest),
        signature: Rc::new(RingEd25519ChecksumSignature::new(RELEASE_SIGNING_KEY_HEX)),
        executable: Rc::new(executable),
        update_checks: Rc::new(MarkerFileUpdateCheckRepository::new(dependencies.data_directory.clone())),
        settings,
        clock: dependencies.clock.clone(),
    });
    Some(SelfUpdateModule { update_tabsmith })
}
