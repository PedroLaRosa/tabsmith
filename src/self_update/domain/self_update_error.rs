use std::fmt;

/// Why an update did not happen. Every failure leaves the executable as it was.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum SelfUpdateError {
    /// The downloaded binary is not the one `SHA256SUMS` lists.
    ChecksumMismatch { asset: String },
}

impl fmt::Display for SelfUpdateError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SelfUpdateError::ChecksumMismatch { asset } => {
                write!(f, "the downloaded {asset} does not match its checksum in SHA256SUMS")
            }
        }
    }
}

impl std::error::Error for SelfUpdateError {}
