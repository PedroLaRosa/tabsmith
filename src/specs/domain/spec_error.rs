use std::fmt;

/// Why a spec could not be used. Shown as its own text, with no prefix.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum SpecError {
    /// The spec is malformed, e.g. "spec is not an object".
    Invalid(String),
}

impl fmt::Display for SpecError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SpecError::Invalid(text) => f.write_str(text),
        }
    }
}

impl std::error::Error for SpecError {}
