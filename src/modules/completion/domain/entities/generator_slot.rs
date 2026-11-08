use crate::modules::completion::domain::value_objects::generator_key::GeneratorKey;
use crate::modules::specs::domain::value_objects::spec_suggestion::SpecSuggestion;

/// Generator results of the current argument, one per generator (SPEC §6.6.1).
pub(crate) struct GeneratorSlot {
    key: GeneratorKey,
    result: Vec<SpecSuggestion>,
    loading: bool,
    /// Trigger generation: runs carrying another value are stale.
    generation: u64,
}

impl GeneratorSlot {
    pub(crate) fn new(key: GeneratorKey) -> Self {
        GeneratorSlot {
            key,
            result: Vec::new(),
            loading: false,
            generation: 0,
        }
    }

    pub(crate) fn key(&self) -> GeneratorKey {
        self.key
    }

    pub(crate) fn result(&self) -> &[SpecSuggestion] {
        &self.result
    }

    pub(crate) fn is_loading(&self) -> bool {
        self.loading
    }

    pub(crate) fn generation(&self) -> u64 {
        self.generation
    }

    /// The slot's generator fired as `generation`: older runs are stale from now on.
    pub(crate) fn fire(&mut self, generation: u64) {
        self.generation = generation;
        self.loading = true;
    }

    /// Takes `items` from a run of `generation`; a run of another generation changes nothing (`false`).
    pub(crate) fn finish(&mut self, generation: u64, items: Vec<SpecSuggestion>) -> bool {
        if self.generation == generation {
            self.result = items;
            self.loading = false;
            true
        } else {
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn should_ignore_the_result_of_an_older_generation() {
        let mut slot = GeneratorSlot::new(GeneratorKey::from_raw(7));
        slot.fire(2);

        let finished = slot.finish(1, vec![SpecSuggestion::from_script_output("main", false)]);

        assert_eq!((finished, slot.is_loading(), slot.result()), (false, true, &[][..]));
    }
}
