#[cfg(test)]
use std::cell::RefCell;

/// Where tabsmith's releases are published.
pub(crate) trait ReleaseChannel {
    /// The version of the latest release.
    fn latest_version(&self) -> String;
    /// The file named `file` of the release of `version`.
    fn download(&self, version: &str, file: &str) -> Vec<u8>;
}

/// A latest version and the files of its release, kept in memory; it remembers which files were downloaded.
#[cfg(test)]
pub(crate) struct InMemoryReleaseChannel {
    latest_version: String,
    downloads: RefCell<Vec<String>>,
}

#[cfg(test)]
impl InMemoryReleaseChannel {
    pub(crate) fn new(latest_version: &str) -> Self {
        InMemoryReleaseChannel {
            latest_version: latest_version.to_string(),
            downloads: RefCell::new(Vec::new()),
        }
    }

    /// The files downloaded so far, in order.
    pub(crate) fn downloads(&self) -> Vec<String> {
        self.downloads.borrow().clone()
    }
}

#[cfg(test)]
impl ReleaseChannel for InMemoryReleaseChannel {
    fn latest_version(&self) -> String {
        self.latest_version.clone()
    }

    fn download(&self, _version: &str, file: &str) -> Vec<u8> {
        self.downloads.borrow_mut().push(file.to_string());
        Vec::new()
    }
}
