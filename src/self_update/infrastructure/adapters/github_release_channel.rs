use crate::self_update::application::ports::release_channel::ReleaseChannel;
use crate::self_update::domain::self_update_error::SelfUpdateError;
use std::time::Duration;

const RELEASES_URL: &str = "https://github.com/PedroLaRosa/tabsmith/releases";
/// For a whole request, the binary download included.
const REQUEST_TIMEOUT: Duration = Duration::from_secs(60);
/// Above ureq's 10 MB default, which a release binary can pass.
const MAX_DOWNLOAD_BYTES: u64 = 256 * 1024 * 1024;

/// tabsmith's GitHub Releases. The latest release's `VERSION` names it; every file then comes from that version's
/// own release (`releases/download/v<version>/<file>`), so all the files of one update come from one release even if
/// a newer one is published meanwhile. HTTPS only, redirects to the asset storage included.
// ponytail: no unit test; the release workflow's verify job runs `tabsmith update` against the real releases.
pub(crate) struct GithubReleaseChannel {
    agent: ureq::Agent,
}

impl GithubReleaseChannel {
    pub(crate) fn new() -> Self {
        let agent = ureq::Agent::config_builder()
            .timeout_global(Some(REQUEST_TIMEOUT))
            .https_only(true)
            .user_agent(format!("tabsmith/{}", env!("CARGO_PKG_VERSION")))
            .build()
            .new_agent();
        GithubReleaseChannel { agent }
    }

    fn fetch(&self, url: &str) -> Result<Vec<u8>, SelfUpdateError> {
        self.agent
            .get(url)
            .call()
            .and_then(|mut response| response.body_mut().with_config().limit(MAX_DOWNLOAD_BYTES).read_to_vec())
            .map_err(|error| SelfUpdateError::ChannelUnavailable(format!("{url}: {error}")))
    }
}

impl ReleaseChannel for GithubReleaseChannel {
    fn latest_version(&self) -> Result<String, SelfUpdateError> {
        let version = self.fetch(&format!("{RELEASES_URL}/latest/download/VERSION"))?;
        Ok(String::from_utf8_lossy(&version).trim().to_string())
    }

    fn download(&self, version: &str, file: &str) -> Result<Vec<u8>, SelfUpdateError> {
        self.fetch(&format!("{RELEASES_URL}/download/v{version}/{file}"))
    }
}
