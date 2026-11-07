use crate::modules::dropdown::domain::entities::menu::Menu;
use crate::modules::dropdown::domain::services::text_fitting::clean;
use crate::modules::specs::domain::value_objects::suggestion_kind::SuggestionKind;

/// (bold label, description): the selected item's, `file`/`folder`, else the current argument's. Sanitized.
pub(crate) fn describe(menu: &Menu) -> (String, String) {
    let sel = menu.items().get(menu.selected());
    let own = sel
        .and_then(|s| s.description())
        .filter(|d| !d.trim().is_empty())
        .map(String::from)
        .or_else(|| match sel.map(|s| s.kind()) {
            Some(SuggestionKind::File) => Some("file".into()),
            Some(SuggestionKind::Folder) => Some("folder".into()),
            Some(
                SuggestionKind::Subcommand
                | SuggestionKind::Option
                | SuggestionKind::Arg
                | SuggestionKind::Special
                | SuggestionKind::Shortcut
                | SuggestionKind::History
                | SuggestionKind::AutoExecute,
            )
            | None => None,
        });
    match (own, menu.arg_hint()) {
        (Some(d), _) => (String::new(), clean(&d)),
        (None, Some(a)) => (clean(a.name()), clean(a.description())),
        (None, None) => (String::new(), String::new()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::completion::domain::test_fixtures::{comp, item};
    use crate::modules::completion::domain::value_objects::suggestion::Suggestion;
    use crate::modules::settings::domain::value_objects::settings::Settings;
    use std::time::Instant;

    fn menu_over(items: Vec<Suggestion>) -> Menu {
        let mut menu = Menu::new();
        menu.on_completion(comp("c", 1, items), &Settings::default(), Instant::now());
        menu
    }

    fn words(label: &str, text: &str) -> (String, String) {
        (label.to_string(), text.to_string())
    }

    #[test]
    fn should_label_nothing_for_the_own_description_of_the_selected_suggestion() {
        let row = item("checkout", SuggestionKind::Subcommand, "c").with_description(Some(" two\twords ".into()));
        let menu = menu_over(vec![row]);

        let described = describe(&menu);

        // no label although an argument hint exists; the tab is one space; nothing is trimmed
        assert_eq!(described, words("", " two words "));
    }

    #[test]
    fn should_treat_a_blank_description_as_missing_and_name_a_file_or_folder() {
        let blank = |kind| menu_over(vec![item("x", kind, "").with_description(Some("  ".into()))]);

        let described = [SuggestionKind::File, SuggestionKind::Folder, SuggestionKind::Subcommand]
            .map(|kind| describe(&blank(kind)));

        // a blank description on any other kind falls through to the argument hint
        assert_eq!(described, [words("", "file"), words("", "folder"), words("path", "A path")]);
    }

    #[test]
    fn should_describe_the_argument_hint_when_nothing_is_selected() {
        let no_rows = menu_over(Vec::new());
        let never_fed = Menu::new();

        let described = [describe(&no_rows), describe(&never_fed)];

        assert_eq!(described, [words("path", "A path"), words("", "")]);
    }
}
