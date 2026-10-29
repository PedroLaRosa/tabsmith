use crate::completion::domain::services::fuzzy_matcher::fuzzysort;
use crate::completion::domain::services::insertion_composer::insertion;
use crate::completion::domain::value_objects::insertion_context::InsertionContext;
use crate::completion::domain::value_objects::suggestion::{Suggestion, SuggestionParts};
use crate::completion::domain::value_objects::suggestion_candidate::SuggestionCandidate;
use crate::specs::domain::value_objects::arg::Arg;
use std::rc::Rc;

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
    let segs: Vec<&str> = match s.display_name() {
        Some(d) => vec![d.as_str()],
        None => s.names().iter().map(String::as_str).collect(),
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
pub(crate) fn to_item(s: SuggestionCandidate, prefix_match: bool, c: &InsertionContext) -> Suggestion {
    let query = s.query().of(c.search_term());
    Suggestion::new(SuggestionParts {
        insert: insertion(&s, c),
        highlight: highlight(&s, &query, c.fuzzy()),
        args_hint: args_hint(s.args()),
        query,
        names: s.names().clone(),
        display_name: s.display_name().clone(),
        description: s.description().clone(),
        kind: s.kind(),
        original_kind: s.original_kind(),
        priority: s.priority(),
        is_dangerous: s.is_dangerous(),
        generator: s.generator(),
        prefix_match,
        template_type: s.template_type().clone(),
        icon: s.icon().clone(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::completion::domain::test_fixtures::ctx;
    use crate::shared::domain::value_objects::shell::Shell;
    use crate::specs::domain::value_objects::suggestion_kind::SuggestionKind;

    #[test]
    fn item_highlight_and_hint() {
        let s = SuggestionCandidate::new("-m", SuggestionKind::Option)
            .with_names(vec!["-m".into(), "--message".into()])
            .with_args(vec![
                Rc::new(Arg::default().with_name(Some("msg".into()))),
                Rc::new(
                    Arg::default()
                        .with_name(Some("more".into()))
                        .with_is_optional(true)
                        .with_is_variadic(true),
                ),
            ]);
        let it = to_item(s, true, &ctx("--m", "--m", Shell::Bash));
        assert_eq!(it.title(), "-m, --message");
        assert_eq!(it.highlight(), [4, 5, 6]);
        assert_eq!(it.args_hint(), "<msg> [more...]");
        let s = SuggestionCandidate::new("Fuzzy Search", SuggestionKind::Arg);
        let it = to_item(
            s,
            false,
            &ctx("fs", "fs", Shell::Bash).with_fuzzy(true),
        );
        assert_eq!(it.highlight(), [0, 6]);
    }
}
