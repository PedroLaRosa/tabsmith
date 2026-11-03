use crate::completion::domain::value_objects::generator_key::GeneratorKey;
use crate::completion::domain::value_objects::suggestion_query::SuggestionQuery;
use crate::specs::domain::value_objects::spec_suggestion::SpecSuggestion;

/// One generator slot as seen by assembly.
pub(crate) struct GeneratorSlotView<'a> {
    key: GeneratorKey,
    /// `SuggestionQuery::Term` ⇔ the generator has no `getQueryTerm`.
    query: SuggestionQuery,
    loading: bool,
    items: &'a [SpecSuggestion],
}

impl<'a> GeneratorSlotView<'a> {
    pub(crate) fn new(key: GeneratorKey, query: SuggestionQuery, loading: bool, items: &'a [SpecSuggestion]) -> Self {
        GeneratorSlotView {
            key,
            query,
            loading,
            items,
        }
    }

    pub(crate) fn key(&self) -> GeneratorKey {
        self.key
    }

    pub(crate) fn query(&self) -> &SuggestionQuery {
        &self.query
    }

    pub(crate) fn is_loading(&self) -> bool {
        self.loading
    }

    pub(crate) fn items(&self) -> &'a [SpecSuggestion] {
        self.items
    }
}
