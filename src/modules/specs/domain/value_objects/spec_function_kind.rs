/// Which of the ten kinds a spec function call is; it decides how its result is read.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SpecFunctionKind {
    QueryTerm,
    Trigger,
    Alias,
    LoadSpec,
    GenerateSpecCacheKey,
    GenerateSpec,
    Script,
    Custom,
    PostProcess,
    FilterTemplateSuggestions,
}
