/// The theme `autocomplete.theme` names: one of the three built-in themes, or a custom theme file.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum ThemeChoice {
    Dark,
    Light,
    System,
    Custom(String),
}

impl ThemeChoice {
    /// What `theme --list` shows first and `theme NAME` accepts without a theme file, in this order.
    pub(crate) const BUILT_IN_NAMES: [&'static str; 3] = ["dark", "light", "system"];

    /// How the drawing reads `autocomplete.theme`: `""` is dark, and any name that is not built in is a custom theme.
    pub(crate) fn from_name(name: &str) -> ThemeChoice {
        match name {
            "" | "dark" => ThemeChoice::Dark,
            "light" => ThemeChoice::Light,
            "system" => ThemeChoice::System,
            custom => ThemeChoice::Custom(custom.to_string()),
        }
    }

    /// How `theme NAME` reads a name that has no theme file: only the three built-in names, never `""`.
    pub(crate) fn built_in(name: &str) -> Option<ThemeChoice> {
        match name {
            "dark" => Some(ThemeChoice::Dark),
            "light" => Some(ThemeChoice::Light),
            "system" => Some(ThemeChoice::System),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn should_list_the_built_in_theme_names_in_order() {
        let names = ThemeChoice::BUILT_IN_NAMES;

        assert_eq!(names, ["dark", "light", "system"]);
    }

    #[test]
    fn should_read_an_empty_name_as_dark_for_the_renderer() {
        let choices = ["", "dark", "light", "system", "zed"].map(ThemeChoice::from_name);

        assert_eq!(
            choices,
            [
                ThemeChoice::Dark,
                ThemeChoice::Dark,
                ThemeChoice::Light,
                ThemeChoice::System,
                ThemeChoice::Custom("zed".to_string())
            ]
        );
    }

    #[test]
    fn should_not_take_an_empty_name_as_a_built_in_for_the_cli() {
        let choices = ["", "dark", "Dark", "dark ", "zed"].map(ThemeChoice::built_in);

        assert_eq!(choices, [None, Some(ThemeChoice::Dark), None, None, None]);
    }
}
