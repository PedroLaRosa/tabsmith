use crate::completion::domain::services::fuzzy_matcher::fuzzysort;
use crate::completion::domain::value_objects::filter_options::FilterOptions;
use crate::completion::domain::value_objects::suggestion_candidate::SuggestionCandidate;
use crate::specs::domain::value_objects::suggestion_kind::SuggestionKind;
use std::cmp::Ordering;

/// ICU `localeCompare` approximation: lowercase compare, ties by exact compare (SPEC §6.7.1).
pub(crate) fn cmp_locale(a: &str, b: &str) -> Ordering {
    a.to_lowercase()
        .cmp(&b.to_lowercase())
        .then_with(|| a.cmp(b))
}

/// Prefix ladder: 0 exact, −1 exact ignoring case, −2 prefix, −3 prefix ignoring case.
pub(crate) fn prefix_score(name: &str, q: &str) -> Option<i32> {
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
pub(crate) fn partial(names: &[String], q: &str, fuzzy: bool) -> Option<(f64, bool)> {
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

pub(crate) fn auto_exec(name: &str, description: &str, original: Option<SuggestionKind>) -> SuggestionCandidate {
    let mut s = SuggestionCandidate::new(name, SuggestionKind::AutoExecute);
    s.original_kind = original;
    s.insert_value = Some("\n".into());
    s.description = Some(description.into());
    s
}

/// `filterSuggestions`: exact (non-folder A, folder B) then partial (C by score, priority). Input must be
/// priority-sorted. Returns `(row, prefix_match)`.
pub(crate) fn filter(list: Vec<SuggestionCandidate>, search_term: &str, o: &FilterOptions) -> Vec<(SuggestionCandidate, bool)> {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::completion::domain::test_fixtures::{arg, ctx, names, opts};
    use crate::completion::domain::value_objects::suggestion::Suggestion;
    use crate::completion::domain::value_objects::suggestion_query::SuggestionQuery;
    use crate::shared::domain::value_objects::shell::Shell;
    use crate::completion::domain::services::suggestion_finalizer::to_item;

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
}
