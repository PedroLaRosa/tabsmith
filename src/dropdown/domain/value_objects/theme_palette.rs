use crate::dropdown::domain::services::color_parser::parse_color;
use crate::dropdown::domain::value_objects::rgb::Rgb;
use crate::dropdown::domain::value_objects::theme::{Theme, ThemeParts};

/// base16 filler for custom themes (`themes.ts:46-83`, dark variant).
const SHADES: [&str; 8] = [
    "#181818", "#282828", "#383838", "#585858", "#b8b8b8", "#d8d8d8", "#e8e8e8", "#f8f8f8",
];
const ACCENTS: [&str; 8] = [
    "#ab4642", "#dc9656", "#f7ca88", "#a1b56c", "#86c1b9", "#7cafc2", "#ba8baf", "#a16946",
];

/// The colours a Fig-format theme file names, one per role; `None` = missing or unparsable.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct FigPaletteParts {
    pub(crate) main_bg: Option<Rgb>,
    pub(crate) main_text: Option<Rgb>,
    pub(crate) matching_bg: Option<Rgb>,
    pub(crate) selected_bg: Option<Rgb>,
    pub(crate) selected_text: Option<Rgb>,
    pub(crate) selected_matching_bg: Option<Rgb>,
    pub(crate) description_text: Option<Rgb>,
    pub(crate) description_border: Option<Rgb>,
}

/// The seven base16 keys a theme file is read for (`shade1` serves two roles); `None` = missing or unparsable.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Base16PaletteParts {
    pub(crate) shade0: Option<Rgb>,
    pub(crate) shade1: Option<Rgb>,
    pub(crate) shade2: Option<Rgb>,
    pub(crate) shade5: Option<Rgb>,
    pub(crate) shade6: Option<Rgb>,
    pub(crate) shade7: Option<Rgb>,
    pub(crate) accent6: Option<Rgb>,
}

#[derive(Clone, Debug, PartialEq)]
enum Roles {
    Fig(FigPaletteParts),
    Base16(Base16PaletteParts),
}

/// What a custom theme file gives, role by role; `theme()` fills the gaps.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct ThemePalette {
    roles: Roles,
}

impl ThemePalette {
    pub(crate) fn fig(parts: FigPaletteParts) -> Self {
        Self {
            roles: Roles::Fig(parts),
        }
    }

    pub(crate) fn base16(parts: Base16PaletteParts) -> Self {
        Self {
            roles: Roles::Base16(parts),
        }
    }

    /// Fig: a missing role comes from `Theme::dark()`. base16: a missing key comes from the dark filler (`SHADES`, `ACCENTS`).
    pub(crate) fn theme(&self) -> Theme {
        match &self.roles {
            Roles::Fig(parts) => fig_theme(parts),
            Roles::Base16(parts) => base16_theme(parts),
        }
    }
}

fn fig_theme(parts: &FigPaletteParts) -> Theme {
    let d = Theme::dark();
    Theme::new(ThemeParts {
        main_bg: parts.main_bg.unwrap_or(d.main_bg()),
        main_text: parts.main_text.unwrap_or(d.main_text()),
        matching_bg: parts.matching_bg.unwrap_or(d.matching_bg()),
        selected_bg: parts.selected_bg.unwrap_or(d.selected_bg()),
        selected_text: parts.selected_text.unwrap_or(d.selected_text()),
        selected_matching_bg: parts
            .selected_matching_bg
            .unwrap_or(d.selected_matching_bg()),
        description_text: parts.description_text.unwrap_or(d.description_text()),
        description_border: parts.description_border.unwrap_or(d.description_border()),
    })
}

fn base16_theme(parts: &Base16PaletteParts) -> Theme {
    let filler = |list: &[&str; 8], i: usize| parse_color(list[i]).unwrap_or_default();
    let shade = |role: Option<Rgb>, i: usize| role.unwrap_or_else(|| filler(&SHADES, i));
    let accent = |role: Option<Rgb>, i: usize| role.unwrap_or_else(|| filler(&ACCENTS, i));
    Theme::new(ThemeParts {
        main_bg: shade(parts.shade0, 0),
        main_text: shade(parts.shade6, 6),
        matching_bg: shade(parts.shade1, 1),
        selected_bg: shade(parts.shade2, 2),
        selected_text: shade(parts.shade7, 7),
        selected_matching_bg: accent(parts.accent6, 6),
        description_text: shade(parts.shade5, 5),
        description_border: shade(parts.shade1, 1),
    })
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

    fn no_fig_roles() -> FigPaletteParts {
        FigPaletteParts {
            main_bg: None,
            main_text: None,
            matching_bg: None,
            selected_bg: None,
            selected_text: None,
            selected_matching_bg: None,
            description_text: None,
            description_border: None,
        }
    }

    fn no_base16_keys() -> Base16PaletteParts {
        Base16PaletteParts {
            shade0: None,
            shade1: None,
            shade2: None,
            shade5: None,
            shade6: None,
            shade7: None,
            accent6: None,
        }
    }

    #[test]
    fn should_fall_back_per_role_of_a_fig_palette_to_the_dark_theme() {
        let palette = ThemePalette::fig(FigPaletteParts {
            main_bg: Some((0, 0, 0)),
            description_text: Some((16, 32, 48)),
            ..no_fig_roles()
        });
        let dark = colours(&Theme::dark());

        let result = colours(&palette.theme());

        assert_eq!(
            result,
            [
                (0, 0, 0),
                dark[1],
                dark[2],
                dark[3],
                dark[4],
                dark[5],
                (16, 32, 48),
                dark[7]
            ]
        );
    }

    #[test]
    fn should_fall_back_per_key_of_a_base16_palette_to_the_filler() {
        let palette = ThemePalette::base16(Base16PaletteParts {
            shade0: Some((16, 16, 16)),
            shade1: Some((10, 11, 12)),
            ..no_base16_keys()
        });

        let result = colours(&palette.theme());

        assert_eq!(
            result,
            [
                (16, 16, 16),
                (232, 232, 232),
                (10, 11, 12),
                (56, 56, 56),
                (248, 248, 248),
                (186, 139, 175),
                (216, 216, 216),
                (10, 11, 12)
            ]
        );
    }

    #[test]
    fn should_build_the_dark_filler_theme_from_an_empty_palette() {
        let base16 = ThemePalette::base16(no_base16_keys());
        let fig = ThemePalette::fig(no_fig_roles());

        let result = (colours(&base16.theme()), fig.theme());

        assert_eq!(
            result,
            (
                [
                    (24, 24, 24),
                    (232, 232, 232),
                    (40, 40, 40),
                    (56, 56, 56),
                    (248, 248, 248),
                    (186, 139, 175),
                    (216, 216, 216),
                    (40, 40, 40)
                ],
                Theme::dark()
            )
        );
    }
}
