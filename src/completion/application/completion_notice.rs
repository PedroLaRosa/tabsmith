/// What a completion service reports while it runs, for its infrastructure caller to log at that moment (D5).
#[derive(Debug, PartialEq)]
pub(crate) enum CompletionNotice {
    /// A spec's generateSpec failed: the whole warn line, `generateSpec ({command token}): {error:?}`.
    GenerateSpecFailed(String),
    /// A generator run failed (also a superseded or background one): the failure text, logged at debug.
    GeneratorFailed(String),
}
