//! Suggestion assembly, generators (triggers, cache, templates), filtering (prefix/fuzzysort), ranking,
//! insertion text, history rows and first-token items (SPEC §6.5–6.11).
//! OWNER: engine agent (internal API; only `engine.rs` uses it). Everything here is pure (no JS); the engine
//! resolves function triggers / `getQueryTerm`s and runs generators.
use crate::completion::domain::value_objects::annotation_kind::AnnotationKind;
use crate::completion::domain::value_objects::parsed_command_line::ParsedCommandLine;
use crate::specs::domain::entities::subcommand::Subcommand;
use crate::specs::domain::value_objects::arg::Arg;
use crate::specs::domain::value_objects::generator_cache_policy::GeneratorCachePolicy;
use crate::specs::domain::value_objects::generator_trigger::GeneratorTrigger;
use crate::specs::domain::value_objects::repeatability::Repeatability;
use crate::specs::domain::value_objects::spec_suggestion::SpecSuggestion;
use crate::history::domain::entities::history_entry::HistoryEntry;
use crate::completion::domain::value_objects::suggestion::{Suggestion, SuggestionParts};
use crate::completion::domain::value_objects::generator_slot_view::GeneratorSlotView;
use crate::completion::domain::value_objects::insertion_context::InsertionContext;
use crate::completion::domain::value_objects::suggestion_candidate::SuggestionCandidate;
use crate::completion::domain::value_objects::suggestion_query::{after_sep, SuggestionQuery};
use crate::completion::domain::services::fuzzy_matcher::fuzzysort;
use crate::completion::domain::services::suggestion_filter::cmp_locale;
use crate::completion::domain::services::insertion_composer::insertion;
use crate::specs::domain::value_objects::suggestion_kind::SuggestionKind;
use serde_json::Value;
use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::rc::Rc;

fn cmp_first(a: &SuggestionCandidate, b: &SuggestionCandidate) -> Ordering {
    cmp_locale(a.first(), b.first())
}

// ---------------------------------------------------------------------------------------------------------
// Assembly (SPEC §6.7.1)

/// Options offered at this level: exclusiveOn / repeat limits / dependsOn priority, alphabetical.
fn static_options(pr: &ParsedCommandLine) -> Vec<SuggestionCandidate> {
    let passed = pr.passed_options();
    let excluded: HashSet<&str> = passed
        .iter()
        .flat_map(|o| o.exclusive_on().iter().map(String::as_str))
        .collect();
    let mut unmet: HashSet<&str> = passed
        .iter()
        .flat_map(|o| o.depends_on().iter().map(String::as_str))
        .collect();
    for o in passed {
        for n in o.names() {
            unmet.remove(n.as_str());
        }
    }
    let comp = pr.completion();
    let mut seen: HashSet<&str> = HashSet::new();
    let mut out = Vec::new();
    for o in comp
        .options()
        .iter()
        .chain(pr.persistent())
        .chain(comp.persistent())
    {
        // "unique": the same option may be reachable as local and inherited persistent
        if o.names().iter().any(|n| seen.contains(n.as_str())) {
            continue;
        }
        seen.extend(o.names().iter().map(String::as_str));
        if o.names().iter().any(|n| excluded.contains(n.as_str())) {
            continue;
        }
        let limit = match o.is_repeatable() {
            Repeatability::No | Repeatability::Times(0) => Some(1),
            Repeatability::Times(n) => Some(n as usize),
            Repeatability::Unlimited => None,
        };
        if let Some(limit) = limit {
            let used = passed
                .iter()
                .filter(|p| p.names().iter().any(|n| o.names().contains(n)))
                .count();
            if used >= limit {
                continue;
            }
        }
        let mut s = SuggestionCandidate::from_opt(o, pr.directives().option_arg_separators());
        if o.names().iter().any(|n| unmet.contains(n.as_str())) {
            s.priority = 75.0;
        }
        out.push(s);
    }
    out.sort_by(cmp_first);
    out
}

/// `-x` short name of an option row.
fn short_name(s: &SuggestionCandidate) -> Option<String> {
    s.names
        .iter()
        .find(|n| n.starts_with('-') && !n.starts_with("--") && n.chars().count() == 2)
        .cloned()
}

/// Build the unfiltered list for a parse result: subcommands, args (static + generators), additional
/// suggestions, options, then option-chain rewriting.
pub fn assemble(pr: &ParsedCommandLine, gens: &[GeneratorSlotView]) -> Vec<SuggestionCandidate> {
    let comp = pr.completion();
    let chain = pr.option_chain().as_deref();
    let mut out = Vec::new();
    if pr.suggest_subcommands() {
        let mut subs: Vec<SuggestionCandidate> = comp.subcommands().iter().map(|s| SuggestionCandidate::from_sub(s)).collect();
        subs.sort_by(cmp_first);
        out.extend(subs);
    }
    if pr.suggest_args() {
        if let Some(arg) = pr.current_arg() {
            out.extend(
                arg.suggestions()
                    .iter()
                    .map(|s| SuggestionCandidate::from_suggestion(s, arg.is_dangerous(), None, SuggestionQuery::Term)),
            );
            for g in gens {
                // SPEC §6.6.1: stale results of a loading generator only without getQueryTerm
                if !g.loading || g.query == SuggestionQuery::Term {
                    out.extend(
                        g.items
                            .iter()
                            .map(|s| SuggestionCandidate::from_suggestion(s, false, Some(g.key), g.query.clone())),
                    );
                }
            }
        }
    }
    let mut additional: Vec<SuggestionCandidate> = comp
        .additional_suggestions()
        .iter()
        // untyped rows are not `arg`s: dropped inside an option chain (only option/arg types are kept)
        .filter(|s| chain.is_none() || s.kind().is_some())
        .map(|s| SuggestionCandidate::from_suggestion(s, comp.is_dangerous(), None, SuggestionQuery::Term))
        .collect();
    additional.sort_by(cmp_first);
    out.extend(additional);
    let options = static_options(pr);
    if pr.suggest_options() {
        out.extend(options.iter().cloned());
    }
    match chain {
        Some(chain) => option_chain(out, &options, chain, pr.suggest_options()),
        None => out,
    }
}

/// SPEC §6.7.1 step 5 (typing `-la`).
fn option_chain(list: Vec<SuggestionCandidate>, options: &[SuggestionCandidate], chain: &str, options_flag: bool) -> Vec<SuggestionCandidate> {
    let Some(last) = chain.chars().last().map(|c| format!("-{c}")) else {
        return list;
    };
    let mut list: Vec<SuggestionCandidate> = list
        .into_iter()
        .filter(|s| matches!(s.kind, SuggestionKind::Option | SuggestionKind::Arg))
        .collect();
    if !options_flag {
        if let Some(o) = options
            .iter()
            .find(|o| short_name(o).as_deref() == Some(last.as_str()))
        {
            list.push(o.clone());
        }
    }
    for s in &mut list {
        if s.kind == SuggestionKind::Arg {
            s.query = SuggestionQuery::Value(String::new());
            continue;
        }
        let Some(short) = short_name(s) else { continue };
        if short == last {
            for n in &mut s.names {
                if *n == short {
                    *n = chain.to_string();
                }
            }
        } else {
            let name = format!("{chain}{}", &short[1..]);
            s.names = vec![name.clone()];
            s.insert_value = Some(name);
        }
    }
    list
}

// ---------------------------------------------------------------------------------------------------------
// History rows (SPEC §6.10)

/// Rows for history entries extending `prefix` (= buffer up to the start of the last token), newest first.
pub fn history_rows(prefix: &str, entries: &[HistoryEntry]) -> Vec<SuggestionCandidate> {
    let first_word = |s: &str| s.split(' ').next().unwrap_or("").to_string();
    let mut names: Vec<&str> = Vec::new();
    let mut freq: HashMap<String, usize> = HashMap::new();
    for e in entries.iter().rev() {
        let Some(rest) = e.command().strip_prefix(prefix) else {
            continue;
        };
        let name = rest.trim_end();
        if name.is_empty() {
            continue; // ponytail: an empty-name row can only render blank
        }
        if name.chars().any(char::is_control) {
            // ponytail: multi-line entry: typing its "\n" runs part of it, stripping it changes the command;
            // bracketed paste (DECSET 2004) could insert it whole if it is ever wanted
            continue;
        }
        let w = first_word(name);
        if !w.is_empty() {
            *freq.entry(w).or_default() += 1;
        }
        names.push(name);
    }
    let mut seen = HashSet::new();
    names
        .into_iter()
        .filter(|n| seen.insert(*n))
        .map(|n| {
            let f = freq.get(&first_word(n)).copied().unwrap_or(0);
            let mut s = SuggestionCandidate::new(n, SuggestionKind::History);
            s.description = Some("past command".into());
            s.insert_value = Some(n.to_string());
            s.priority = if f > 1 {
                75.0 + f.min(10) as f64 / 10.0
            } else {
                50.0
            };
            s
        })
        .collect()
}

/// `show` → spec rows + history rows not already named, spec rows boosted to the matching history priority;
/// otherwise (history_only / toggle) → history rows only.
pub fn mix_history(spec: Vec<SuggestionCandidate>, hist: Vec<SuggestionCandidate>, show: bool) -> Vec<SuggestionCandidate> {
    if !show {
        return hist;
    }
    let mut by_word: HashMap<&str, f64> = HashMap::new();
    for h in &hist {
        by_word.insert(h.first().split(' ').next().unwrap_or(""), h.priority);
    }
    let existing: HashSet<&str> = spec
        .iter()
        .flat_map(|s| s.names.iter().map(String::as_str))
        .collect();
    let extra: Vec<SuggestionCandidate> = hist
        .iter()
        .filter(|h| !existing.contains(h.first()))
        .cloned()
        .collect();
    let mut out: Vec<SuggestionCandidate> = spec
        .iter()
        .map(|s| {
            let mut s = s.clone();
            s.priority = s
                .names
                .iter()
                .filter_map(|n| by_word.get(n.as_str()))
                .fold(s.priority, |a, &b| a.max(b));
            s
        })
        .collect();
    out.extend(extra);
    out
}

// ---------------------------------------------------------------------------------------------------------
// Insertion text (SPEC §6.8) and items

/// `<file> [mode]...` (SPEC §7.4.2).
fn args_hint(args: &[Rc<Arg>]) -> String {
    args.iter()
        .filter_map(|a| {
            let n = a.name().as_deref().filter(|n| !n.is_empty())?;
            let n = if a.is_variadic() {
                format!("{n}...")
            } else {
                n.to_string()
            };
            Some(if a.is_optional() {
                format!("[{n}]")
            } else {
                format!("<{n}>")
            })
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// Char indexes of `title()` matching the query (SPEC §7.4.2).
fn highlight(s: &SuggestionCandidate, q: &str, fuzzy: bool) -> Vec<usize> {
    if q.is_empty() {
        return Vec::new();
    }
    let segs: Vec<&str> = match &s.display_name {
        Some(d) => vec![d.as_str()],
        None => s.names.iter().map(String::as_str).collect(),
    };
    let mut offsets = Vec::with_capacity(segs.len());
    let mut off = 0;
    for seg in &segs {
        offsets.push(off);
        off += seg.chars().count() + 2; // ", "
    }
    if fuzzy {
        let best = segs
            .iter()
            .zip(&offsets)
            .filter_map(|(seg, &o)| fuzzysort(q, seg).map(|r| (r, o)))
            .max_by_key(|((score, _), _)| *score);
        return best
            .map(|((_, idx), o)| idx.into_iter().map(|i| i + o).collect())
            .unwrap_or_default();
    }
    let (ql, qn) = (q.to_lowercase(), q.chars().count());
    segs.iter()
        .zip(&offsets)
        .filter(|(seg, _)| seg.to_lowercase().starts_with(&ql))
        .flat_map(|(_, &o)| o..o + qn)
        .collect()
}

/// Final dropdown row with its insertion precomputed.
pub fn to_item(s: SuggestionCandidate, prefix_match: bool, c: &InsertionContext) -> Suggestion {
    let query = s.query.of(c.search_term);
    Suggestion::new(SuggestionParts {
        insert: insertion(&s, c),
        highlight: highlight(&s, &query, c.fuzzy),
        args_hint: args_hint(&s.args),
        query,
        names: s.names,
        display_name: s.display_name,
        description: s.description,
        kind: s.kind,
        original_kind: s.original_kind,
        priority: s.priority,
        is_dangerous: s.is_dangerous,
        generator: s.generator,
        prefix_match,
        template_type: s.template_type,
        icon: s.icon,
    })
}

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
    use crate::history::domain::entities::history_entry::HistoryEntryPrimitives;
    use crate::completion::domain::services::suggestion_ranker::rank;
    use crate::completion::domain::test_fixtures::{ctx, names, opt, opts, pr_for, sub};
    use crate::completion::domain::value_objects::token_annotation::TokenAnnotation;
    use crate::shared::domain::value_objects::shell::Shell;
    use crate::shared::infrastructure::testing::temp_dir::TempDir;
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

    #[test]
    fn assembly_options_rules() {
        let excl = Rc::new(
            opt(&["-a"])
                .with_exclusive_on(vec!["-b".into()])
                .with_depends_on(vec!["-c".into()]),
        );
        let comp = Subcommand::default()
            .with_subcommands(vec![sub(&["zeta"]), sub(&["Alpha"]), sub(&["beta"])])
            .with_options(vec![
                excl.clone(),
                Rc::new(opt(&["-b"])),
                Rc::new(opt(&["-c"])),
                Rc::new(opt(&["-v"]).with_is_repeatable(Repeatability::Unlimited)),
            ]);
        let mut pr = pr_for(comp, "");
        pr.passed_options = vec![
            excl,
            Rc::new(opt(&["-v"]).with_is_repeatable(Repeatability::Unlimited)),
        ];
        let out = assemble(&ParsedCommandLine::new(pr), &[]);
        let got: Vec<(&str, f64)> = out.iter().map(|s| (s.first(), s.priority)).collect();
        // subcommands alphabetical; -a used once (gone), -b excluded, -c depended on → 75, -v repeatable
        assert_eq!(
            got,
            [
                ("Alpha", 0.0),
                ("beta", 0.0),
                ("zeta", 0.0),
                ("-c", 75.0),
                ("-v", 0.0)
            ]
        );
    }

    #[test]
    fn option_chain_rewriting() {
        let l = Rc::new(opt(&["-l"]));
        let comp = Subcommand::default()
            .with_options(vec![
                l.clone(),
                Rc::new(opt(&["-a", "--all"])),
                Rc::new(opt(&["-h"])),
                Rc::new(opt(&["--long"])),
            ])
            .with_subcommands(vec![sub(&["sub"])]);
        let mut pr = pr_for(comp, "-la");
        pr.passed_options = vec![l];
        pr.option_chain = Some("-la".into());
        pr.annotations = vec![TokenAnnotation::new(AnnotationKind::Option, "-la".into(), vec![])];
        let out = assemble(&ParsedCommandLine::new(pr), &[]);
        let got: Vec<Vec<String>> = out.iter().map(|s| s.names.clone()).collect();
        assert_eq!(
            got,
            [
                vec!["--long".to_string()],
                vec!["-la".into(), "--all".into()],
                vec!["-lah".into()]
            ]
        );
        assert_eq!(out[2].insert_value.as_deref(), Some("-lah"));
        assert_eq!(
            names(&rank(out, "-la", None, &opts())),
            ["!-la", "-la", "-lah"]
        );
    }

    #[test]
    fn history_rows_priority_and_mixing() {
        let e = |c: &str| HistoryEntry::new(HistoryEntryPrimitives {
            command: c.into(),
            ..Default::default()
        });
        let hist = [
            e("git checkout main"),
            e("git checkout dev"),
            e("git status"),
            e("git checkout main"),
            e("ls"),
        ];
        let rows = history_rows("git ", &hist);
        let got: Vec<(&str, f64)> = rows.iter().map(|s| (s.first(), s.priority)).collect();
        assert_eq!(
            got,
            [
                ("checkout main", 75.3),
                ("status", 50.0),
                ("checkout dev", 75.3)
            ]
        );
        assert_eq!(rows[0].insert_value.as_deref(), Some("checkout main"));
        let spec = vec![
            SuggestionCandidate::new("checkout", SuggestionKind::Subcommand),
            SuggestionCandidate::new("status", SuggestionKind::Subcommand),
        ];
        let mixed = mix_history(spec.clone(), rows.clone(), true);
        let got: Vec<(&str, f64)> = mixed.iter().map(|s| (s.first(), s.priority)).collect();
        assert_eq!(
            got,
            [
                ("checkout", 75.3),
                ("status", 50.0),
                ("checkout main", 75.3),
                ("checkout dev", 75.3)
            ]
        );
        assert_eq!(mix_history(spec, rows, false).len(), 3);
    }

    #[test]
    fn history_rows_skip_multiline_entries() {
        // history-multiline-mangled: inserting them would run (or, stripped, change) the command
        let e = |c: &str| HistoryEntry::new(HistoryEntryPrimitives {
            command: c.into(),
            ..Default::default()
        });
        let rows = history_rows(
            "git commit ",
            &[
                e("git commit -m \"Fix login\nCloses #12\""),
                e("git commit -m wip"),
            ],
        );
        assert_eq!(
            rows.iter().map(|s| s.first()).collect::<Vec<_>>(),
            ["-m wip"]
        );
    }

    #[test]
    fn item_highlight_and_hint() {
        let mut s = SuggestionCandidate::new("-m", SuggestionKind::Option);
        s.names.push("--message".into());
        s.args = vec![
            Rc::new(Arg::default().with_name(Some("msg".into()))),
            Rc::new(
                Arg::default()
                    .with_name(Some("more".into()))
                    .with_is_optional(true)
                    .with_is_variadic(true),
            ),
        ];
        let it = to_item(s, true, &ctx("--m", "--m", Shell::Bash));
        assert_eq!(it.title(), "-m, --message");
        assert_eq!(it.highlight(), [4, 5, 6]);
        assert_eq!(it.args_hint(), "<msg> [more...]");
        let s = SuggestionCandidate::new("Fuzzy Search", SuggestionKind::Arg);
        let it = to_item(
            s,
            false,
            &InsertionContext {
                fuzzy: true,
                ..ctx("fs", "fs", Shell::Bash)
            },
        );
        assert_eq!(it.highlight(), [0, 6]);
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
