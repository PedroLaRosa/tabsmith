use crate::modules::specs::domain::value_objects::spec_suggestion::SpecSuggestion;

/// What a generator run's step gave: still waiting, its rows, or why it failed.
#[derive(Debug)]
pub(crate) enum GeneratorOutcome {
    Pending,
    Done(Vec<SpecSuggestion>),
    Fail(String),
}
