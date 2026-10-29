use crate::completion::domain::services::suggestion_filter::filter;
use crate::completion::domain::value_objects::filter_options::FilterOptions;
use crate::completion::domain::value_objects::suggestion_candidate::SuggestionCandidate;
use crate::specs::domain::value_objects::suggestion_kind::SuggestionKind;
use std::cmp::Ordering;
use std::collections::HashMap;
use std::rc::Rc;

/// Recency boost; `recency` = `recency[cmd]`, `None` with `sortMethod = alphabetical` (SPEC §6.7.5).
pub(crate) fn update_priorities(list: &mut [SuggestionCandidate], recency: Option<&HashMap<String, u64>>) {
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

fn same_row(a: &SuggestionCandidate, b: &SuggestionCandidate) -> bool {
    // ponytail: args compared by identity (not deep-compared); distinct specs never share names anyway
    a.names == b.names
        && a.insert_value == b.insert_value
        && a.display_name == b.display_name
        && a.args.len() == b.args.len()
        && a.args.iter().zip(&b.args).all(|(x, y)| Rc::ptr_eq(x, y))
}

/// Keep the first of rows equal in (names, insertValue, displayName, args).
pub(crate) fn dedupe(list: Vec<(SuggestionCandidate, bool)>) -> Vec<(SuggestionCandidate, bool)> {
    let mut out: Vec<(SuggestionCandidate, bool)> = Vec::with_capacity(list.len());
    for x in list {
        if !out.iter().any(|y| same_row(&x.0, &y.0)) {
            out.push(x);
        }
    }
    out
}

/// SPEC §6.7.5: recency priorities → stable sort by priority → filter → dedupe when 0 < n ≤ 50.
pub(crate) fn rank(
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::completion::domain::test_fixtures::{names, opts};

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
}
