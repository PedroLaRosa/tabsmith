use std::fmt;

/// Why a theme file is not a valid theme: the text is not a theme file, or a colour key does not hold text.
#[derive(Debug)]
pub(crate) enum ThemeFileError {
    /// The reader's own message, line and column included.
    Json(String),
    /// The colour key whose value is not text.
    ColourNotText(String),
}

impl fmt::Display for ThemeFileError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ThemeFileError::Json(text) => write!(f, "{text}"),
            ThemeFileError::ColourNotText(key) => write!(f, "{key} must be a string"),
        }
    }
}
