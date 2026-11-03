use std::fmt;

/// Why a spec could not be used. Shown as its own text, with no prefix.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum SpecError {
    /// A newer completion request arrived while a spec call was waiting; the request is kept for the engine.
    Aborted,
    /// A spec call did not finish before its deadline.
    Timeout,
    /// The spec or a spec call failed, as its own text: "spec is not an object", a module load error, a JS failure.
    Invalid(String),
    /// The spec bundle cannot be installed or read: the failure with its causes, already rendered.
    BundleUnavailable(String),
}

impl fmt::Display for SpecError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SpecError::Aborted => write!(f, "aborted by a newer request"),
            SpecError::Timeout => write!(f, "timed out"),
            SpecError::Invalid(text) | SpecError::BundleUnavailable(text) => f.write_str(text),
        }
    }
}

impl std::error::Error for SpecError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn should_say_a_newer_request_aborted_the_call() {
        let error = SpecError::Aborted;

        let text = error.to_string();

        assert_eq!(text, "aborted by a newer request");
    }

    #[test]
    fn should_say_a_spec_call_timed_out() {
        let error = SpecError::Timeout;

        let text = error.to_string();

        assert_eq!(text, "timed out");
    }
}
