use crate::completion::domain::value_objects::cache_lookup::CacheLookup;
use crate::specs::domain::value_objects::generator_cache_policy::GeneratorCachePolicy;
use crate::specs::domain::value_objects::task_id::TaskId;
use std::collections::HashMap;

struct CacheEntry<V> {
    value: Option<V>,
    at: u64,
    inflight: Option<TaskId>,
}

/// Generator result cache with ish semantics (D10): failures are never stored. Clock passed in (unix ms).
pub(crate) struct GeneratorCache<V> {
    map: HashMap<String, CacheEntry<V>>,
}

impl<V> Default for GeneratorCache<V> {
    fn default() -> Self {
        GeneratorCache {
            map: HashMap::new(),
        }
    }
}

impl<V: Clone> GeneratorCache<V> {
    pub(crate) fn lookup(&self, key: &str, cfg: &GeneratorCachePolicy, now: u64) -> CacheLookup<V> {
        match self.map.get(key) {
            Some(CacheEntry {
                value: Some(v),
                at,
                inflight,
            }) => {
                if now.saturating_sub(*at) <= cfg.ttl_ms() {
                    CacheLookup::Fresh(v.clone())
                } else if cfg.max_age() {
                    inflight.map_or(CacheLookup::Miss, CacheLookup::Shared)
                } else {
                    CacheLookup::Stale {
                        value: v.clone(),
                        refresh: inflight.is_none(),
                    }
                }
            }
            Some(CacheEntry {
                inflight: Some(t), ..
            }) => CacheLookup::Shared(*t),
            _ => CacheLookup::Miss,
        }
    }

    /// A fetch for `key` started as `task`.
    pub(crate) fn start(&mut self, key: &str, task: TaskId) {
        self.map
            .entry(key.to_string())
            .or_insert(CacheEntry {
                value: None,
                at: 0,
                inflight: None,
            })
            .inflight = Some(task);
    }

    pub(crate) fn store(&mut self, key: &str, value: V, now: u64) {
        self.map.insert(
            key.to_string(),
            CacheEntry {
                value: Some(value),
                at: now,
                inflight: None,
            },
        );
    }

    /// The fetch failed: keep any older value, forget the in-flight marker.
    pub(crate) fn fail(&mut self, key: &str) {
        if let Some(e) = self.map.get_mut(key) {
            e.inflight = None;
            if e.value.is_none() {
                self.map.remove(key);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::completion::domain::services::generator_trigger_policy::cache_key;
    use crate::completion::domain::value_objects::cache_lookup::CacheLookup;
    use crate::specs::domain::value_objects::generator_cache_policy::GeneratorCachePolicy;
    use serde_json::json;

    #[test]
    fn cache_ttl_swr_max_age() {
        let mut c = GeneratorCache::default();
        let swr = GeneratorCachePolicy::default().with_ttl_ms(1000);
        let max_age = GeneratorCachePolicy::default()
            .with_ttl_ms(1000)
            .with_max_age(true);
        assert_eq!(c.lookup("k", &swr, 0), CacheLookup::Miss);
        c.start("k", 7);
        assert_eq!(c.lookup("k", &swr, 0), CacheLookup::Shared(7));
        c.fail("k"); // failures are not cached
        assert_eq!(c.lookup("k", &swr, 0), CacheLookup::Miss);
        c.store("k", json!(1), 100);
        assert_eq!(c.lookup("k", &swr, 1100), CacheLookup::Fresh(json!(1)));
        assert_eq!(
            c.lookup("k", &swr, 1101),
            CacheLookup::Stale {
                value: json!(1),
                refresh: true
            }
        );
        assert_eq!(c.lookup("k", &max_age, 1101), CacheLookup::Miss);
        c.start("k", 9);
        assert_eq!(
            c.lookup("k", &swr, 1101),
            CacheLookup::Stale {
                value: json!(1),
                refresh: false
            }
        );
        assert_eq!(c.lookup("k", &max_age, 1101), CacheLookup::Shared(9));
        c.fail("k"); // a failed refresh keeps the old value
        assert_eq!(
            c.lookup("k", &swr, 1101),
            CacheLookup::Stale {
                value: json!(1),
                refresh: true
            }
        );
        let by_dir = GeneratorCachePolicy::default().with_cache_by_directory(true);
        assert_eq!(
            cache_key(&by_dir, "/w", "git branch".into()),
            "/w,git branch"
        );
        assert_eq!(cache_key(&swr, "/w", "git branch".into()), ",git branch");
    }
}
