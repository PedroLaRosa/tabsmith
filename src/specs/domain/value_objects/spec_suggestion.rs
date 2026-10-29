use crate::specs::domain::value_objects::suggestion_kind::SuggestionKind;

/// A static suggestion, `additionalSuggestions` entry or generator result, before filtering.
#[derive(Clone, Debug, Default)]
pub(crate) struct SpecSuggestion {
    pub(crate) names: Vec<String>,
    pub(crate) display_name: Option<String>,
    pub(crate) insert_value: Option<String>,
    pub(crate) description: Option<String>,
    /// `None` → caller decides (static arg suggestions/generator results default to Arg).
    pub(crate) kind: Option<SuggestionKind>,
    pub(crate) priority: Option<f64>,
    pub(crate) hidden: bool,
    pub(crate) is_dangerous: bool,
    /// `context.templateType` (filepaths/folders/help).
    pub(crate) template_type: Option<String>,
    pub(crate) icon: Option<String>,
}
