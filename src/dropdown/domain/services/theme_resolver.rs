use crate::dropdown::domain::repositories::theme_repository::ThemeRepository;
use crate::dropdown::domain::value_objects::rgb::Rgb;
use crate::dropdown::domain::value_objects::theme::Theme;
use crate::dropdown::domain::value_objects::theme_choice::ThemeChoice;
use crate::settings::domain::services::settings_catalog::AUTOCOMPLETE_THEME;
use crate::settings::domain::value_objects::settings::Settings;

/// Which `Theme` `autocomplete.theme` and the terminal background give. The repository is asked only for a custom name: `dark`, `light`, `system` and an empty name never read a file, even when `themes/light.json` exists (the `theme` command, which reads the file first, disagrees by design).
pub(crate) fn resolve_theme(
    settings: &Settings,
    terminal_bg: Option<Rgb>,
    themes: &dyn ThemeRepository,
) -> Theme {
    match ThemeChoice::from_name(settings.string(AUTOCOMPLETE_THEME).unwrap_or_default()) {
        ThemeChoice::Dark => Theme::dark(),
        ThemeChoice::Light => Theme::light(),
        ThemeChoice::System => match terminal_bg {
            Some((r, g, b))
                if (0.299 * r as f64 + 0.587 * g as f64 + 0.114 * b as f64) / 255.0 >= 0.5 =>
            {
                Theme::light()
            }
            _ => Theme::dark(),
        },
        ThemeChoice::Custom(name) => themes
            .find_palette(&name)
            .map(|palette| palette.theme())
            .unwrap_or_else(Theme::dark),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::completion::domain::test_fixtures::settings;
    use crate::dropdown::domain::repositories::theme_repository::InMemoryThemeRepository;
    use crate::dropdown::domain::value_objects::theme_palette::{Base16PaletteParts, ThemePalette};

    fn with_theme(name: &str) -> Settings {
        settings(&format!(r#"{{"autocomplete.theme": "{name}"}}"#))
    }

    fn palette(main_bg: Rgb) -> ThemePalette {
        ThemePalette::base16(Base16PaletteParts {
            shade0: Some(main_bg),
            shade1: None,
            shade2: None,
            shade5: None,
            shade6: None,
            shade7: None,
            accent6: None,
        })
    }

    #[test]
    fn should_resolve_the_built_in_names_without_asking_the_repository() {
        let themes = InMemoryThemeRepository::default().with_palette("light", palette((1, 2, 3)));
        let backgrounds = [None, Some((250, 250, 250)), Some((10, 10, 30))];

        let resolved = ["dark", "light", "system", ""]
            .map(|name| backgrounds.map(|bg| resolve_theme(&with_theme(name), bg, &themes)));

        let (dark, light) = (Theme::dark(), Theme::light());
        assert_eq!(
            resolved,
            [
                [dark.clone(), dark.clone(), dark.clone()],
                [light.clone(), light.clone(), light.clone()],
                [dark.clone(), light, dark.clone()],
                [dark.clone(), dark.clone(), dark],
            ]
        );
        assert!(themes.asked_palettes().is_empty());
    }

    #[test]
    fn should_resolve_an_empty_name_to_dark() {
        let themes = InMemoryThemeRepository::default();

        let resolved = (
            resolve_theme(&with_theme(""), None, &themes),
            resolve_theme(&Settings::default(), None, &themes),
        );

        assert_eq!(resolved, (Theme::dark(), Theme::dark()));
    }

    #[test]
    fn should_resolve_system_by_the_luma_of_the_terminal_background() {
        let themes = InMemoryThemeRepository::default();
        let system = with_theme("system");
        let is_light = |bg| resolve_theme(&system, bg, &themes) == Theme::light();

        let light = [
            Some((250, 250, 250)),
            Some((128, 128, 128)),
            Some((1, 173, 225)),
        ]
        .map(is_light);
        let dark = [
            None,
            Some((10, 10, 30)),
            Some((127, 127, 127)),
            Some((0, 204, 68)),
        ]
        .map(is_light);

        assert_eq!((light, dark), ([true; 3], [false; 4]));
    }

    #[test]
    fn should_resolve_a_custom_theme_through_its_palette() {
        let themes = InMemoryThemeRepository::default().with_palette("zed", palette((16, 16, 16)));

        let resolved = resolve_theme(&with_theme("zed"), None, &themes);

        assert_eq!(resolved, palette((16, 16, 16)).theme());
        assert_eq!(themes.asked_palettes(), ["zed"]);
    }

    #[test]
    fn should_resolve_a_missing_custom_theme_to_dark() {
        let themes = InMemoryThemeRepository::default();

        let resolved = resolve_theme(&with_theme("zed"), None, &themes);

        assert_eq!(resolved, Theme::dark());
        assert_eq!(themes.asked_palettes(), ["zed"]);
    }

    #[test]
    fn should_resolve_a_custom_theme_named_like_a_built_in_to_the_built_in() {
        let themes = InMemoryThemeRepository::default()
            .with_palette("dark", palette((1, 1, 1)))
            .with_palette("light", palette((2, 2, 2)))
            .with_palette("system", palette((3, 3, 3)));

        let resolved = (
            resolve_theme(&with_theme("light"), None, &themes),
            resolve_theme(&with_theme("dark"), None, &themes),
            resolve_theme(&with_theme("system"), None, &themes),
        );

        assert_eq!(resolved, (Theme::light(), Theme::dark(), Theme::dark()));
        assert!(themes.asked_palettes().is_empty());
    }

    #[test]
    fn colours_and_themes_theme_choice() {
        let theme = |name: &str, bg| {
            resolve_theme(
                &settings(&format!(r#"{{"autocomplete.theme": "{name}"}}"#)),
                bg,
                &InMemoryThemeRepository::default(),
            )
        };
        assert_eq!(theme("light", None), Theme::light());
        assert_eq!(theme("system", Some((250, 250, 250))), Theme::light());
        assert_eq!(theme("system", Some((10, 10, 30))), Theme::dark());
        assert_eq!(theme("system", None), Theme::dark());
        assert_eq!(
            resolve_theme(
                &Settings::default(),
                None,
                &InMemoryThemeRepository::default()
            ),
            Theme::dark()
        );
    }
}
