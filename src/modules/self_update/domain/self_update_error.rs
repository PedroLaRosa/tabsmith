use std::fmt;
use std::path::PathBuf;

/// Why an update did not happen. Every failure leaves the executable as it was.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum SelfUpdateError {
    /// The release channel could not be reached or did not have a file; the text is its own error.
    ChannelUnavailable(String),
    /// tabsmith runs from `path`, not from the native install, so whatever installed it updates it.
    NotNativeInstall { path: PathBuf },
    /// The latest release names a version that is not `MAJOR.MINOR.PATCH`.
    InvalidVersion { version: String },
    /// `SHA256SUMS.sig` is not the release signing key's signature over `SHA256SUMS`.
    UntrustedChecksums,
    /// The release's `SHA256SUMS` has no line for this platform's binary.
    MissingChecksum { asset: String },
    /// The downloaded binary is not the one `SHA256SUMS` lists.
    ChecksumMismatch { asset: String },
    /// The verified binary could not take the executable's place (it did not run as the expected version, or the
    /// file could not be written or renamed); the text holds the cause.
    InstallFailed(String),
}

impl fmt::Display for SelfUpdateError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SelfUpdateError::ChannelUnavailable(reason) => write!(f, "could not download the latest release: {reason}"),
            SelfUpdateError::NotNativeInstall { path } => write!(
                f,
                "tabsmith runs from {}, not ~/.local/bin; update it the way you installed it (e.g. cargo install --path .)",
                path.display()
            ),
            SelfUpdateError::InvalidVersion { version } => {
                write!(f, "the latest release has no valid version: {version:?}")
            }
            SelfUpdateError::UntrustedChecksums => {
                f.write_str("the release's SHA256SUMS is not signed by the tabsmith release signing key")
            }
            SelfUpdateError::MissingChecksum { asset } => write!(f, "the release's SHA256SUMS does not list {asset}"),
            SelfUpdateError::ChecksumMismatch { asset } => {
                write!(f, "the downloaded {asset} does not match its checksum in SHA256SUMS")
            }
            SelfUpdateError::InstallFailed(reason) => write!(f, "could not install the new tabsmith: {reason}"),
        }
    }
}

impl std::error::Error for SelfUpdateError {}

#[cfg(test)]
mod tests {
    use super::*;

    /// `tabsmith update` prints these after `error:`; the background check logs them to pty.log.
    #[test]
    fn should_describe_each_update_failure() {
        let failures = [
            SelfUpdateError::NotNativeInstall { path: PathBuf::from("/home/ana/.cargo/bin/tabsmith") },
            SelfUpdateError::ChannelUnavailable("connection refused".to_string()),
            SelfUpdateError::InvalidVersion { version: "<!DOCTYPE html>".to_string() },
            SelfUpdateError::UntrustedChecksums,
            SelfUpdateError::MissingChecksum { asset: "tabsmith-linux-x64".to_string() },
            SelfUpdateError::ChecksumMismatch { asset: "tabsmith-linux-x64".to_string() },
            SelfUpdateError::InstallFailed("tabsmith --version printed tabsmith 0.1.0".to_string()),
        ];

        let described: Vec<String> = failures.iter().map(ToString::to_string).collect();

        assert_eq!(
            described,
            [
                "tabsmith runs from /home/ana/.cargo/bin/tabsmith, not ~/.local/bin; update it the way you installed it \
                 (e.g. cargo install --path .)",
                "could not download the latest release: connection refused",
                "the latest release has no valid version: \"<!DOCTYPE html>\"",
                "the release's SHA256SUMS is not signed by the tabsmith release signing key",
                "the release's SHA256SUMS does not list tabsmith-linux-x64",
                "the downloaded tabsmith-linux-x64 does not match its checksum in SHA256SUMS",
                "could not install the new tabsmith: tabsmith --version printed tabsmith 0.1.0",
            ]
        );
    }
}
