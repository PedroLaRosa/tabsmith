//! Generator triggers and the generator result cache (SPEC §6.6.1, §6.6.5), until 6.4a moves them into
//! `completion` and deletes this file. The rest of the suggestion pipeline lives in `completion`.
//! OWNER: engine agent (internal API; only `engine.rs` uses it). Everything here is pure (no JS, no I/O); the
//! engine resolves function triggers / `getQueryTerm`s and runs generators.
use crate::specs::domain::value_objects::generator_cache_policy::GeneratorCachePolicy;
use crate::specs::domain::value_objects::generator_trigger::GeneratorTrigger;
use crate::completion::domain::value_objects::suggestion_query::after_sep;
use serde_json::Value;
use std::collections::HashMap;

// ---------------------------------------------------------------------------------------------------------
// Generators: triggers and cache (SPEC §6.6.1, §6.6.5)

/// Non-function trigger decision (`new` = current search term, `old` = previous); `None` for `GeneratorTrigger::Fn`.
pub fn triggered(t: &GeneratorTrigger, new: &str, old: &str) -> Option<bool> {
    let last = |s: &str, p: &str| s.rfind(p).map(|i| s[..i].chars().count()); // JS lastIndexOf (char units)
    Some(match t {
        GeneratorTrigger::Str(p) => last(new, p) != last(old, p),
        GeneratorTrigger::Change => new != old,
        GeneratorTrigger::Threshold(n) => new.chars().count() > *n && old.chars().count() <= *n,
        GeneratorTrigger::Match(list) => {
            list.iter().position(|x| x == new) != list.iter().position(|x| x == old)
        }
        GeneratorTrigger::Fn(_) => return None,
    })
}

/// Bundled `filepaths()` trigger: the directory part (up to the last separator, see [`after_sep`]) changed.
pub fn dir_changed(new: &str, old: &str, backslash: bool) -> bool {
    let dir = |s: &str| s[..after_sep(s, backslash)].to_string();
    dir(new) != dir(old)
}

/// `[cacheByDirectory ? cwd : "", key].join(",")`.
pub fn cache_key(cfg: &GeneratorCachePolicy, cwd: &str, key: String) -> String {
    format!("{},{key}", if cfg.cache_by_directory() { cwd } else { "" })
}

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
    use serde_json::json;

    #[test]
    fn triggers() {
        let s = GeneratorTrigger::Str("/".into());
        assert_eq!(triggered(&s, "src/", "src"), Some(true));
        assert_eq!(triggered(&s, "src/a", "src/"), Some(false));
        assert_eq!(triggered(&GeneratorTrigger::Change, "a", "a"), Some(false));
        assert_eq!(triggered(&GeneratorTrigger::Change, "ab", "a"), Some(true));
        assert_eq!(triggered(&GeneratorTrigger::Threshold(2), "abc", "ab"), Some(true));
        assert_eq!(triggered(&GeneratorTrigger::Threshold(2), "ab", "abc"), Some(false));
        assert_eq!(
            triggered(&GeneratorTrigger::Threshold(2), "abcd", "abc"),
            Some(false)
        );
        let m = GeneratorTrigger::Match(vec!["x".into(), "y".into()]);
        assert_eq!(triggered(&m, "y", "x"), Some(true));
        assert_eq!(triggered(&m, "z", "w"), Some(false));
        assert_eq!(triggered(&GeneratorTrigger::Fn(1), "a", "b"), None);
        assert!(dir_changed("src/a", "lib/a", false));
        assert!(!dir_changed("src/ab", "src/a", false));
        assert!(dir_changed("src/", "src", false));
        // windows-backslash-paths: `\` separates in pwsh only (POSIX: it is the escape char)
        assert!(dir_changed("src\\", "src", true));
        assert!(!dir_changed("src\\ab", "src\\a", true));
        assert!(!dir_changed("my\\ ", "my", false));
    }

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
