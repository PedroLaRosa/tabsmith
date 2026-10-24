use std::fmt;
use std::io;
use std::path::PathBuf;

/// Why an [`AtomicFile`](super::atomic_file::AtomicFile) write failed. Its Display and `source()` chain print what
/// the callers' `anyhow` errors printed before (`{:#}`): a bare I/O error, or "writing <path>: <I/O error>".
#[derive(Debug)]
pub(crate) enum AtomicFileError {
    /// The path has no parent directory to hold the file.
    NoParentDirectory,
    /// A step before the final rename failed. `path` is the directory for directory steps, else the path written.
    Io { path: PathBuf, source: io::Error },
    /// Renaming the temp file over the target failed. `path` is the path the caller asked to write.
    Rename { path: PathBuf, source: io::Error },
}

impl fmt::Display for AtomicFileError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AtomicFileError::NoParentDirectory => write!(f, "no parent directory"),
            AtomicFileError::Io { source, .. } => write!(f, "{source}"),
            AtomicFileError::Rename { path, .. } => write!(f, "writing {}", path.display()),
        }
    }
}

impl std::error::Error for AtomicFileError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            AtomicFileError::NoParentDirectory => None,
            // Display already is the I/O error: continue with its own cause so nothing prints twice.
            AtomicFileError::Io { source, .. } => source.source(),
            AtomicFileError::Rename { source, .. } => Some(source),
        }
    }
}
