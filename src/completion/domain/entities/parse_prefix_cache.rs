use crate::completion::domain::services::argv_parser::ArgvState;
use crate::specs::domain::entities::subcommand::Subcommand;
use std::collections::HashMap;
use std::rc::Rc;

/// ponytail: caches are cleared wholesale past this size (entries are never evicted otherwise).
const MAX_CACHED: usize = 1000;

/// Prefix-parse cache and generateSpec cache (SPEC §6.4), owned by whoever parses and lent to each parse.
#[derive(Default)]
pub(crate) struct ParsePrefixCache {
    prefix: HashMap<String, ArgvState>,
    generated: HashMap<String, Rc<Subcommand>>,
    /// The empty first-token spec, one Rc for every parse (the engine keys arg identity on its address).
    first: Rc<Subcommand>,
}

impl ParsePrefixCache {
    pub(crate) fn first(&self) -> &Rc<Subcommand> {
        &self.first
    }

    pub(crate) fn prefix_state(&self, key: &str) -> Option<&ArgvState> {
        self.prefix.get(key)
    }

    pub(crate) fn remember_prefix_state(&mut self, key: String, state: ArgvState) {
        if self.prefix.len() >= MAX_CACHED {
            self.prefix.clear();
        }
        self.prefix.insert(key, state);
    }

    pub(crate) fn generated_spec(&self, key: &str) -> Option<&Rc<Subcommand>> {
        self.generated.get(key)
    }

    pub(crate) fn remember_generated_spec(&mut self, key: String, spec: Rc<Subcommand>) {
        if self.generated.len() >= MAX_CACHED {
            self.generated.clear();
        }
        self.generated.insert(key, spec);
    }

    /// Forgets every cached state and generated spec; the first-token spec keeps its Rc.
    pub(crate) fn clear_keeping_first(&mut self) {
        *self = ParsePrefixCache {
            first: self.first.clone(),
            ..ParsePrefixCache::default()
        };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn state() -> ArgvState {
        ArgvState::initial(Rc::new(Subcommand::default()), "x", "x", Vec::new())
    }

    #[test]
    fn should_forget_every_prefix_state_before_storing_one_past_the_limit() {
        let mut cache = ParsePrefixCache::default();
        for key in 0..MAX_CACHED {
            cache.remember_prefix_state(key.to_string(), state());
        }

        cache.remember_prefix_state("0".to_string(), state());

        let kept: Vec<usize> = (0..MAX_CACHED)
            .filter(|key| cache.prefix_state(&key.to_string()).is_some())
            .collect();
        assert_eq!(kept, [0]);
    }

    #[test]
    fn should_forget_every_generated_spec_before_storing_one_past_the_limit() {
        let mut cache = ParsePrefixCache::default();
        for key in 0..MAX_CACHED {
            cache.remember_generated_spec(key.to_string(), Rc::new(Subcommand::default()));
        }

        cache.remember_generated_spec("0".to_string(), Rc::new(Subcommand::default()));

        let kept: Vec<usize> = (0..MAX_CACHED)
            .filter(|key| cache.generated_spec(&key.to_string()).is_some())
            .collect();
        assert_eq!(kept, [0]);
    }
}
