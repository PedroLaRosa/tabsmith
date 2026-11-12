use crate::modules::completion::domain::value_objects::insertion::{Insertion, InsertionParts};
use crate::modules::completion::domain::value_objects::suggestion::Suggestion;
use crate::modules::dropdown::domain::value_objects::common_prefix_outcome::CommonPrefixOutcome;
use crate::modules::specs::domain::value_objects::suggestion_kind::SuggestionKind;

/// What inserting the common prefix of `items` does for the `selected` row. Changes nothing.
pub(crate) fn decide(items: &[Suggestion], selected: usize) -> CommonPrefixOutcome {
    let sel = &items[selected];
    if items.len() == 1 {
        return CommonPrefixOutcome::InsertSelected;
    }
    let auto = sel.kind() == SuggestionKind::AutoExecute;
    let kind = if auto {
        match sel.original_kind() {
            Some(kind) => kind,
            None => return CommonPrefixOutcome::Failed,
        }
    } else {
        sel.kind()
    };
    if matches!(kind, SuggestionKind::Special | SuggestionKind::AutoExecute) {
        return CommonPrefixOutcome::Failed;
    }
    let mut sel_first = sel.first_name().to_string();
    if auto && kind == SuggestionKind::Folder && !sel_first.ends_with('/') {
        sel_first.push('/');
    }
    let q = sel.query().to_lowercase();
    let mut names: Vec<String> = items
        .iter()
        .filter(|a| matching_type(a, &sel_first, kind))
        .map(|a| a.first_name().to_lowercase())
        .filter(|n| n.starts_with(&q))
        .collect();
    if names.len() == 1 {
        return if auto {
            CommonPrefixOutcome::Failed
        } else {
            CommonPrefixOutcome::InsertSelected
        };
    }
    names.sort();
    let lcp = match (names.first(), names.last()) {
        (Some(a), Some(b)) => a.chars().zip(b.chars()).take_while(|(x, y)| x == y).count(),
        _ => 0,
    };
    let shared: String = sel.first_name().chars().take(lcp).collect();
    if shared.is_empty() || shared == sel.query() {
        return CommonPrefixOutcome::Failed;
    }
    let text: String = shared
        .chars()
        .map(|c| {
            if c.is_whitespace() {
                "\\ ".into()
            } else {
                c.to_string()
            }
        })
        .collect();
    // The engine strips an already-typed query from `insert.text` (delete == 0): put it back to compare.
    let full = if sel.insert().delete() == 0 {
        format!("{}{}", sel.query(), sel.insert().text())
    } else {
        sel.insert().text().to_string()
    };
    // The engine escaped the row for its shell: type the shared prefix only as the start of that insertion
    // (`$XDG_` of `$XDG_CONFIG_HOME`), else BEL (`zz;` of `'zz;a'`, pwsh `my\ dir` of `'my dir1.txt'`).
    // ponytail: a prefix that needs quoting is not quoted half-way.
    if !full.starts_with(&text) {
        return CommonPrefixOutcome::Failed;
    }
    if text == full || format!("{text} ") == full {
        return if auto {
            CommonPrefixOutcome::Failed
        } else {
            CommonPrefixOutcome::InsertSelected
        };
    }
    // Partial insertion. Raw on-screen token chars (D22): the item's own delete count, or the query when it was
    // stripped (then the typed text equals the query and is not retyped).
    let q_chars = sel.query().chars().count();
    let raw = if sel.insert().delete() > 0 {
        sel.insert().delete()
    } else {
        q_chars
    };
    let (delete, text) = match text.strip_prefix(sel.query()) {
        Some(rest) if raw == q_chars => (0, rest.to_string()),
        _ => (raw, text),
    };
    CommonPrefixOutcome::InsertPrefix(Insertion::new(InsertionParts {
        delete,
        text,
        cursor_back: 0,
        execute: false,
    }))
}

/// Whether the types match (`../` only matches `../`; file and folder match each other).
fn matching_type(a: &Suggestion, b_first: &str, b_kind: SuggestionKind) -> bool {
    if a.first_name() == "../" {
        return b_first == "../";
    }
    let file_like = |k| matches!(k, SuggestionKind::File | SuggestionKind::Folder);
    (file_like(a.kind()) && file_like(b_kind)) || a.kind() == b_kind
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::completion::domain::test_fixtures::item;

    fn escaped(name: &str, query: &str, text: &str) -> Suggestion {
        item(name, SuggestionKind::File, query).with_insertion(Insertion::new(InsertionParts {
            delete: query.chars().count(),
            text: text.into(),
            cursor_back: 0,
            execute: false,
        }))
    }

    #[test]
    fn should_decide_a_full_insertion_for_a_lone_row_whatever_its_kind() {
        let kinds = [
            SuggestionKind::Special,
            SuggestionKind::AutoExecute,
            SuggestionKind::Folder,
        ];
        let lone_rows: Vec<Vec<Suggestion>> = kinds
            .iter()
            .map(|kind| vec![item("x", *kind, "")])
            .collect();

        let outcomes: Vec<CommonPrefixOutcome> =
            lone_rows.iter().map(|rows| decide(rows, 0)).collect();

        assert_eq!(outcomes, vec![CommonPrefixOutcome::InsertSelected; 3]);
    }

    #[test]
    fn should_fail_for_an_auto_execute_row_without_an_original_kind() {
        let rows = vec![
            item("↪", SuggestionKind::AutoExecute, ""),
            item("x", SuggestionKind::Arg, ""),
        ];

        let outcome = decide(&rows, 0);

        assert_eq!(outcome, CommonPrefixOutcome::Failed);
    }

    #[test]
    fn should_keep_the_original_case_of_the_selected_name_in_the_shared_prefix() {
        let rows = vec![
            item("Checkout", SuggestionKind::Subcommand, "ch"),
            item("CherryPick", SuggestionKind::Subcommand, "ch"),
        ];

        let outcome = decide(&rows, 0);

        assert_eq!(
            outcome,
            CommonPrefixOutcome::InsertPrefix(Insertion::new(InsertionParts {
                delete: 2,
                text: "Che".into(),
                cursor_back: 0,
                execute: false,
            }))
        );
    }

    #[test]
    fn should_escape_every_whitespace_character_of_the_shared_prefix() {
        let rows = vec![
            escaped("a b\tc1", "a", "a\\ b\\ c1"),
            escaped("a b\tc2", "a", "a\\ b\\ c2"),
        ];

        let outcome = decide(&rows, 0);

        assert_eq!(
            outcome,
            CommonPrefixOutcome::InsertPrefix(Insertion::new(InsertionParts {
                delete: 0,
                text: "\\ b\\ c".into(),
                cursor_back: 0,
                execute: false,
            }))
        );
    }

    #[test]
    fn should_fail_when_the_shared_prefix_needs_quoting() {
        let rows = vec![
            escaped("zz;a", "z", "'zz;a'"),
            escaped("zz;b", "z", "'zz;b'"),
        ];

        let outcome = decide(&rows, 0);

        assert_eq!(outcome, CommonPrefixOutcome::Failed);
    }
}
