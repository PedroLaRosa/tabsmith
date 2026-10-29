//! Suggestion assembly, generators (triggers, cache, templates), filtering (prefix/fuzzysort), ranking,
//! insertion text, history rows and first-token items (SPEC §6.5–6.11).
//! OWNER: engine agent (internal API; only `engine.rs` uses it). Everything here is pure (no JS); the engine
//! resolves function triggers / `getQueryTerm`s and runs generators.
use crate::completion::domain::value_objects::annotation_kind::AnnotationKind;
use crate::completion::domain::value_objects::parsed_command_line::ParsedCommandLine;
use crate::specs::domain::entities::subcommand::Subcommand;
use crate::specs::domain::value_objects::generator_cache_policy::GeneratorCachePolicy;
use crate::specs::domain::value_objects::generator_trigger::GeneratorTrigger;
use crate::specs::domain::value_objects::spec_suggestion::SpecSuggestion;
use crate::completion::domain::value_objects::suggestion_candidate::SuggestionCandidate;
use crate::completion::domain::value_objects::suggestion_query::after_sep;
use crate::completion::domain::services::suggestion_filter::cmp_locale;
use crate::specs::domain::value_objects::suggestion_kind::SuggestionKind;
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

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

// ---------------------------------------------------------------------------------------------------------
// Templates (SPEC §6.6.4)

/// Bundled `shellExpand`: `~`, `$NAME`, `${NAME}` / `${NAME:-default}` from `env`.
pub fn shell_expand(s: &str, env: &HashMap<String, String>) -> String {
    let home = env
        .get("HOME")
        .or_else(|| env.get("USERPROFILE"))
        .map_or("~", String::as_str);
    let s = if s.starts_with('~') && (s.len() == 1 || s[1..].starts_with('/')) {
        format!("{home}{}", &s[1..])
    } else {
        s.to_string()
    };
    let is_name = |c: char| c.is_ascii_alphanumeric() || c == '_';
    // pass 1: $NAME
    let mut out = String::new();
    let mut rest = s.as_str();
    while let Some(i) = rest.find('$') {
        out.push_str(&rest[..i]);
        let after = &rest[i + 1..];
        let n = after.find(|c: char| !is_name(c)).unwrap_or(after.len());
        if n == 0 {
            out.push('$');
            rest = after;
            continue;
        }
        match env.get(&after[..n]) {
            Some(v) => out.push_str(v),
            None => out.push_str(&rest[i..i + 1 + n]),
        }
        rest = &after[n..];
    }
    out.push_str(rest);
    // pass 2: ${NAME} / ${NAME:-default}
    let s = out;
    let mut out = String::new();
    let mut rest = s.as_str();
    while let Some(i) = rest.find("${") {
        out.push_str(&rest[..i]);
        let after = &rest[i + 2..];
        let n = after.find(|c: char| !is_name(c)).unwrap_or(after.len());
        let tail = &after[n..];
        let parsed = if n == 0 {
            None
        } else if let Some(t) = tail.strip_prefix('}') {
            Some((None, t))
        } else if let Some(d) = tail.strip_prefix(":-") {
            d.find('}')
                .filter(|&j| j > 0)
                .map(|j| (Some(&d[..j]), &d[j + 1..]))
        } else {
            None
        };
        match parsed {
            Some((default, t)) => {
                let whole = &rest[i..rest.len() - t.len()];
                out.push_str(
                    env.get(&after[..n])
                        .map(String::as_str)
                        .or(default)
                        .unwrap_or(whole),
                );
                rest = t;
            }
            None => {
                out.push_str("${");
                rest = after;
            }
        }
    }
    out.push_str(rest);
    out
}

/// Bundled `getCurrentInsertedDirectory(cwd, searchTerm)`; `backslash`: `\` separates too (pwsh).
pub fn inserted_directory(
    cwd: &str,
    search_term: &str,
    env: &HashMap<String, String>,
    backslash: bool,
) -> PathBuf {
    let s = shell_expand(search_term, env);
    let dir = &s[..after_sep(&s, backslash)];
    let base = if cwd.ends_with('/') || (backslash && cwd.ends_with('\\')) {
        cwd.to_string()
    } else {
        format!("{cwd}/")
    };
    if dir.is_empty() {
        PathBuf::from(base)
    } else if dir.starts_with('/') || Path::new(dir).is_absolute() {
        PathBuf::from(dir)
    } else {
        PathBuf::from(format!("{base}{dir}"))
    }
}

/// Native `filepaths`/`folders` listing: `ls -1ApL` semantics, sorted non-dot then dot names, `.DS_Store`
/// skipped, `../` appended. Unreadable dir → `[]`.
pub fn list_dir(dir: &Path, files: bool, is_dangerous: bool) -> Vec<SpecSuggestion> {
    let Ok(rd) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut names: Vec<String> = rd
        .flatten()
        .map(|e| {
            let name = e.file_name().to_string_lossy().into_owned();
            let is_dir = std::fs::metadata(e.path()).is_ok_and(|m| m.is_dir()); // follows symlinks (-L)
            if is_dir {
                format!("{name}/")
            } else {
                name
            }
        })
        .filter(|n| !n.eq_ignore_ascii_case(".DS_Store"))
        .filter(|n| !n.chars().any(char::is_control)) // a CR in a name would run the line when inserted
        .collect();
    names.sort_by(|a, b| {
        a.starts_with('.')
            .cmp(&b.starts_with('.'))
            .then_with(|| cmp_locale(a, b))
    });
    names.push("../".into());
    names
        .into_iter()
        .filter_map(|n| {
            let folder = n.ends_with('/');
            if !folder && !files {
                return None;
            }
            Some(
                SpecSuggestion::default()
                    .with_insert_value(Some(n.clone()))
                    .with_kind(Some(if folder {
                        SuggestionKind::Folder
                    } else {
                        SuggestionKind::File
                    }))
                    .with_is_dangerous(is_dangerous)
                    .with_template_type(Some(if folder { "folders" } else { "filepaths" }.into()))
                    .with_names(vec![n]),
            )
        })
        .collect()
}

/// `help` template: subcommand names of the level above the triggering subcommand, minus its aliases.
pub fn help_items(pr: &ParsedCommandLine) -> Vec<SpecSuggestion> {
    let trig = pr
        .annotations()
        .iter()
        .rev()
        .find(|a| matches!(a.kind(), AnnotationKind::Subcommand | AnnotationKind::Option));
    let n = pr.path().len();
    let (level, exclude): (&Subcommand, &[String]) = match trig {
        Some(a) if a.kind() == AnnotationKind::Subcommand && n >= 2 => {
            (&pr.path()[n - 2], pr.path()[n - 1].names())
        }
        _ => (pr.path().last().unwrap_or(pr.completion()), &[]),
    };
    level
        .subcommands()
        .iter()
        .flat_map(|s| s.names().iter())
        .filter(|n| !exclude.contains(n))
        .map(|n| {
            SpecSuggestion::default()
                .with_names(vec![n.clone()])
                .with_insert_value(Some(n.clone()))
                .with_kind(Some(SuggestionKind::Special))
                .with_template_type(Some("help".into()))
        })
        .collect()
}

// ---------------------------------------------------------------------------------------------------------
// First token (SPEC §6.9)

/// Root spec names: no `/`, or a scoped npm name `@scope/name`.
fn is_root_spec(n: &str) -> bool {
    match n.matches('/').count() {
        0 => true,
        1 => n.starts_with('@'),
        _ => false,
    }
}

#[cfg(unix)]
fn exe_name(name: String, meta: &std::fs::Metadata, _pathext: Option<&str>) -> Option<String> {
    use std::os::unix::fs::PermissionsExt;
    (meta.permissions().mode() & 0o111 != 0).then_some(name)
}

#[cfg(not(unix))]
fn exe_name(name: String, _meta: &std::fs::Metadata, pathext: Option<&str>) -> Option<String> {
    let exts = pathext.unwrap_or(".COM;.EXE;.BAT;.CMD");
    let (stem, ext) = name.rsplit_once('.')?;
    exts.split(';')
        .any(|e| e.trim_start_matches('.').eq_ignore_ascii_case(ext))
        .then(|| stem.to_string())
}

fn executables(dir: &Path, pathext: Option<&str>) -> Vec<String> {
    let Ok(rd) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut out: Vec<String> = rd
        .flatten()
        .filter_map(|e| {
            let meta = std::fs::metadata(e.path()).ok().filter(|m| m.is_file())?;
            exe_name(e.file_name().to_string_lossy().into_owned(), &meta, pathext)
        })
        .collect();
    out.sort();
    out
}

/// Spec names (priority 40) ∪ aliases (shortcut, priority 100) ∪ PATH executables (priority 40), deduped by name.
pub fn first_token_items(
    specs: &[String],
    aliases: &HashMap<String, String>,
    path: Option<&str>,
    pathext: Option<&str>,
) -> Vec<SuggestionCandidate> {
    let mut seen = HashSet::new();
    let mut out = Vec::new();
    let mut push = |name: &str, kind: SuggestionKind, priority: f64, description: Option<&String>| {
        if !name.is_empty() && seen.insert(name.to_string()) {
            let mut s = SuggestionCandidate::new(name, kind);
            s.priority = priority;
            s.description = description.cloned();
            out.push(s);
        }
    };
    for n in specs.iter().filter(|n| is_root_spec(n)) {
        push(n, SuggestionKind::Subcommand, 40.0, None);
    }
    let mut al: Vec<_> = aliases.iter().collect();
    al.sort();
    for (k, v) in al {
        push(k, SuggestionKind::Shortcut, 100.0, Some(v));
    }
    for dir in path
        .map(|p| std::env::split_paths(p).collect::<Vec<_>>())
        .unwrap_or_default()
    {
        for exe in executables(&dir, pathext) {
            push(&exe, SuggestionKind::Subcommand, 40.0, None);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::completion::domain::services::insertion_composer::insertion;
    use crate::completion::domain::test_fixtures::{ctx, pr_for, sub};
    use crate::completion::domain::value_objects::suggestion_query::SuggestionQuery;
    use crate::completion::domain::value_objects::token_annotation::TokenAnnotation;
    use crate::shared::domain::value_objects::shell::Shell;
    use crate::shared::infrastructure::testing::temp_dir::TempDir;
    use serde_json::json;
    use std::rc::Rc;

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

    #[test]
    fn shell_expand_and_directory() {
        let env: HashMap<String, String> = [
            ("HOME".to_string(), "/h".to_string()),
            ("X".into(), "x".into()),
        ]
        .into();
        assert_eq!(shell_expand("~/a", &env), "/h/a");
        assert_eq!(shell_expand("~a", &env), "~a");
        assert_eq!(
            shell_expand("$X/$NOPE/${X}/${NOPE:-d}/${NOPE}", &env),
            "x/$NOPE/x/d/${NOPE}"
        );
        assert_eq!(
            inserted_directory("/w", "src/ma", &env, false),
            PathBuf::from("/w/src/")
        );
        assert_eq!(
            inserted_directory("/w/", "ma", &env, false),
            PathBuf::from("/w/")
        );
        assert_eq!(
            inserted_directory("/w", "/etc/pa", &env, false),
            PathBuf::from("/etc/")
        );
        assert_eq!(
            inserted_directory("/w", "$HOME/d", &env, false),
            PathBuf::from("/h/")
        );
        // windows-backslash-paths (pwsh)
        assert_eq!(
            inserted_directory("C:\\w", "src\\ma", &env, true),
            PathBuf::from("C:\\w/src\\")
        );
        assert_eq!(
            inserted_directory("C:\\w\\", "ma", &env, true),
            PathBuf::from("C:\\w\\")
        );
        assert_eq!(
            inserted_directory("/w", "my\\ d", &env, false),
            PathBuf::from("/w/")
        );
        let q = SuggestionQuery::Path { backslash: true };
        assert_eq!(
            (q.of("src\\ma"), q.of("a/b\\c"), q.of("ma")),
            ("ma".into(), "c".into(), "ma".into())
        );
        assert_eq!(SuggestionQuery::Path { backslash: false }.of("a\\b/c\\d"), "c\\d");
        let mut main = SuggestionCandidate::new("main.rs", SuggestionKind::File);
        main.query = q;
        let i = insertion(&main, &ctx("src\\ma", "src\\ma", Shell::Pwsh));
        assert_eq!((i.delete(), i.text()), (0, "in.rs"));
        let i = insertion(&main, &ctx("src\\MA", "src\\MA", Shell::Pwsh));
        assert_eq!((i.delete(), i.text()), (2, "main.rs"));
    }

    #[test]
    fn list_dir_sorted() {
        let temp_dir = TempDir::new("ls");
        let d = temp_dir.path().to_path_buf();
        for f in ["c.txt", "B.txt", ".hidden", ".DS_Store", "a.txt"] {
            std::fs::write(d.join(f), "").unwrap();
        }
        std::fs::create_dir(d.join("src")).unwrap();
        let got: Vec<String> = list_dir(&d, true, false)
            .into_iter()
            .map(|s| s.names()[0].clone())
            .collect();
        assert_eq!(got, ["a.txt", "B.txt", "c.txt", "src/", ".hidden", "../"]);
        let folders = list_dir(&d, false, false);
        assert_eq!(
            folders
                .iter()
                .map(|s| s.names()[0].as_str())
                .collect::<Vec<_>>(),
            ["src/", "../"]
        );
        assert_eq!(folders[0].template_type().as_deref(), Some("folders"));
        assert!(list_dir(&d.join("missing"), true, false).is_empty());
        std::fs::remove_dir_all(&d).unwrap();
    }

    #[cfg(unix)] // Windows file names cannot hold control chars
    #[test]
    fn list_dir_skips_control_char_names() {
        let temp_dir = TempDir::new("ctl");
        let d = temp_dir.path().to_path_buf();
        for f in ["ok", "aa\rtouch${IFS}PWNED\r", "esc\x1b[2J"] {
            std::fs::write(d.join(f), "").unwrap();
        }
        let got: Vec<String> = list_dir(&d, true, false)
            .into_iter()
            .map(|s| s.names()[0].clone())
            .collect();
        assert_eq!(got, ["ok", "../"]);
        std::fs::remove_dir_all(&d).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn first_token_sources() {
        use std::os::unix::fs::PermissionsExt;
        let temp_dir = TempDir::new("path");
        let d = temp_dir.path().to_path_buf();
        for (f, mode) in [("mytool", 0o755), ("notexec", 0o644), ("git", 0o755)] {
            std::fs::write(d.join(f), "").unwrap();
            std::fs::set_permissions(d.join(f), std::fs::Permissions::from_mode(mode)).unwrap();
        }
        let specs = vec!["git".to_string(), "aws/s3".into(), "@scope/cli".into()];
        let aliases: HashMap<String, String> =
            [("gst".to_string(), "git status".to_string())].into();
        let items = first_token_items(&specs, &aliases, d.to_str(), None);
        let got: Vec<(&str, SuggestionKind, f64)> = items
            .iter()
            .map(|s| (s.first(), s.kind, s.priority))
            .collect();
        assert_eq!(
            got,
            [
                ("git", SuggestionKind::Subcommand, 40.0),
                ("@scope/cli", SuggestionKind::Subcommand, 40.0),
                ("gst", SuggestionKind::Shortcut, 100.0),
                ("mytool", SuggestionKind::Subcommand, 40.0)
            ]
        );
        assert_eq!(items[2].description.as_deref(), Some("git status"));
        std::fs::remove_dir_all(&d).unwrap();
    }

    #[test]
    fn help_template() {
        let help = sub(&["help", "h"]);
        let root = Rc::new(
            Subcommand::default()
                .with_subcommands(vec![sub(&["add"]), help.clone(), sub(&["commit", "ci"])]),
        );
        let mut pr = pr_for(Subcommand::default(), "");
        pr.completion = help.clone();
        pr.path = vec![root, help];
        pr.annotations = vec![TokenAnnotation::new(AnnotationKind::Subcommand, "help".into(), vec![])];
        let got: Vec<String> = help_items(&ParsedCommandLine::new(pr))
            .into_iter()
            .map(|s| s.names()[0].clone())
            .collect();
        assert_eq!(got, ["add", "commit", "ci"]);
    }
}
