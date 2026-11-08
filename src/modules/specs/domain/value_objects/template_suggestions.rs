use crate::modules::specs::domain::value_objects::spec_suggestion::SpecSuggestion;
use crate::modules::specs::domain::value_objects::spec_value::SpecValue;

/// What `filterTemplateSuggestions` receives.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum TemplateSuggestions {
    /// The suggestions a template listed.
    Listed(Vec<SpecSuggestion>),
    /// A templated custom generator result, passed back unchanged.
    Returned(SpecValue),
}
