use crate::completion::domain::value_objects::generator_key::GeneratorKey;
use crate::completion::domain::value_objects::suggestion_query::SuggestionQuery;
use crate::specs::domain::value_objects::spec_suggestion::SpecSuggestion;

/// One generator slot as seen by assembly.
pub(crate) struct GeneratorSlotView<'a> {
    pub(crate) key: GeneratorKey,
    /// `SuggestionQuery::Term` ⇔ the generator has no `getQueryTerm`.
    pub(crate) query: SuggestionQuery,
    pub(crate) loading: bool,
    pub(crate) items: &'a [SpecSuggestion],
}
