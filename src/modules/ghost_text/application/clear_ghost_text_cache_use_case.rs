use crate::modules::ghost_text::domain::entities::ghost_text_cache::GhostTextCache;
use std::sync::{Arc, Mutex};

/// A finished or ended command makes the LLM ghost texts of its line stale.
pub(crate) struct ClearGhostTextCacheUseCase {
    cache: Arc<Mutex<GhostTextCache>>,
}

impl ClearGhostTextCacheUseCase {
    pub(crate) fn new(cache: Arc<Mutex<GhostTextCache>>) -> Self {
        ClearGhostTextCacheUseCase { cache }
    }

    pub(crate) fn execute(&self) {
        if let Ok(mut cache) = self.cache.lock() {
            cache.clear();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn should_forget_every_cached_completion() {
        let mut cache = GhostTextCache::default();
        cache.insert(&["git status".to_string(), "git stash".to_string()]);
        let cache = Arc::new(Mutex::new(cache));
        let use_case = ClearGhostTextCacheUseCase::new(Arc::clone(&cache));

        use_case.execute();

        assert_eq!(cache.lock().unwrap().lookup("git st"), None);
    }
}
