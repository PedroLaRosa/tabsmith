use std::fmt;
use std::io;
use std::path::PathBuf;

/// Why a [`JsonDocumentStore`](super::json_document_store::JsonDocumentStore) could not be read. Its Display and
/// `source()` chain print what the settings commands printed before (`{:#}`), e.g.
/// "<path> is corrupted: expected value at line 1 column 1".
#[derive(Debug)]
pub(crate) enum JsonDocumentStoreError {
    /// The file is not valid JSON.
    Corrupted { path: PathBuf, source: serde_json::Error },
    /// The file is valid JSON but not an object.
    NotAnObject { path: PathBuf },
    /// The file exists but could not be read.
    Read { path: PathBuf, source: io::Error },
}

impl fmt::Display for JsonDocumentStoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            JsonDocumentStoreError::Corrupted { path, .. } => write!(f, "{} is corrupted", path.display()),
            JsonDocumentStoreError::NotAnObject { path } => {
                write!(f, "{} is corrupted (not a JSON object)", path.display())
            }
            JsonDocumentStoreError::Read { path, .. } => write!(f, "reading {}", path.display()),
        }
    }
}

impl std::error::Error for JsonDocumentStoreError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            JsonDocumentStoreError::Corrupted { source, .. } => Some(source),
            JsonDocumentStoreError::NotAnObject { .. } => None,
            JsonDocumentStoreError::Read { source, .. } => Some(source),
        }
    }
}
