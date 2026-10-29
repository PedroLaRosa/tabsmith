use crate::completion::domain::value_objects::suggestion_candidate::SuggestionCandidate;
use crate::history::domain::entities::history_entry::HistoryEntry;
use crate::specs::domain::value_objects::suggestion_kind::SuggestionKind;
use std::collections::{HashMap, HashSet};

/// Rows for history entries extending `prefix` (= buffer up to the start of the last token), newest first.
pub(crate) fn history_rows(prefix: &str, entries: &[HistoryEntry]) -> Vec<SuggestionCandidate> {
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
pub(crate) fn mix_history(spec: Vec<SuggestionCandidate>, hist: Vec<SuggestionCandidate>, show: bool) -> Vec<SuggestionCandidate> {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::history::domain::entities::history_entry::HistoryEntryPrimitives;

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
}
