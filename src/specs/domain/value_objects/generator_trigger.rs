use crate::specs::domain::value_objects::spec_function_ref::SpecFunctionRef;

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum GeneratorTrigger {
    /// Re-run when the last index of this string in the search term changes.
    Str(String),
    /// `trigger(newToken, oldToken) -> bool`.
    Fn(SpecFunctionRef),
    /// `{on: "change"}`.
    Change,
    /// `{on: "threshold", length}`.
    Threshold(usize),
    /// `{on: "match", string: [...]}`.
    Match(Vec<String>),
}
