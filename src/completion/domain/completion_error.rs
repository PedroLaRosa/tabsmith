use crate::specs::domain::spec_error::SpecError;

#[derive(Debug)]
pub(crate) enum CompletionError {
    Aborted,
    /// No suggestions (unknown spec, unconsumable token, load failure). Logged by the caller.
    NoSuggestions(String),
}

impl From<SpecError> for CompletionError {
    fn from(e: SpecError) -> Self {
        match e {
            SpecError::Aborted => CompletionError::Aborted,
            SpecError::Timeout | SpecError::Invalid(_) | SpecError::BundleUnavailable(_) => {
                CompletionError::NoSuggestions(e.to_string())
            }
        }
    }
}
