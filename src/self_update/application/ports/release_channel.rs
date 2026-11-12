use crate::self_update::domain::self_update_error::SelfUpdateError;
#[cfg(test)]
use std::cell::{Cell, RefCell};
#[cfg(test)]
use std::collections::HashMap;

/// Where tabsmith's releases are published.
pub(crate) trait ReleaseChannel {
    /// The version of the latest release, as published.
    fn latest_version(&self) -> Result<String, SelfUpdateError>;
    /// The file named `file` of the release of `version`.
    fn download(&self, version: &str, file: &str) -> Result<Vec<u8>, SelfUpdateError>;
}

/// A latest version and the files of its release, kept in memory, or a channel that cannot be reached; it remembers
/// what it was asked for. A file the release does not carry fails to download, as a missing release asset does.
#[cfg(test)]
pub(crate) struct InMemoryReleaseChannel {
    latest_version: Result<String, SelfUpdateError>,
    files: HashMap<String, Vec<u8>>,
    latest_version_requests: Cell<usize>,
    downloads: RefCell<Vec<String>>,
}

#[cfg(test)]
impl InMemoryReleaseChannel {
    pub(crate) fn new(latest_version: &str) -> Self {
        Self::answering(Ok(latest_version.to_string()))
    }

    /// Every request fails with `connection refused`.
    pub(crate) fn unreachable() -> Self {
        Self::answering(Err(SelfUpdateError::ChannelUnavailable("connection refused".to_string())))
    }

    fn answering(latest_version: Result<String, SelfUpdateError>) -> Self {
        InMemoryReleaseChannel {
            latest_version,
            files: HashMap::new(),
            latest_version_requests: Cell::new(0),
            downloads: RefCell::new(Vec::new()),
        }
    }

    /// The latest release also carries `file`, holding `bytes`.
    pub(crate) fn with_file(mut self, file: &str, bytes: &[u8]) -> Self {
        self.files.insert(file.to_string(), bytes.to_vec());
        self
    }

    /// How many times the latest version was asked for.
    pub(crate) fn latest_version_requests(&self) -> usize {
        self.latest_version_requests.get()
    }

    /// The files downloaded so far, in order.
    pub(crate) fn downloads(&self) -> Vec<String> {
        self.downloads.borrow().clone()
    }
}

#[cfg(test)]
impl ReleaseChannel for InMemoryReleaseChannel {
    fn latest_version(&self) -> Result<String, SelfUpdateError> {
        self.latest_version_requests.set(self.latest_version_requests.get() + 1);
        self.latest_version.clone()
    }

    fn download(&self, _version: &str, file: &str) -> Result<Vec<u8>, SelfUpdateError> {
        self.downloads.borrow_mut().push(file.to_string());
        self.files
            .get(file)
            .cloned()
            .ok_or_else(|| SelfUpdateError::ChannelUnavailable(format!("{file}: 404 Not Found")))
    }
}
