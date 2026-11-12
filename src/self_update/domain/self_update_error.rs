use std::fmt;

/// Why an update did not happen. Every failure leaves the executable as it was.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum SelfUpdateError {
    /// The latest release names a version that is not `MAJOR.MINOR.PATCH`.
    InvalidVersion { version: String },
    /// `SHA256SUMS.sig` is not the release signing key's signature over `SHA256SUMS`.
    UntrustedChecksums,
    /// The release's `SHA256SUMS` has no line for this platform's binary.
    MissingChecksum { asset: String },
    /// The downloaded binary is not the one `SHA256SUMS` lists.
    ChecksumMismatch { asset: String },
}

impl fmt::Display for SelfUpdateError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
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
        }
    }
}

impl std::error::Error for SelfUpdateError {}
