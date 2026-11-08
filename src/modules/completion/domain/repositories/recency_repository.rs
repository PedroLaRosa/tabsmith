use crate::modules::completion::domain::entities::suggestion_recency::SuggestionRecency;
#[cfg(test)]
use std::cell::RefCell;

/// Where the suggestion recency outlives the session (SPEC §3.2 `state.json`).
pub(crate) trait RecencyRepository {
    /// The recency saved so far; empty when there is none or it cannot be read.
    fn load(&self) -> SuggestionRecency;
    /// Saves `recency`. The write may be throttled: it then happens later, with the latest recency saved by then.
    fn save(&self, recency: &SuggestionRecency);
    /// Writes a save still pending now, and only then: with nothing pending it writes nothing.
    fn flush(&self);
}

/// A recency kept in memory: load answers the last save.
#[cfg(test)]
#[derive(Default)]
pub(crate) struct InMemoryRecencyRepository {
    saved: RefCell<SuggestionRecency>,
}

#[cfg(test)]
impl RecencyRepository for InMemoryRecencyRepository {
    fn load(&self) -> SuggestionRecency {
        self.saved.borrow().clone()
    }

    fn save(&self, recency: &SuggestionRecency) {
        *self.saved.borrow_mut() = recency.clone();
    }

    fn flush(&self) {}
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn should_load_the_recency_it_saved() {
        let repository = InMemoryRecencyRepository::default();
        let mut recency = SuggestionRecency::default();
        recency.record("git".into(), "checkout".into(), 1_700_000_000_000);

        repository.save(&recency);

        assert_eq!(repository.load(), recency);
    }
}
