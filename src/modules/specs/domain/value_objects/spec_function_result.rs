use crate::modules::specs::domain::entities::subcommand::Subcommand;
use crate::modules::specs::domain::value_objects::custom_generator_result::CustomGeneratorResult;
use crate::modules::specs::domain::value_objects::script_resolution::ScriptResolution;
use crate::modules::specs::domain::value_objects::spec_load_directive::SpecLoadDirective;
use crate::modules::specs::domain::value_objects::spec_suggestion::SpecSuggestion;

/// What a spec function returned, read the way its call's kind reads it: one variant per `SpecFunctionKind`.
#[derive(Clone, Debug)]
pub(crate) enum SpecFunctionResult {
    /// The query term, when the function returned a string.
    QueryTerm(Option<String>),
    /// Whether the generator runs again (JS truthiness).
    Trigger(bool),
    /// The alias, when the function returned a string (not trimmed).
    Alias(Option<String>),
    LoadSpec(Option<SpecLoadDirective>),
    /// The cache key, when the function returned a string.
    GenerateSpecCacheKey(Option<String>),
    /// The generated spec, or why it is not one. Boxed, so every result is not as large as a `Subcommand`.
    GenerateSpec(Result<Box<Subcommand>, String>),
    Script(ScriptResolution),
    Custom(CustomGeneratorResult),
    PostProcess(Vec<SpecSuggestion>),
    FilterTemplateSuggestions(Vec<SpecSuggestion>),
}
