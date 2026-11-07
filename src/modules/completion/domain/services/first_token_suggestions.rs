use crate::modules::completion::domain::value_objects::suggestion_candidate::SuggestionCandidate;
use crate::modules::specs::domain::value_objects::suggestion_kind::SuggestionKind;
use std::collections::{HashMap, HashSet};

/// Root spec names: no `/`, or a scoped npm name `@scope/name`.
fn is_root_spec(n: &str) -> bool {
    match n.matches('/').count() {
        0 => true,
        1 => n.starts_with('@'),
        _ => false,
    }
}

/// Spec names (priority 40) ∪ aliases (shortcut, priority 100) ∪ PATH executables (priority 40), deduped by name.
pub(crate) fn first_token_items(
    specs: &[String],
    aliases: &HashMap<String, String>,
    executables: &[String],
) -> Vec<SuggestionCandidate> {
    let mut seen = HashSet::new();
    let mut out = Vec::new();
    let mut push = |name: &str, kind: SuggestionKind, priority: f64, description: Option<&String>| {
        if !name.is_empty() && seen.insert(name.to_string()) {
            let s = SuggestionCandidate::new(name, kind)
                .with_priority(priority)
                .with_description(description.cloned());
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
    for exe in executables {
        push(exe, SuggestionKind::Subcommand, 40.0, None);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn should_offer_spec_names_then_aliases_then_executables_each_name_once() {
        let specs = vec!["git".to_string(), "aws/s3".into(), "@scope/cli".into()];
        let aliases: HashMap<String, String> = [("gst".to_string(), "git status".to_string())].into();
        let executables = vec!["mytool".to_string(), "git".into(), "mytool".into()];

        let items = first_token_items(&specs, &aliases, &executables);

        let rows: Vec<(&str, SuggestionKind, f64)> = items
            .iter()
            .map(|s| (s.first(), s.kind(), s.priority()))
            .collect();
        let shortcut_description = items[2].description().as_deref();
        assert_eq!(
            (rows, shortcut_description),
            (
                vec![
                    ("git", SuggestionKind::Subcommand, 40.0),
                    ("@scope/cli", SuggestionKind::Subcommand, 40.0),
                    ("gst", SuggestionKind::Shortcut, 100.0),
                    ("mytool", SuggestionKind::Subcommand, 40.0)
                ],
                Some("git status")
            )
        );
    }
}
