use crate::dropdown::domain::value_objects::rgb::Rgb;

/// Eight theme colours as RGB (SPEC §7.7).
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Theme {
    main_bg: Rgb,
    main_text: Rgb,
    matching_bg: Rgb,
    selected_bg: Rgb,
    selected_text: Rgb,
    selected_matching_bg: Rgb,
    description_text: Rgb,
    description_border: Rgb,
}

/// What a `Theme` is built from: named fields, so no two of the eight colours can be swapped (a positional `new` would take eight parameters).
pub(crate) struct ThemeParts {
    pub(crate) main_bg: Rgb,
    pub(crate) main_text: Rgb,
    pub(crate) matching_bg: Rgb,
    pub(crate) selected_bg: Rgb,
    pub(crate) selected_text: Rgb,
    pub(crate) selected_matching_bg: Rgb,
    pub(crate) description_text: Rgb,
    pub(crate) description_border: Rgb,
}

impl Theme {
    pub(crate) fn new(parts: ThemeParts) -> Self {
        let ThemeParts {
            main_bg,
            main_text,
            matching_bg,
            selected_bg,
            selected_text,
            selected_matching_bg,
            description_text,
            description_border,
        } = parts;
        Theme {
            main_bg,
            main_text,
            matching_bg,
            selected_bg,
            selected_text,
            selected_matching_bg,
            description_text,
            description_border,
        }
    }

    pub(crate) fn main_bg(&self) -> Rgb {
        self.main_bg
    }

    pub(crate) fn main_text(&self) -> Rgb {
        self.main_text
    }

    pub(crate) fn matching_bg(&self) -> Rgb {
        self.matching_bg
    }

    pub(crate) fn selected_bg(&self) -> Rgb {
        self.selected_bg
    }

    pub(crate) fn selected_text(&self) -> Rgb {
        self.selected_text
    }

    pub(crate) fn selected_matching_bg(&self) -> Rgb {
        self.selected_matching_bg
    }

    pub(crate) fn description_text(&self) -> Rgb {
        self.description_text
    }

    pub(crate) fn description_border(&self) -> Rgb {
        self.description_border
    }

    pub(crate) fn dark() -> Self {
        Theme::new(ThemeParts {
            main_bg: (48, 48, 48),
            main_text: (180, 180, 180),
            matching_bg: (95, 89, 56),
            selected_bg: (30, 90, 199),
            selected_text: (253, 253, 253),
            selected_matching_bg: (106, 142, 218),
            description_text: (180, 180, 180),
            description_border: (65, 65, 65),
        })
    }

    pub(crate) fn light() -> Self {
        Theme::new(ThemeParts {
            main_bg: (254, 254, 254),
            main_text: (7, 7, 7),
            matching_bg: (255, 239, 152),
            selected_bg: (41, 105, 218),
            selected_text: (253, 253, 253),
            selected_matching_bg: (106, 142, 218),
            description_text: (7, 7, 7),
            description_border: (199, 199, 199),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn colours(theme: &Theme) -> [Rgb; 8] {
        [
            theme.main_bg(),
            theme.main_text(),
            theme.matching_bg(),
            theme.selected_bg(),
            theme.selected_text(),
            theme.selected_matching_bg(),
            theme.description_text(),
            theme.description_border(),
        ]
    }

    /// Dark and light give `main_text` and `description_text` the same colour, so the custom theme gives each role its own value:
    /// swapped fields or accessors would otherwise pass.
    #[test]
    fn should_read_every_colour_of_a_theme_through_its_accessor() {
        // Arrange
        let custom = Theme::new(ThemeParts {
            main_bg: (1, 1, 1),
            main_text: (2, 2, 2),
            matching_bg: (3, 3, 3),
            selected_bg: (4, 4, 4),
            selected_text: (5, 5, 5),
            selected_matching_bg: (6, 6, 6),
            description_text: (7, 7, 7),
            description_border: (8, 8, 8),
        });

        // Act
        let (dark, light, custom) = (
            colours(&Theme::dark()),
            colours(&Theme::light()),
            colours(&custom),
        );

        // Assert
        assert_eq!(
            dark,
            [
                (48, 48, 48),
                (180, 180, 180),
                (95, 89, 56),
                (30, 90, 199),
                (253, 253, 253),
                (106, 142, 218),
                (180, 180, 180),
                (65, 65, 65)
            ]
        );
        assert_eq!(
            light,
            [
                (254, 254, 254),
                (7, 7, 7),
                (255, 239, 152),
                (41, 105, 218),
                (253, 253, 253),
                (106, 142, 218),
                (7, 7, 7),
                (199, 199, 199)
            ]
        );
        assert_eq!(
            custom,
            [
                (1, 1, 1),
                (2, 2, 2),
                (3, 3, 3),
                (4, 4, 4),
                (5, 5, 5),
                (6, 6, 6),
                (7, 7, 7),
                (8, 8, 8)
            ]
        );
    }
}
