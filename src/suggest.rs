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
use crate::shared::domain::services::shell_quoting;
use crate::shared::domain::value_objects::shell::Shell;
use crate::history::domain::entities::history_entry::HistoryEntry;
use crate::completion::domain::value_objects::insertion::{Insertion, InsertionParts};
use crate::completion::domain::value_objects::suggestion::{Suggestion, SuggestionParts};
use crate::completion::domain::value_objects::filter_options::FilterOptions;
use crate::completion::domain::value_objects::generator_slot_view::GeneratorSlotView;
use crate::completion::domain::value_objects::insertion_context::InsertionContext;
use crate::completion::domain::value_objects::suggestion_candidate::SuggestionCandidate;
use crate::completion::domain::value_objects::suggestion_query::{after_sep, SuggestionQuery};
use crate::specs::domain::value_objects::suggestion_kind::SuggestionKind;
use serde_json::Value;
use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::rc::Rc;

/// ICU `localeCompare` approximation: lowercase compare, ties by exact compare (SPEC §6.7.1).
pub fn cmp_locale(a: &str, b: &str) -> Ordering {
    a.to_lowercase()
        .cmp(&b.to_lowercase())
        .then_with(|| a.cmp(b))
}

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
// Ranking and filtering (SPEC §6.7.3–6.7.5)

/// Recency boost; `recency` = `recency[cmd]`, `None` with `sortMethod = alphabetical` (SPEC §6.7.5).
pub fn update_priorities(list: &mut [SuggestionCandidate], recency: Option<&HashMap<String, u64>>) {
    for s in list {
        let name = s.first();
        let rec = recency
            .filter(|_| name != "../")
            .and_then(|m| m.get(name))
            .copied()
            .filter(|&ms| ms != 0);
        let boost = rec.map_or(0.0, |ms| ms as f64 / 1e13);
        let mut p = if s.priority == 0.0 { 50.0 } else { s.priority }; // 0 is treated as unset, i.e. 50
        if s.kind != SuggestionKind::AutoExecute {
            p = p.clamp(0.0, 100.0);
        }
        s.priority = if rec.is_some() && (50.0..=75.0).contains(&p) {
            75.0 + boost
        } else {
            p + boost
        };
    }
}

/// fuzzysort `single(q, t)` (fresh struct per call): `(score, matched char indexes)`; higher is better, 0 = exact.
pub fn fuzzysort(q: &str, t: &str) -> Option<(i64, Vec<usize>)> {
    if q.is_empty() || t.is_empty() {
        return None;
    }
    let lower = |c: char| c.to_lowercase().next().unwrap_or(c);
    let ql: Vec<char> = q.chars().map(lower).collect();
    let tc: Vec<char> = t.chars().collect();
    let tl: Vec<char> = tc.iter().map(|&c| lower(c)).collect();
    let (qn, tn) = (ql.len(), tl.len());
    // simple pass: query chars in order
    let mut simple = Vec::with_capacity(qn);
    for (ti, &c) in tl.iter().enumerate() {
        if c == ql[simple.len()] {
            simple.push(ti);
            if simple.len() == qn {
                break;
            }
        }
    }
    if simple.len() < qn {
        return None;
    }
    // beginnings: upper after non-upper, first alnum after non-alnum, any non-alnum (ASCII, original case)
    let mut beginning = vec![false; tn];
    let (mut was_upper, mut was_alnum) = (false, false);
    for (i, &c) in tc.iter().enumerate() {
        let (upper, alnum) = (c.is_ascii_uppercase(), c.is_ascii_alphanumeric());
        beginning[i] = (upper && !was_upper) || !was_alnum || !alnum;
        was_upper = upper;
        was_alnum = alnum;
    }
    let mut next = vec![tn; tn];
    let mut nb = tn;
    for i in (0..tn).rev() {
        next[i] = nb;
        if beginning[i] {
            nb = i;
        }
    }
    // strict pass: only consecutive matches or word beginnings, with backtracking
    let mut strict: Vec<usize> = Vec::with_capacity(qn);
    let mut ok = false;
    let mut ti = if simple[0] == 0 {
        0
    } else {
        next[simple[0] - 1]
    };
    if ti != tn {
        let mut qi = 0;
        loop {
            if ti >= tn {
                if qi == 0 {
                    break;
                }
                qi -= 1;
                let last = strict.pop().unwrap_or(0);
                ti = next[last];
            } else if ql[qi] == tl[ti] {
                strict.push(ti);
                qi += 1;
                if qi == qn {
                    ok = true;
                    break;
                }
                ti += 1;
            } else {
                ti = next[ti];
            }
        }
    }
    let best = if ok { strict } else { simple };
    let mut score: i64 = 0;
    let mut last: i64 = -1;
    for &i in &best {
        let i = i as i64;
        if last != i - 1 {
            score -= i;
        }
        last = i;
    }
    if !ok {
        score *= 1000;
    }
    score -= (tn - qn) as i64;
    Some((score, best))
}

/// Prefix ladder: 0 exact, −1 exact ignoring case, −2 prefix, −3 prefix ignoring case.
pub fn prefix_score(name: &str, q: &str) -> Option<i32> {
    let (nl, ql) = (name.to_lowercase(), q.to_lowercase());
    if name == q {
        Some(0)
    } else if nl == ql {
        Some(-1)
    } else if name.starts_with(q) {
        Some(-2)
    } else if nl.starts_with(&ql) {
        Some(-3)
    } else {
        None
    }
}

/// `getPartialMatch`: `(score, matched by prefix)`. Fuzzy hits that also prefix-match score `prefix / 10`.
pub fn partial(names: &[String], q: &str, fuzzy: bool) -> Option<(f64, bool)> {
    let prefix = names.iter().filter_map(|n| prefix_score(n, q)).max();
    if !fuzzy {
        return prefix.map(|p| (p as f64, true));
    }
    let f = if q.is_empty() {
        Some(0)
    } else {
        names
            .iter()
            .filter_map(|n| fuzzysort(q, n).map(|r| r.0))
            .max()
    };
    f.map(|f| match prefix {
        Some(p) => (p as f64 / 10.0, true),
        None => (f as f64, false),
    })
}

fn auto_exec(name: &str, description: &str, original: Option<SuggestionKind>) -> SuggestionCandidate {
    let mut s = SuggestionCandidate::new(name, SuggestionKind::AutoExecute);
    s.original_kind = original;
    s.insert_value = Some("\n".into());
    s.description = Some(description.into());
    s
}

/// `filterSuggestions`: exact (non-folder A, folder B) then partial (C by score, priority). Input must be
/// priority-sorted. Returns `(row, prefix_match)`.
pub fn filter(list: Vec<SuggestionCandidate>, search_term: &str, o: &FilterOptions) -> Vec<(SuggestionCandidate, bool)> {
    if search_term.is_empty() {
        // nothing typed: nothing prefix-matched, so no spec ghost (SPEC §6.11)
        let mut out: Vec<(SuggestionCandidate, bool)> = list
            .into_iter()
            .filter(|s| !s.hidden)
            .map(|s| (s, false))
            .collect();
        if o.exec_after_space && !out.is_empty() && !o.auto_exec_hidden {
            out.insert(0, (auto_exec("↪", "Immediately execute", None), false));
        }
        return out;
    }
    let mut a: Vec<(SuggestionCandidate, bool)> = Vec::new();
    let mut b: Vec<(SuggestionCandidate, bool)> = Vec::new();
    let mut c: Vec<(SuggestionCandidate, f64, bool)> = Vec::new();
    let mut added = false;
    for s in list {
        let q = s.query.of(search_term);
        let folder = s.kind == SuggestionKind::Folder;
        let qx = if folder { format!("{q}/") } else { q.clone() }.to_lowercase();
        if let Some(exact) = s.names.iter().find(|n| n.to_lowercase() == qx).cloned() {
            let plain = s.names.first().is_none_or(|n| n.is_empty())
                || s.insert_value.is_none()
                || s.names.first() == s.insert_value.as_ref();
            if !added
                && !o.auto_exec_hidden
                && plain
                && (!s.is_dangerous || o.run_dangerous)
                && s.args.first().is_none_or(|a| a.is_optional())
            {
                let mut x = s.clone();
                x.kind = SuggestionKind::AutoExecute;
                x.original_kind = Some(s.kind);
                if folder {
                    x.names = vec![exact.strip_suffix('/').unwrap_or(&exact).to_string()];
                    x.description = Some("folder".into());
                }
                x.insert_value = Some("\n".into());
                a.push((x, false));
                added = true;
            }
            if folder { &mut b } else { &mut a }.push((s.clone(), true));
        } else if !s.hidden {
            if let Some((score, p)) = partial(&s.names, &q, o.fuzzy) {
                c.push((s.clone(), score, p));
            }
        }
        if let Some(d) = &s.display_name {
            if let Some((score, p)) = partial(std::slice::from_ref(d), &q, o.fuzzy) {
                c.push((s, score, p)); // possible duplicate, removed by dedupe (≤ 50)
            }
        }
    }
    c.sort_by(|x, y| {
        y.1.partial_cmp(&x.1).unwrap_or(Ordering::Equal).then(
            y.0.priority
                .partial_cmp(&x.0.priority)
                .unwrap_or(Ordering::Equal),
        )
    });
    let mut out: Vec<(SuggestionCandidate, bool)> = a
        .into_iter()
        .chain(b)
        .chain(c.into_iter().map(|(s, _, p)| (s, p)))
        .collect();
    if !out.is_empty() && !added && !o.auto_exec_hidden {
        let folders = search_term.ends_with('/')
            && out
                .iter()
                .any(|(s, _)| s.template_type.as_deref() == Some("folders"));
        if search_term == "." || folders {
            let name = if search_term == "." { "." } else { "↪" };
            out.insert(
                0,
                (
                    auto_exec(name, "Enter the current directory", Some(SuggestionKind::Folder)),
                    false,
                ),
            );
        } else if o.suggest_current_token {
            out.insert(
                0,
                (
                    auto_exec(search_term, "Enter the current argument", None),
                    false,
                ),
            );
        }
    }
    out
}

fn same_row(a: &SuggestionCandidate, b: &SuggestionCandidate) -> bool {
    // ponytail: args compared by identity (not deep-compared); distinct specs never share names anyway
    a.names == b.names
        && a.insert_value == b.insert_value
        && a.display_name == b.display_name
        && a.args.len() == b.args.len()
        && a.args.iter().zip(&b.args).all(|(x, y)| Rc::ptr_eq(x, y))
}

/// Keep the first of rows equal in (names, insertValue, displayName, args).
pub fn dedupe(list: Vec<(SuggestionCandidate, bool)>) -> Vec<(SuggestionCandidate, bool)> {
    let mut out: Vec<(SuggestionCandidate, bool)> = Vec::with_capacity(list.len());
    for x in list {
        if !out.iter().any(|y| same_row(&x.0, &y.0)) {
            out.push(x);
        }
    }
    out
}

/// SPEC §6.7.5: recency priorities → stable sort by priority → filter → dedupe when 0 < n ≤ 50.
pub fn rank(
    mut list: Vec<SuggestionCandidate>,
    search_term: &str,
    recency: Option<&HashMap<String, u64>>,
    o: &FilterOptions,
) -> Vec<(SuggestionCandidate, bool)> {
    update_priorities(&mut list, recency);
    list.sort_by(|a, b| {
        b.priority
            .partial_cmp(&a.priority)
            .unwrap_or(Ordering::Equal)
    });
    let out = filter(list, search_term, o);
    if !out.is_empty() && out.len() <= 50 {
        dedupe(out)
    } else {
        out
    }
}

// ---------------------------------------------------------------------------------------------------------
// Insertion text (SPEC §6.8) and items

/// `getNameMatch`: which alias to insert.
fn name_match(s: &SuggestionCandidate, q: &str, fuzzy: bool, verbose: bool) -> Option<String> {
    let pick = |names: Vec<&String>| -> Option<String> {
        if verbose && matches!(s.kind, SuggestionKind::Option | SuggestionKind::Subcommand) {
            let mut best: Option<&String> = None;
            for n in names {
                if n.chars().count() > best.map_or(0, |b| b.chars().count()) {
                    best = Some(n);
                }
            }
            best.cloned()
        } else {
            names.first().map(|n| n.to_string())
        }
    };
    let matched: Vec<&String> = s
        .names
        .iter()
        .filter(|n| {
            if fuzzy {
                q.is_empty() || fuzzysort(q, n).is_some()
            } else {
                prefix_score(n, q).is_some()
            }
        })
        .collect();
    if !matched.is_empty() || s.display_name.is_none() {
        pick(matched)
    } else {
        pick(s.names.iter().collect())
    }
}

/// A bare variable reference (`$HOME`, pwsh `$env:PATH`): shell syntax the row means to insert.
fn is_var_ref(s: &str) -> bool {
    s.strip_prefix('$').is_some_and(|v| {
        !v.is_empty()
            && v.chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == ':')
    })
}

/// POSIX/fish escaping of a name. `quote`: the user started the token with a quote → keep quoting; `path`: a
/// file/folder row (its `$` is never a variable). An allow-list, since a deny-list
/// misses `$ ` ; { }` and a leading `~`/`=` (SPEC §6.8 D-note).
fn escape_posix(s: &str, folder: bool, quote: bool, path: bool, shell: Shell) -> String {
    let safe = |c: char| c.is_alphanumeric() || "-_./+,:@%=~".contains(c);
    // A file name's `~` expands at the start and after `=`/`:` (assignments, zsh MAGIC_EQUAL_SUBST); `=cmd` in
    // zsh. A spec row (`cd ~`) means that expansion.
    let tilde = path && (s.starts_with(['~', '=', '%']) || s.contains("=~") || s.contains(":~"));
    if !quote && !tilde {
        if s.chars().all(safe) || (!path && is_var_ref(s)) {
            return s.to_string();
        }
        if s.chars().all(|c| safe(c) || c == ' ') {
            return s.replace(' ', "\\ ");
        }
    }
    match s.strip_suffix('/').filter(|_| folder) {
        Some(stem) => format!("{}/", shell_quoting::quote(shell, stem)),
        None => shell_quoting::quote(shell, s),
    }
}

/// PowerShell escaping: single quotes when needed. Folders become backtick-escaped bare words instead, so the
/// cursor never sits inside a quote (the mid-word guard would block the next segment).
fn escape_pwsh(s: &str, folder: bool, quote: bool, path: bool) -> String {
    // PowerShell also delimits strings with the Unicode single (‘ ’ ‚ ‛) and double (“ ” „) quotes
    const SPECIAL: &str =
        "$`'\"(){};,&|@#<>\u{2018}\u{2019}\u{201A}\u{201B}\u{201C}\u{201D}\u{201E}";
    let special = |c: char| c.is_whitespace() || SPECIAL.contains(c);
    if !quote && (!s.chars().any(special) || (!path && is_var_ref(s))) {
        return s.to_string();
    }
    if folder {
        return s
            .chars()
            .flat_map(|c| ['`', c].into_iter().skip(usize::from(!special(c))))
            .collect();
    }
    shell_quoting::quote(Shell::Pwsh, s)
}

/// `(char index, char)`.
type At = (usize, char);

/// Each logical char of `raw` (quotes removed, escapes resolved) with the char index it starts at, and the quote
/// `raw` leaves open (`'` or `"`) with its index.
fn unquote(raw: &[char], shell: Shell) -> (Vec<At>, Option<(char, usize)>) {
    let pwsh = shell == Shell::Pwsh;
    let esc = if pwsh { '`' } else { '\\' };
    let (mut out, mut open, mut i) = (Vec::new(), None, 0);
    while i < raw.len() {
        let ch = raw[i];
        match open {
            // no escapes inside single quotes, except pwsh `''`
            Some(('\'', _)) if ch == '\'' && pwsh && raw.get(i + 1) == Some(&'\'') => {
                out.push((i, ch));
                i += 1;
            }
            Some(('\'', _)) if ch == '\'' => open = None,
            Some(('\'', _)) => out.push((i, ch)),
            _ if ch == esc && i + 1 < raw.len() => {
                out.push((i, raw[i + 1]));
                i += 1;
            }
            Some((q, _)) if ch == q => open = None,
            None if ch == '\'' || ch == '"' => open = Some((ch, i)),
            _ => out.push((i, ch)),
        }
        i += 1;
    }
    (out, open)
}

/// Escaping inside a quote the user opened before the replaced text (no quotes added around it).
fn escape_in_quote(s: &str, q: char, shell: Shell) -> String {
    match (q, shell) {
        ('\'', Shell::Pwsh) => {
            let q = shell_quoting::quote(Shell::Pwsh, s);
            q[1..q.len() - 1].to_string() // the escaping without the `'…'` around it
        }
        ('\'', Shell::Fish) => s.replace('\\', "\\\\").replace('\'', "\\'"),
        ('\'', _) => s.replace('\'', "'\\''"),
        _ => {
            let (special, esc) = match shell {
                Shell::Pwsh => ("`\"$\u{201C}\u{201D}\u{201E}", '`'),
                Shell::Fish => ("\\\"$", '\\'), // fish double quotes know only \\ \" \$
                _ => ("\\\"$`", '\\'),
            };
            s.chars()
                .map(|c| match c {
                    // interactive bash/zsh history-expand `!` even inside double quotes: single-quote it in between
                    '!' if matches!(shell, Shell::Bash | Shell::Zsh) => "\"'!'\"".to_string(),
                    c if special.contains(c) => format!("{esc}{c}"),
                    c => c.to_string(),
                })
                .collect()
        }
    }
}

/// Precomputed full insertion for a row (SPEC §6.8, D22/D25).
pub fn insertion(s: &SuggestionCandidate, c: &InsertionContext) -> Insertion {
    let q = s.query.of(c.search_term);
    let raw: Vec<char> = c.raw.chars().collect();
    let (logical, open) = unquote(&raw, c.shell);
    // special (`help` template) rows replace the typed prefix like any other row
    let delete = if s.kind == SuggestionKind::AutoExecute {
        0
    } else if q == c.search_term {
        raw.len()
    } else if let SuggestionQuery::Str(g) = &s.query {
        c.raw[c.raw.rfind(g.as_str()).map_or(0, |i| i + g.len())..]
            .chars()
            .count()
    } else if let SuggestionQuery::Path { backslash } = s.query {
        c.raw[after_sep(c.raw, backslash)..].chars().count()
    } else {
        // function getQueryTerm: delete the raw chars that spell q's logical chars (quotes/escapes included);
        // otherwise an approximation (escaped whitespace), never past the token
        let n = q.chars().count();
        let tail = logical
            .get(logical.len().saturating_sub(n)..)
            .filter(|t| t.len() == n && t.iter().map(|x| x.1).eq(q.chars()));
        match tail {
            Some(t) => raw.len() - t.first().map_or(raw.len(), |x| x.0),
            None => q
                .chars()
                .map(|ch| if ch.is_whitespace() { 2 } else { 1 })
                .sum::<usize>()
                .min(raw.len()),
        }
    };
    let deleted: String = raw[raw.len() - delete..].iter().collect();
    let quote = deleted.starts_with(['\'', '"']);
    // a quote opened before the replaced part (`cat "my dir/x`): escape for inside it
    let inside = open
        .filter(|&(_, i)| i < raw.len() - delete)
        .map(|(q, _)| q);
    let (folder, path) = (
        s.kind == SuggestionKind::Folder,
        matches!(s.kind, SuggestionKind::File | SuggestionKind::Folder),
    );
    let mut text = match &s.insert_value {
        Some(v) if !path => v.clone(),
        _ => match name_match(s, &q, c.fuzzy, c.prefer_verbose).filter(|m| !m.is_empty()) {
            None => String::new(),
            Some(m) => {
                let name = match (inside, c.shell) {
                    // a file closes the quote; a folder leaves it open for the next segment
                    (Some(qc), sh) => {
                        let mut t = escape_in_quote(&m, qc, sh);
                        t.extend((s.kind == SuggestionKind::File).then_some(qc));
                        t
                    }
                    (None, Shell::Pwsh) => escape_pwsh(&m, folder, quote, path),
                    (None, sh) => escape_posix(&m, folder, quote, path, sh),
                };
                // only the name is escaped: `--out=` keeps its separator and the cursor outside any quote
                match &s.separator {
                    Some(sep) => format!("{name}{sep}{{cursor}}"),
                    None => name,
                }
            }
        },
    };
    if text.ends_with('\n') {
        let n = text.trim_end_matches('\n').len();
        text.truncate(n);
        text.push('\n');
    }
    if !text.ends_with('\n') && s.should_add_space && c.insert_space {
        text.push(' ');
    }
    let mut delete = delete;
    // don't retype what is already typed (only when the deleted raw chars are exactly the query)
    if !q.is_empty() && deleted == q && text.starts_with(&q) {
        delete = 0; // the deleted chars are exactly q
        text.drain(..q.len());
    }
    let execute = text.ends_with('\n');
    if execute {
        text.pop();
    }
    // Names from directories, generators and specs reach the PTY as typed bytes: a CR/LF/ESC/... in them would
    // run or rewrite the line. Only the trailing "\n" (→ `execute`) is meaningful.
    text.retain(|ch| !ch.is_control());
    let mut cursor_back = 0;
    if let Some(i) = text.find("{cursor}") {
        text.replace_range(i..i + "{cursor}".len(), "");
        cursor_back = text[i..].chars().count();
    }
    Insertion::new(InsertionParts {
        delete,
        text,
        cursor_back,
        execute,
    })
}

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
    use crate::completion::domain::value_objects::parsed_command_line::ParsedCommandLineParts;
    use crate::completion::domain::value_objects::token_annotation::TokenAnnotation;
    use crate::shared::infrastructure::testing::temp_dir::TempDir;
    use crate::specs::domain::value_objects::parser_directives::ParserDirectives;
    use crate::specs::domain::value_objects::spec_option::SpecOption;
    use serde_json::json;

    fn pr_for(comp: Subcommand, search: &str) -> ParsedCommandLineParts {
        let comp = Rc::new(comp);
        ParsedCommandLineParts {
            completion: comp.clone(),
            path: vec![comp],
            current_arg: None,
            search_term: search.into(),
            suggest_subcommands: true,
            suggest_options: true,
            suggest_args: true,
            passed_options: vec![],
            persistent: vec![],
            annotations: vec![],
            command_index: 0,
            tokens: vec![],
            raw_last_token: search.into(),
            directives: ParserDirectives::default(),
            option_chain: None,
            first_token: false,
            spec_name: "cmd".into(),
        }
    }

    fn opts() -> FilterOptions {
        FilterOptions {
            fuzzy: false,
            auto_exec_hidden: false,
            exec_after_space: false,
            run_dangerous: false,
            suggest_current_token: false,
        }
    }

    fn sub(names: &[&str]) -> Rc<Subcommand> {
        Rc::new(Subcommand::default().with_names(names.iter().map(|s| s.to_string()).collect()))
    }

    fn opt(names: &[&str]) -> SpecOption {
        SpecOption::default().with_names(names.iter().map(|s| s.to_string()).collect())
    }

    fn arg(optional: bool) -> Rc<Arg> {
        Rc::new(Arg::default().with_name(Some("x".into())).with_is_optional(optional))
    }

    fn names(v: &[(SuggestionCandidate, bool)]) -> Vec<String> {
        v.iter()
            .map(|(s, _)| {
                format!(
                    "{}{}",
                    if s.kind == SuggestionKind::AutoExecute {
                        "!"
                    } else {
                        ""
                    },
                    s.first()
                )
            })
            .collect()
    }

    fn ctx<'a>(search: &'a str, raw: &'a str, shell: Shell) -> InsertionContext<'a> {
        InsertionContext {
            search_term: search,
            raw,
            shell,
            fuzzy: false,
            prefer_verbose: false,
            insert_space: true,
        }
    }

    #[test]
    fn fuzzysort_vectors() {
        assert_eq!(fuzzysort("fs", "Fuzzy Search"), Some((-16, vec![0, 6])));
        assert_eq!(fuzzysort("test", "test").map(|r| r.0), Some(0));
        assert_eq!(fuzzysort("doesnt exist", "target"), None);
        assert_eq!(fuzzysort("", "x"), None);
        // no strict match (not at a word beginning) → simple indexes, ×1000
        assert_eq!(fuzzysort("ut", "output"), Some((-1004, vec![1, 2])));
    }

    #[test]
    fn prefix_ladder() {
        assert_eq!(prefix_score("git", "git"), Some(0));
        assert_eq!(prefix_score("Git", "git"), Some(-1));
        assert_eq!(prefix_score("gitk", "git"), Some(-2));
        assert_eq!(prefix_score("Gitk", "git"), Some(-3));
        assert_eq!(prefix_score("hub", "git"), None);
        assert_eq!(
            partial(&["checkout".into()], "ch", true),
            Some((-0.2, true))
        );
        assert_eq!(
            partial(&["cherry-pick".into()], "cp", true).map(|r| r.1),
            Some(false)
        );
    }

    #[test]
    fn bucket_order_and_auto_execute() {
        let file = SuggestionCandidate::new("foo", SuggestionKind::File);
        let folder = SuggestionCandidate::new("foo/", SuggestionKind::Folder);
        let partial_row = SuggestionCandidate::new("foobar", SuggestionKind::Arg);
        let list = vec![partial_row, folder, file];
        assert_eq!(
            names(&filter(list.clone(), "foo", &opts())),
            ["!foo", "foo", "foo/", "foobar"]
        );
        // hidden setting
        let o = FilterOptions {
            auto_exec_hidden: true,
            ..opts()
        };
        assert_eq!(names(&filter(list, "foo", &o)), ["foo", "foo/", "foobar"]);
        // dangerous / mandatory arg → no auto-execute row; dangerous allowed by setting
        let mut d = SuggestionCandidate::new("rm", SuggestionKind::Subcommand);
        d.is_dangerous = true;
        assert_eq!(names(&filter(vec![d.clone()], "rm", &opts())), ["rm"]);
        let o = FilterOptions {
            run_dangerous: true,
            ..opts()
        };
        assert_eq!(names(&filter(vec![d], "rm", &o)), ["!rm", "rm"]);
        let mut m = SuggestionCandidate::new("commit", SuggestionKind::Subcommand);
        m.args = vec![arg(false)];
        assert_eq!(
            names(&filter(vec![m.clone()], "commit", &opts())),
            ["commit"]
        );
        m.args = vec![arg(true)];
        assert_eq!(
            names(&filter(vec![m], "commit", &opts())),
            ["!commit", "commit"]
        );
        // insertValue differing from the name → no auto-execute
        let mut iv = SuggestionCandidate::new("x", SuggestionKind::Arg);
        iv.insert_value = Some("y".into());
        assert_eq!(names(&filter(vec![iv], "x", &opts())), ["x"]);
        // folder exact match: auto-execute named without "/"
        let out = filter(vec![SuggestionCandidate::new("src/", SuggestionKind::Folder)], "src", &opts());
        assert_eq!(names(&out), ["!src", "src/"]);
        assert_eq!(out[0].0.description.as_deref(), Some("folder"));
    }

    #[test]
    fn partial_sorted_by_score_then_priority() {
        let mut a = SuggestionCandidate::new("CHeck", SuggestionKind::Arg);
        a.priority = 90.0;
        let mut b = SuggestionCandidate::new("check", SuggestionKind::Arg);
        b.priority = 10.0;
        let mut c = SuggestionCandidate::new("chmod", SuggestionKind::Arg);
        c.priority = 80.0;
        // check/chmod tie on score −2 → priority decides; CHeck is −3
        assert_eq!(
            names(&filter(vec![a, b, c], "ch", &opts())),
            ["chmod", "check", "CHeck"]
        );
    }

    #[test]
    fn empty_search_term_is_no_prefix_match() {
        // spec-ghost-on-empty-token: `git ` must not ghost the alphabetically first subcommand
        let subs = || {
            vec![
                SuggestionCandidate::new("abort", SuggestionKind::Subcommand),
                SuggestionCandidate::new("add", SuggestionKind::Subcommand),
            ]
        };
        let ghost = |search: &str| {
            let items: Vec<Suggestion> = filter(subs(), search, &opts())
                .into_iter()
                .map(|(s, p)| to_item(s, p, &ctx(search, search, Shell::Zsh)))
                .collect();
            crate::inline::spec_ghost(&items)
        };
        assert_eq!(ghost(""), None);
        assert_eq!(ghost("ab").as_deref(), Some("ort"));
    }

    #[test]
    fn directory_and_current_token_rows() {
        let row = |s: &str| {
            let mut x = SuggestionCandidate::new(s, SuggestionKind::Folder);
            x.template_type = Some("folders".into());
            x.query = SuggestionQuery::Str("/".into());
            x
        };
        let out = filter(vec![row("../"), row("./x/")], ".", &opts());
        assert_eq!(out[0].0.first(), ".");
        assert_eq!(out[0].0.original_kind, Some(SuggestionKind::Folder));
        let out = filter(vec![row("lib/")], "src/", &opts());
        assert_eq!(names(&out), ["!↪", "lib/"]);
        let o = FilterOptions {
            suggest_current_token: true,
            ..opts()
        };
        let out = filter(vec![SuggestionCandidate::new("main", SuggestionKind::Arg)], "ma", &o);
        assert_eq!(names(&out), ["!ma", "main"]);
        assert_eq!(
            out[0].0.description.as_deref(),
            Some("Enter the current argument")
        );
        let o = FilterOptions {
            exec_after_space: true,
            ..opts()
        };
        assert_eq!(
            names(&filter(vec![SuggestionCandidate::new("a", SuggestionKind::Arg)], "", &o)),
            ["!↪", "a"]
        );
    }

    #[test]
    fn dedupe_only_up_to_50() {
        let mut d = SuggestionCandidate::new("dup", SuggestionKind::Arg);
        d.display_name = Some("dup".into());
        // name and display name both match → pushed twice, deduped
        let out = rank(vec![d.clone()], "du", None, &opts());
        assert_eq!(names(&out), ["dup"]);
        let many: Vec<SuggestionCandidate> = (0..30).map(|_| d.clone()).collect();
        assert_eq!(rank(many, "du", None, &opts()).len(), 60);
    }

    #[test]
    fn recency_bands_and_priority_zero() {
        let rec: HashMap<String, u64> = [
            ("a".to_string(), 1_700_000_000_000u64),
            ("b".into(), 1_700_000_000_000),
            ("../".into(), 1),
        ]
        .into();
        let mut list = vec![
            SuggestionCandidate::new("a", SuggestionKind::Arg),
            SuggestionCandidate::new("b", SuggestionKind::Arg),
            SuggestionCandidate::new("c", SuggestionKind::Arg),
            SuggestionCandidate::new("../", SuggestionKind::Folder),
        ];
        list[1].priority = 80.0;
        list[2].priority = 0.0;
        update_priorities(&mut list, Some(&rec));
        assert!((list[0].priority - 75.17).abs() < 1e-9);
        assert!((list[1].priority - 80.17).abs() < 1e-9);
        assert_eq!(list[2].priority, 50.0);
        assert_eq!(list[3].priority, 50.0);
        // alphabetical → no recency map
        let mut list = vec![SuggestionCandidate::new("a", SuggestionKind::Arg)];
        update_priorities(&mut list, None);
        assert_eq!(list[0].priority, 50.0);
        // auto-execute is not clamped
        let mut list = vec![SuggestionCandidate::new("↪", SuggestionKind::AutoExecute)];
        list[0].priority = 150.0;
        update_priorities(&mut list, None);
        assert_eq!(list[0].priority, 150.0);
    }

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
    fn insertion_examples() {
        let mut status = SuggestionCandidate::new("status", SuggestionKind::Subcommand);
        status.should_add_space = true;
        let i = insertion(&status, &ctx("st", "st", Shell::Bash));
        assert_eq!((i.delete(), i.text()), (0, "atus "));
        let mut checkout = SuggestionCandidate::new("checkout", SuggestionKind::Subcommand);
        checkout.should_add_space = true;
        assert_eq!(
            insertion(&checkout, &ctx("ch", "ch", Shell::Zsh)).text(),
            "eckout "
        );
        // case differs → retype the whole token
        let i = insertion(&checkout, &ctx("CH", "CH", Shell::Zsh));
        assert_eq!((i.delete(), i.text()), (2, "checkout "));
        // requiresSeparator + mandatory arg: `--out= ` with the cursor right after `=`
        let mut out = SuggestionCandidate::new("--out", SuggestionKind::Option);
        out.separator = Some("=".into());
        out.should_add_space = true;
        let i = insertion(&out, &ctx("", "", Shell::Bash));
        assert_eq!(
            i,
            Insertion::new(InsertionParts {
                delete: 0,
                text: "--out= ".into(),
                cursor_back: 1,
                execute: false
            })
        );
        let i = insertion(&out, &ctx("--o", "--o", Shell::Bash));
        assert_eq!(
            i,
            Insertion::new(InsertionParts {
                delete: 0,
                text: "ut= ".into(),
                cursor_back: 1,
                execute: false
            })
        );
        // auto-space off
        let c = InsertionContext {
            insert_space: false,
            ..ctx("ch", "ch", Shell::Zsh)
        };
        assert_eq!(insertion(&checkout, &c).text(), "eckout");
        // escaped space in the raw token
        let mut dir = SuggestionCandidate::new("my dir/", SuggestionKind::Folder);
        dir.insert_value = Some("my dir/".into());
        dir.query = SuggestionQuery::Str("/".into());
        let i = insertion(&dir, &ctx("my di", "my\\ di", Shell::Bash));
        assert_eq!((i.delete(), i.text()), (6, "my\\ dir/"));
        // quoted raw token
        let mut file = SuggestionCandidate::new("my file", SuggestionKind::File);
        file.query = SuggestionQuery::Str("/".into());
        let i = insertion(&file, &ctx("my fi", "'my fi", Shell::Bash));
        assert_eq!((i.delete(), i.text()), (6, "'my file'"));
        // getQueryTerm "/": only the part after the last "/" is deleted / not retyped
        let mut main = SuggestionCandidate::new("main.rs", SuggestionKind::File);
        main.query = SuggestionQuery::Str("/".into());
        let i = insertion(&main, &ctx("src/ma", "src/ma", Shell::Bash));
        assert_eq!((i.delete(), i.text()), (0, "in.rs"));
        // POSIX special chars → single quotes, embedded quote escaped
        let i = insertion(
            &SuggestionCandidate::new("it's", SuggestionKind::File),
            &ctx("", "", Shell::Bash),
        );
        assert_eq!(i.text(), "'it'\\''s'");
        let i = insertion(
            &SuggestionCandidate::new("a(b)/", SuggestionKind::Folder),
            &ctx("", "", Shell::Bash),
        );
        assert_eq!(i.text(), "'a(b)'/");
        // pwsh
        let i = insertion(
            &SuggestionCandidate::new("my file", SuggestionKind::File),
            &ctx("", "", Shell::Pwsh),
        );
        assert_eq!(i.text(), "'my file'");
        let i = insertion(
            &SuggestionCandidate::new("it's", SuggestionKind::File),
            &ctx("", "", Shell::Pwsh),
        );
        assert_eq!(i.text(), "'it''s'");
        let i = insertion(
            &SuggestionCandidate::new("my dir/", SuggestionKind::Folder),
            &ctx("", "", Shell::Pwsh),
        );
        assert_eq!((i.text(), i.cursor_back()), ("my` dir/", 0));
        let i = insertion(&out, &ctx("", "", Shell::Pwsh));
        assert_eq!((i.text(), i.cursor_back()), ("--out= ", 1));
        // insertValue verbatim with {cursor}; "\n" → execute; auto-execute deletes nothing
        let mut iv = SuggestionCandidate::new("x", SuggestionKind::Arg);
        iv.insert_value = Some("a{cursor}b\n\n".into());
        assert_eq!(
            insertion(&iv, &ctx("x", "x", Shell::Bash)),
            Insertion::new(InsertionParts {
                delete: 1,
                text: "ab".into(),
                cursor_back: 1,
                execute: true
            })
        );
        let ae = auto_exec("↪", "", None);
        assert_eq!(
            insertion(&ae, &ctx("x", "x", Shell::Bash)),
            Insertion::new(InsertionParts {
                delete: 0,
                text: String::new(),
                cursor_back: 0,
                execute: true
            })
        );
        // option chain arg rows: query "" → nothing deleted
        let mut a = SuggestionCandidate::new("val", SuggestionKind::Arg);
        a.query = SuggestionQuery::Value(String::new());
        assert_eq!(insertion(&a, &ctx("-la", "-la", Shell::Bash)).delete(), 0);
        // preferVerboseSuggestions
        let o = SuggestionCandidate {
            names: vec!["-m".into(), "--message".into()],
            ..SuggestionCandidate::new("", SuggestionKind::Option)
        };
        assert_eq!(insertion(&o, &ctx("", "", Shell::Bash)).text(), "-m");
        let c = InsertionContext {
            prefer_verbose: true,
            ..ctx("", "", Shell::Bash)
        };
        assert_eq!(insertion(&o, &c).text(), "--message");
        let i = insertion(&o, &ctx("--m", "--m", Shell::Bash));
        assert_eq!((i.delete(), i.text()), (0, "essage"));
    }

    #[test]
    fn insertion_review_regressions() {
        // composite-optarg-delete: the parser hands over only the value (`al` of `--color=al`)
        let always = SuggestionCandidate::new("always", SuggestionKind::Arg);
        assert_eq!(
            insertion(&always, &ctx("al", "al", Shell::Zsh)),
            Insertion::new(InsertionParts {
                delete: 0,
                text: "ways".into(),
                cursor_back: 0,
                execute: false
            })
        );
        // insert-control-chars: CR/LF/ESC/DEL never reach the PTY; a trailing "\n" still means execute
        for (kind, name) in [
            (SuggestionKind::File, "a\rtouch${IFS}PWNED\r"),
            (SuggestionKind::Arg, "x\x1b[2J\nrm\x7f"),
        ] {
            let i = insertion(&SuggestionCandidate::new(name, kind), &ctx("", "", Shell::Bash));
            assert!(
                !i.text().chars().any(char::is_control) && !i.executes(),
                "{name:?} → {i:?}"
            );
        }
        let mut generator = SuggestionCandidate::new("x", SuggestionKind::Arg);
        generator.insert_value = Some("dev\rrm -rf ~\n".into());
        assert_eq!(
            insertion(&generator, &ctx("", "", Shell::Bash)),
            Insertion::new(InsertionParts {
                delete: 0,
                text: "devrm -rf ~".into(),
                cursor_back: 0,
                execute: true
            })
        );
        // special-rows-no-delete: `help` rows replace the typed prefix instead of doubling it
        let build = SuggestionCandidate::new("build", SuggestionKind::Special);
        assert_eq!(insertion(&build, &ctx("bu", "bu", Shell::Bash)).text(), "ild");
        let i = insertion(&build, &ctx("BU", "BU", Shell::Bash));
        assert_eq!((i.delete(), i.text()), (2, "build"));
        // open-quote-escaping: inside a quote opened before the replaced segment, escape for that quote
        let path = |name: &str, kind| SuggestionCandidate {
            query: SuggestionQuery::Path { backslash: false },
            ..SuggestionCandidate::new(name, kind)
        };
        let file = path("x y.txt", SuggestionKind::File);
        let i = insertion(&file, &ctx("my dir/x", "\"my dir/x", Shell::Zsh));
        assert_eq!((i.delete(), i.text()), (0, " y.txt\""));
        let i = insertion(&file, &ctx("my dir/x", "'my dir/x", Shell::Bash));
        assert_eq!((i.delete(), i.text()), (0, " y.txt'"));
        let i = insertion(
            &path("sub dir/", SuggestionKind::Folder),
            &ctx("my dir/", "\"my dir/", Shell::Bash),
        );
        assert_eq!((i.delete(), i.text()), (0, "sub dir/")); // folders leave the quote open
        assert_eq!(
            insertion(
                &path("a\"b$`\\", SuggestionKind::File),
                &ctx("d/", "\"d/", Shell::Bash)
            )
            .text(),
            "a\\\"b\\$\\`\\\\\""
        );
        assert_eq!(
            insertion(
                &path("it's", SuggestionKind::File),
                &ctx("d/", "'d/", Shell::Bash)
            )
            .text(),
            "it'\\''s'"
        );
        assert_eq!(
            insertion(
                &path("it's", SuggestionKind::File),
                &ctx("d/", "'d/", Shell::Pwsh)
            )
            .text(),
            "it''s'"
        );
        assert_eq!(
            insertion(
                &path("a\"$b", SuggestionKind::File),
                &ctx("d/", "\"d/", Shell::Pwsh)
            )
            .text(),
            "a`\"`$b\""
        );
        // closed or escaped quotes, or a quote inside the replaced segment: unchanged escaping
        assert_eq!(
            insertion(&file, &ctx("my dir/x", "\"my dir\"/x", Shell::Bash)).text(),
            "\\ y.txt"
        );
        assert_eq!(
            insertion(&file, &ctx("\"d/x", "\\\"d/x", Shell::Bash)).text(),
            "\\ y.txt"
        );
        assert_eq!(
            insertion(&file, &ctx("d/x", "d/\"x", Shell::Bash)).text(),
            "'x y.txt'"
        );
    }

    #[test]
    fn insertion_round2_regressions() {
        let text = |s: SuggestionCandidate, sh| insertion(&s, &ctx("", "", sh)).text().to_string();
        // posix-escape-misses-shell-metachars / path-metachar-injection: names never become live shell syntax
        for sh in [Shell::Bash, Shell::Zsh, Shell::Fish] {
            for (name, kind) in [
                ("zz;id", SuggestionKind::Arg),
                ("a$b.txt", SuggestionKind::File),
                ("a`id`b", SuggestionKind::File),
                ("x{1,2}.txt", SuggestionKind::File),
                ("notes;touch${IFS}PWNED.txt", SuggestionKind::File),
                ("semi;colon dir.txt", SuggestionKind::File),
                ("~x", SuggestionKind::File),
                ("=cmd", SuggestionKind::File),
                ("a=~b", SuggestionKind::File),
            ] {
                assert_eq!(
                    text(SuggestionCandidate::new(name, kind), sh),
                    format!("'{name}'"),
                    "{sh:?} {name}"
                );
            }
            assert_eq!(text(SuggestionCandidate::new("$b;x/", SuggestionKind::Folder), sh), "'$b;x'/");
            assert_eq!(
                text(SuggestionCandidate::new("a b-c_d.e+f,g:h@i%j=k~", SuggestionKind::File), sh),
                "a\\ b-c_d.e+f,g:h@i%j=k~"
            );
        }
        assert_eq!(
            text(SuggestionCandidate::new("it's\\", SuggestionKind::File), Shell::Fish),
            "'it\\'s\\\\'"
        );
        // a separator stays outside the escaping (`--out=` + cursor), even when the name needs quotes
        let sep = SuggestionCandidate {
            separator: Some("=".into()),
            ..SuggestionCandidate::new("--a;b", SuggestionKind::Option)
        };
        assert_eq!(
            insertion(&sep, &ctx("", "", Shell::Bash)),
            Insertion::new(InsertionParts {
                delete: 0,
                text: "'--a;b'=".into(),
                cursor_back: 0,
                execute: false
            })
        );
        // pwsh-quotes-variable-refs: a `$NAME` row stays a variable reference; a file named so is still quoted
        for sh in [Shell::Bash, Shell::Fish, Shell::Pwsh] {
            let i = insertion(&SuggestionCandidate::new("$HOME", SuggestionKind::Arg), &ctx("$HO", "$HO", sh));
            assert_eq!((i.delete(), i.text()), (0, "ME"), "{sh:?}");
            assert_eq!(text(SuggestionCandidate::new("$foo", SuggestionKind::File), sh), "'$foo'");
        }
        assert_eq!(
            text(SuggestionCandidate::new("$env:PATH", SuggestionKind::Arg), Shell::Pwsh),
            "$env:PATH"
        );
        assert_eq!(
            text(SuggestionCandidate::new("a$b.txt", SuggestionKind::File), Shell::Pwsh),
            "'a$b.txt'"
        );
        // pwsh-unicode-smart-quote-injection: pwsh ends a '…' string at ’ too, so smart single quotes are doubled
        assert_eq!(
            text(SuggestionCandidate::new("r’;x;’.txt", SuggestionKind::File), Shell::Pwsh),
            "'r’’;x;’’.txt'"
        );
        // pwsh-quoted-folder-midword-guard: folders are backtick-escaped bare words (cursor at the end), and a
        // quoted file can follow in the same word
        let i = insertion(
            &SuggestionCandidate::new("dir with space/", SuggestionKind::Folder),
            &ctx("di", "'di", Shell::Pwsh),
        );
        assert_eq!(
            (i.delete(), i.text(), i.cursor_back()),
            (3, "dir` with` space/", 0)
        );
        assert_eq!(
            text(SuggestionCandidate::new("a$b'(c)/", SuggestionKind::Folder), Shell::Pwsh),
            "a`$b`'`(c`)/"
        );
        let file = SuggestionCandidate {
            query: SuggestionQuery::Path { backslash: true },
            ..SuggestionCandidate::new("a b.txt", SuggestionKind::File)
        };
        let i = insertion(
            &file,
            &ctx("dir with space/", "dir` with` space/", Shell::Pwsh),
        );
        assert_eq!((i.delete(), i.text()), (0, "'a b.txt'"));
        // fn-queryterm-quoted-delete: a function getQueryTerm deletes the raw chars spelling the query
        let fq = |name: &str, kind, q: &str| SuggestionCandidate {
            query: SuggestionQuery::Value(q.into()),
            ..SuggestionCandidate::new(name, kind)
        };
        for (raw, sh) in [
            ("\"dir with space/sub d", Shell::Zsh),
            ("'dir with space/sub d", Shell::Bash),
            ("\"dir with space/sub d", Shell::Fish),
            ("'dir with space/sub d", Shell::Pwsh),
        ] {
            let i = insertion(
                &fq("sub dir/", SuggestionKind::Folder, "sub d"),
                &ctx("dir with space/sub d", raw, sh),
            );
            assert_eq!((i.delete(), i.text()), (0, "ir/"), "{raw} {sh:?}");
        }
        let i = insertion(
            &fq("my file.py", SuggestionKind::File, "my fi"),
            &ctx(
                "dir with space/my fi",
                "\"dir with space/my fi",
                Shell::Bash,
            ),
        );
        assert_eq!((i.delete(), i.text()), (0, "le.py\""));
        let i = insertion(
            &fq("my file.py", SuggestionKind::File, "my fi"),
            &ctx(
                "dir with space/my fi",
                "dir\\ with\\ space/my\\ fi",
                Shell::Bash,
            ),
        );
        assert_eq!((i.delete(), i.text()), (6, "my\\ file.py"));
        let i = insertion(
            &fq("my file.py", SuggestionKind::File, "my fi"),
            &ctx(
                "dir with space/my fi",
                "dir` with` space/my` fi",
                Shell::Pwsh,
            ),
        );
        assert_eq!((i.delete(), i.text()), (6, "'my file.py'"));
        let i = insertion(
            &fq("it's.txt", SuggestionKind::File, "it's"),
            &ctx("d/it's", "'d/it''s", Shell::Pwsh),
        );
        assert_eq!((i.delete(), i.text()), (5, "it''s.txt'"));
        // dq-bang-history-expansion: `!` inside bash/zsh double quotes; fish has no backtick escape there
        let path = |name: &str| SuggestionCandidate {
            query: SuggestionQuery::Path { backslash: false },
            ..SuggestionCandidate::new(name, SuggestionKind::File)
        };
        for sh in [Shell::Bash, Shell::Zsh] {
            assert_eq!(
                insertion(&path("wow!x"), &ctx("d/", "\"d/", sh)).text(),
                "wow\"'!'\"x\""
            );
        }
        assert_eq!(
            insertion(&path("a`b!$"), &ctx("d/", "\"d/", Shell::Fish)).text(),
            "a`b!\\$\""
        );
        assert_eq!(
            insertion(&path("a\\b'"), &ctx("d/", "'d/", Shell::Fish)).text(),
            "a\\\\b\\''"
        );
        // r3-escape-quotes-spec-tilde: a spec row's leading `~`/`%` is shell syntax (`cd ~`, `kill %1`)
        for sh in [Shell::Bash, Shell::Zsh, Shell::Fish] {
            assert_eq!(text(SuggestionCandidate::new("~", SuggestionKind::Arg), sh), "~", "{sh:?}");
            assert_eq!(text(SuggestionCandidate::new("%1", SuggestionKind::Arg), sh), "%1", "{sh:?}");
        }
        // pwsh-unicode-smart-quote-injection: PowerShell also ends strings at ‘ ’ ‚ ‛ (single) and “ ” „ (double):
        // they force quoting (shell_quoting::quote escapes them), are backtick-escaped in folders and open `"`
        let sq = |s: &str| shell_quoting::quote(Shell::Pwsh, s);
        for name in [
            "plain’name.txt",
            "report’;New-Item -Name TS -ItemType File;’.txt",
            "a‘b‚c‛d“e”f„g",
        ] {
            for kind in [SuggestionKind::File, SuggestionKind::Arg] {
                assert_eq!(
                    text(SuggestionCandidate::new(name, kind), Shell::Pwsh),
                    sq(name),
                    "{name} {kind:?}"
                );
            }
            assert_eq!(
                insertion(&path(name), &ctx("d/", "'d/", Shell::Pwsh)).text().to_string(),
                sq(name)[1..],
                "{name} in '"
            );
        }
        assert_eq!(
            text(SuggestionCandidate::new("x’;calc;‘/", SuggestionKind::Folder), Shell::Pwsh),
            "x`’`;calc`;`‘/"
        );
        assert_eq!(
            insertion(&path("a”;calc;“„"), &ctx("d/", "\"d/", Shell::Pwsh)).text(),
            "a`”;calc;`“`„\""
        );
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
