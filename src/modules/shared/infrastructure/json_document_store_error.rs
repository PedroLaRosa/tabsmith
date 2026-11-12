use crate::modules::shared::infrastructure::atomic_file_error::AtomicFileError;
use std::fmt;
use std::io;
use std::path::PathBuf;

/// Why a [`JsonDocumentStore`](super::json_document_store::JsonDocumentStore) could not be read or saved. Its
/// Display and `source()` chain print what the settings commands printed before (`{:#}`), e.g.
/// "<path> is corrupted: expected value at line 1 column 1".
#[derive(Debug)]
pub(crate) enum JsonDocumentStoreError {
    /// The file is not valid JSON.
    Corrupted { path: PathBuf, source: serde_json::Error },
    /// The file is valid JSON but not an object.
    NotAnObject { path: PathBuf },
    /// The file exists but could not be read.
    Read { path: PathBuf, source: io::Error },
    /// The document could not be turned into JSON text. Shown as the serde error itself.
    Serialize(serde_json::Error),
    /// Writing the file failed. Shown as the [`AtomicFileError`] itself.
    Write(AtomicFileError),
}

impl fmt::Display for JsonDocumentStoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            JsonDocumentStoreError::Corrupted { path, .. } => write!(f, "{} is corrupted", path.display()),
            JsonDocumentStoreError::NotAnObject { path } => {
                write!(f, "{} is corrupted (not a JSON object)", path.display())
            }
            JsonDocumentStoreError::Read { path, .. } => write!(f, "reading {}", path.display()),
            JsonDocumentStoreError::Serialize(error) => write!(f, "{error}"),
            JsonDocumentStoreError::Write(error) => write!(f, "{error}"),
        }
    }
}

impl std::error::Error for JsonDocumentStoreError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            JsonDocumentStoreError::Corrupted { source, .. } => Some(source),
            JsonDocumentStoreError::NotAnObject { .. } => None,
            JsonDocumentStoreError::Read { source, .. } => Some(source),
            // Display already is the wrapped error: continue with its own cause so nothing prints twice.
            JsonDocumentStoreError::Serialize(error) => error.source(),
            JsonDocumentStoreError::Write(error) => error.source(),
        }
    }
}
