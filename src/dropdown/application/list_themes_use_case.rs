use crate::dropdown::domain::repositories::theme_repository::ThemeRepository;
use crate::dropdown::domain::value_objects::theme_choice::ThemeChoice;
use std::rc::Rc;

/// `tabsmith theme --list`: the built-in themes, then the custom ones.
pub(crate) struct ListThemesUseCase {
    themes: Rc<dyn ThemeRepository>,
}

impl ListThemesUseCase {
    pub(crate) fn new(themes: Rc<dyn ThemeRepository>) -> Self {
        ListThemesUseCase { themes }
    }

    /// The built-in names first, then the theme files' names as the repository lists them; a file named like a
    /// built-in theme is not listed again (it stays selectable).
    pub(crate) fn execute(&self) -> Vec<String> {
        let custom = self
            .themes
            .list()
            .into_iter()
            .filter(|name| !ThemeChoice::BUILT_IN_NAMES.contains(&name.as_str()));
        ThemeChoice::BUILT_IN_NAMES
            .iter()
            .map(|name| name.to_string())
            .chain(custom)
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dropdown::domain::repositories::theme_repository::InMemoryThemeRepository;
    use crate::dropdown::domain::value_objects::theme_definition::ThemeDefinition;

    fn list(themes: InMemoryThemeRepository) -> Vec<String> {
        ListThemesUseCase::new(Rc::new(themes)).execute()
    }

    fn with_themes(names: &[&str]) -> InMemoryThemeRepository {
        names
            .iter()
            .fold(InMemoryThemeRepository::default(), |themes, name| {
                themes.with_theme(name, ThemeDefinition::new(None))
            })
    }

    #[test]
    fn theme_list_order_built_ins() {
        let with_custom = list(with_themes(&["zed", "alpha", "dark"]));
        let without_custom = list(InMemoryThemeRepository::default());

        assert_eq!(with_custom, ["dark", "light", "system", "alpha", "zed"]);
        assert_eq!(without_custom, ["dark", "light", "system"]);
    }

    #[test]
    fn should_list_the_built_in_themes_first_then_the_custom_ones() {
        let names = list(with_themes(&["zed", "alpha"]));

        assert_eq!(names, ["dark", "light", "system", "alpha", "zed"]);
    }

    #[test]
    fn should_not_list_a_custom_theme_named_like_a_built_in_twice() {
        let names = list(with_themes(&["dark", "system", "zed"]));

        assert_eq!(names, ["dark", "light", "system", "zed"]);
    }

    #[test]
    fn should_list_only_the_built_in_themes_when_there_are_no_custom_ones() {
        let names = list(InMemoryThemeRepository::default());

        assert_eq!(names, ["dark", "light", "system"]);
    }
}
