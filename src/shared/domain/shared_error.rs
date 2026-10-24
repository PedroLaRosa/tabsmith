use std::fmt;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SharedError {
    HomeDirectoryNotFound,
}

impl fmt::Display for SharedError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SharedError::HomeDirectoryNotFound => {
                f.write_str("could not determine the home directory (set HOME or TABSMITH_HOME)")
            }
        }
    }
}

impl std::error::Error for SharedError {}
