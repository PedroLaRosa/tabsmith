use crate::completion::domain::value_objects::suggestion::Suggestion;
use crate::specs::domain::value_objects::suggestion_kind::SuggestionKind;

/// Specs source (SPEC §6.11): first non-auto-execute/history item with `delete == 0`, prefix match and non-empty
/// text (trailing space trimmed).
pub(crate) fn spec_ghost(items: &[Suggestion]) -> Option<String> {
    items.iter().find_map(|i| {
        let text = i.insert().text().strip_suffix(' ').unwrap_or(i.insert().text());
        // accepting a ghost types its text and leaves the cursor at the end: rows that move it back are skipped
        let ok = !matches!(i.kind(), SuggestionKind::AutoExecute | SuggestionKind::History)
            && i.insert().delete() == 0
            && i.insert().cursor_back() <= i.insert().text().len() - text.len()
            && i.prefix_match()
            && !text.is_empty();
        ok.then(|| text.to_string())
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::completion::domain::value_objects::insertion::{Insertion, InsertionParts};
    use crate::completion::domain::services::suggestion_filter::filter;
    use crate::completion::domain::services::suggestion_finalizer::to_item;
    use crate::completion::domain::test_fixtures::{ctx, opts};
    use crate::completion::domain::value_objects::suggestion::SuggestionParts;
    use crate::completion::domain::value_objects::suggestion_candidate::SuggestionCandidate;
    use crate::shared::domain::value_objects::shell::Shell;

    fn item(kind: SuggestionKind, delete: usize, text: &str, prefix: bool) -> Suggestion {
        Suggestion::new(SuggestionParts {
            names: vec!["n".into()],
            display_name: None,
            description: None,
            kind,
            original_kind: None,
            args_hint: String::new(),
            highlight: Vec::new(),
            query: String::new(),
            insert: Insertion::new(InsertionParts {
                delete,
                text: text.into(),
                cursor_back: 0,
                execute: false,
            }),
            generator: None,
            prefix_match: prefix,
            icon: None,
        })
    }

    #[test]
    fn spec_ghost_rules() {
        let items = [
            item(SuggestionKind::AutoExecute, 0, "x", true),
            item(SuggestionKind::History, 0, "y", true),
            item(SuggestionKind::Subcommand, 2, "checkout ", true),
            item(SuggestionKind::Subcommand, 0, "erry-pick ", false),
            item(SuggestionKind::Option, 0, " ", true),
            item(SuggestionKind::Subcommand, 0, "eckout ", true),
            item(SuggestionKind::Subcommand, 0, "erry ", true),
        ];
        assert_eq!(spec_ghost(&items).as_deref(), Some("eckout"));
        assert_eq!(spec_ghost(&items[..5]), None);
        assert_eq!(
            spec_ghost(&[item(SuggestionKind::Arg, 0, "a  ", true)]).as_deref(),
            Some("a ")
        );
        // spec-ghost-ignores-cursor: `commit -m '{cursor}'` would leave the cursor outside the quotes; `--out=`
        // (cursor right after `=`, before the auto-space) is fine
        let with_back = |text: &str, back| {
            let i = item(SuggestionKind::Arg, 0, text, true);
            let insert = i.insert().clone().with_cursor_back(back);
            i.with_insertion(insert)
        };
        assert_eq!(spec_ghost(&[with_back("commit -m ''", 1)]), None);
        assert_eq!(
            spec_ghost(&[with_back("author= ", 1)]).as_deref(),
            Some("author=")
        );
        assert_eq!(spec_ghost(&[with_back("author= ", 2)]), None);
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
            spec_ghost(&items)
        };
        assert_eq!(ghost(""), None);
        assert_eq!(ghost("ab").as_deref(), Some("ort"));
    }
}
