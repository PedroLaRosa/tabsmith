//! Generator triggers and the generator result cache (SPEC §6.6.1, §6.6.5), until 6.4a moves them into
//! `completion` and deletes this file. The rest of the suggestion pipeline lives in `completion`.
//! OWNER: engine agent (internal API; only `engine.rs` uses it). Everything here is pure (no JS, no I/O); the
//! engine resolves function triggers / `getQueryTerm`s and runs generators.
use crate::specs::domain::value_objects::generator_cache_policy::GeneratorCachePolicy;
use serde_json::Value;
use std::collections::HashMap;

// ---------------------------------------------------------------------------------------------------------
// Generators: triggers and cache (SPEC §6.6.1, §6.6.5)

/// Cache decision for a generator run.
#[derive(Debug, PartialEq)]
pub enum Lookup {
    /// Within TTL.
    Fresh(Value),
    /// Past TTL, stale-while-revalidate: serve it; start a background refresh when `refresh`.
    Stale { value: Value, refresh: bool },
    /// Nothing usable: fetch (blocking for the slot).
    Miss,
    /// Nothing usable, but an identical fetch is in flight: share its task.
    Shared(u64),
}

struct CacheEntry {
    value: Option<Value>,
    at: u64,
    inflight: Option<u64>,
}

/// Generator result cache with ish semantics (D10): failures are never stored. Clock passed in (unix ms).
#[derive(Default)]
pub struct GenCache {
    map: HashMap<String, CacheEntry>,
}

impl GenCache {
    pub fn lookup(&self, key: &str, cfg: &GeneratorCachePolicy, now: u64) -> Lookup {
        match self.map.get(key) {
            Some(CacheEntry {
                value: Some(v),
                at,
                inflight,
            }) => {
                if now.saturating_sub(*at) <= cfg.ttl_ms() {
                    Lookup::Fresh(v.clone())
                } else if cfg.max_age() {
                    inflight.map_or(Lookup::Miss, Lookup::Shared)
                } else {
                    Lookup::Stale {
                        value: v.clone(),
                        refresh: inflight.is_none(),
                    }
                }
            }
            Some(CacheEntry {
                inflight: Some(t), ..
            }) => Lookup::Shared(*t),
            _ => Lookup::Miss,
        }
    }

    /// A fetch for `key` started as `task`.
    pub fn start(&mut self, key: &str, task: u64) {
        self.map
            .entry(key.to_string())
            .or_insert(CacheEntry {
                value: None,
                at: 0,
                inflight: None,
            })
            .inflight = Some(task);
    }

    pub fn store(&mut self, key: &str, value: Value, now: u64) {
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
    pub fn fail(&mut self, key: &str) {
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
    use serde_json::json;

    #[test]
    fn cache_ttl_swr_max_age() {
        let mut c = GenCache::default();
        let swr = GeneratorCachePolicy::default().with_ttl_ms(1000);
        let max_age = GeneratorCachePolicy::default()
            .with_ttl_ms(1000)
            .with_max_age(true);
        assert_eq!(c.lookup("k", &swr, 0), Lookup::Miss);
        c.start("k", 7);
        assert_eq!(c.lookup("k", &swr, 0), Lookup::Shared(7));
        c.fail("k"); // failures are not cached
        assert_eq!(c.lookup("k", &swr, 0), Lookup::Miss);
        c.store("k", json!(1), 100);
        assert_eq!(c.lookup("k", &swr, 1100), Lookup::Fresh(json!(1)));
        assert_eq!(
            c.lookup("k", &swr, 1101),
            Lookup::Stale {
                value: json!(1),
                refresh: true
            }
        );
        assert_eq!(c.lookup("k", &max_age, 1101), Lookup::Miss);
        c.start("k", 9);
        assert_eq!(
            c.lookup("k", &swr, 1101),
            Lookup::Stale {
                value: json!(1),
                refresh: false
            }
        );
        assert_eq!(c.lookup("k", &max_age, 1101), Lookup::Shared(9));
        c.fail("k"); // a failed refresh keeps the old value
        assert_eq!(
            c.lookup("k", &swr, 1101),
            Lookup::Stale {
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
