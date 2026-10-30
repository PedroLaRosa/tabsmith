use crate::dropdown::domain::services::color_parser::parse_color;
use crate::dropdown::domain::value_objects::theme_author;
use crate::dropdown::domain::value_objects::theme_definition::ThemeDefinition;
use crate::dropdown::domain::value_objects::theme_palette::{
    Base16PaletteParts, FigPaletteParts, ThemePalette,
};
use crate::dropdown::infrastructure::adapters::theme_file_error::ThemeFileError;
use serde::Deserialize;
use serde_json::{Map, Value};

#[derive(Deserialize)]
struct ThemeAuthor {
    name: Option<String>,
    twitter: Option<String>,
    github: Option<String>,
}

/// Custom theme file, parsed only to validate it and read the author; drawing reads the colours with its own,
/// lenient parser (8.5d).
#[derive(Deserialize)]
struct ThemeFile {
    author: Option<ThemeAuthor>,
    #[allow(dead_code)]
    version: Option<String>,
    #[allow(dead_code)]
    theme: Option<Map<String, Value>>,
    #[serde(flatten)]
    rest: Map<String, Value>,
}

/// The strict reading of `theme NAME`: the text must be a theme file and every `shade*`/`accent*` key text.
pub(crate) fn parse_theme_definition(text: &str) -> Result<ThemeDefinition, ThemeFileError> {
    let file: ThemeFile =
        serde_json::from_str(text).map_err(|error| ThemeFileError::Json(error.to_string()))?;
    for (key, value) in &file.rest {
        if (key.starts_with("shade") || key.starts_with("accent")) && !value.is_string() {
            return Err(ThemeFileError::ColourNotText(key.clone()));
        }
    }
    Ok(ThemeDefinition::new(file.author.map(|author| {
        theme_author::ThemeAuthor::new(author.name, author.twitter, author.github)
    })))
}

/// Lenient reading for drawing: any JSON value gives a palette, a colour that is missing, not a string or unparsable is `None` for its role only. The Fig format needs `version` to be the string "1.0" AND `theme` an object; everything else is read as base16 (P8-T26).
pub(crate) fn theme_palette_from_json(v: &Value) -> ThemePalette {
    let col = |p: &str| v.pointer(p).and_then(Value::as_str).and_then(parse_color);
    if v.get("version").and_then(Value::as_str) == Some("1.0")
        && v.get("theme").is_some_and(Value::is_object)
    {
        return ThemePalette::fig(FigPaletteParts {
            main_bg: col("/theme/backgroundColor"),
            main_text: col("/theme/textColor"),
            matching_bg: col("/theme/matchBackgroundColor"),
            selected_bg: col("/theme/selection/backgroundColor"),
            selected_text: col("/theme/selection/textColor"),
            selected_matching_bg: col("/theme/selection/matchBackgroundColor"),
            description_text: col("/theme/description/textColor"),
            description_border: col("/theme/description/borderColor"),
        });
    }
    ThemePalette::base16(Base16PaletteParts {
        shade0: col("/shade0"),
        shade1: col("/shade1"),
        shade2: col("/shade2"),
        shade5: col("/shade5"),
        shade6: col("/shade6"),
        shade7: col("/shade7"),
        accent6: col("/accent6"),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dropdown::domain::value_objects::rgb::Rgb;
    use crate::dropdown::domain::value_objects::theme::Theme;
    use serde_json::json;

    fn error_texts(inputs: &[&str]) -> Vec<String> {
        inputs
            .iter()
            .map(|input| parse_theme_definition(input).unwrap_err().to_string())
            .collect()
    }

    #[test]
    fn theme_file_parsing() {
        parse_theme_definition(
            r##"{"author":{"name":"Ann","github":"ann"},"version":"1.0","shade0":"#000000"}"##,
        )
        .unwrap();
        assert!(parse_theme_definition(r#"{"author":"Ann"}"#).is_err());
        assert!(parse_theme_definition(r#"{"shade3":5}"#).is_err());
        assert!(parse_theme_definition("not json").is_err());
    }

    #[test]
    fn should_parse_a_full_author_and_a_name_only_author() {
        let full = r##"{"author":{"name":"Ann","twitter":"@ann","github":"ann"},"version":"1.0","shade0":"#000000"}"##;
        let name_only = r#"{"author":{"name":"Ann"}}"#;

        let authors = [full, name_only].map(|text| {
            let definition = parse_theme_definition(text).unwrap();
            let author = definition.author().unwrap();
            (
                author.name().map(str::to_string),
                author.twitter().map(str::to_string),
                author.github().map(str::to_string),
            )
        });

        assert_eq!(
            authors,
            [
                (
                    Some("Ann".to_string()),
                    Some("@ann".to_string()),
                    Some("ann".to_string())
                ),
                (Some("Ann".to_string()), None, None),
            ]
        );
    }

    #[test]
    fn should_read_a_three_item_author_array_positionally() {
        let definition = parse_theme_definition(r#"{"author":["Ann","@t","gh"]}"#).unwrap();

        let author = definition.author().unwrap();

        assert_eq!(
            (author.name(), author.twitter(), author.github()),
            (Some("Ann"), Some("@t"), Some("gh"))
        );
    }

    #[test]
    fn should_report_the_serde_text_of_a_theme_file_that_is_not_an_object() {
        let inputs = ["[1]", r#""x""#, "null", "5"];

        let texts = error_texts(&inputs);

        assert_eq!(
            texts,
            [
                "invalid type: sequence, expected struct ThemeFile at line 1 column 0",
                "invalid type: string \"x\", expected struct ThemeFile at line 1 column 3",
                "invalid type: null, expected struct ThemeFile at line 1 column 4",
                "invalid type: integer `5`, expected struct ThemeFile at line 1 column 1",
            ]
        );
    }

    #[test]
    fn should_report_the_serde_text_of_a_bad_author() {
        let inputs = [
            r#"{"author":"Ann"}"#,
            r#"{"author":5}"#,
            r#"{"author":["Ann"]}"#,
            r#"{"author":[]}"#,
            r#"{"author":["a","b","c","d"]}"#,
            r#"{"author":{"name":5}}"#,
            r#"{"author":{"twitter":true}}"#,
        ];

        let texts = error_texts(&inputs);

        assert_eq!(
            texts,
            [
                "invalid type: string \"Ann\", expected struct ThemeAuthor at line 1 column 15",
                "invalid type: integer `5`, expected struct ThemeAuthor at line 1 column 11",
                "invalid length 1, expected struct ThemeAuthor with 3 elements at line 1 column 17",
                "invalid length 0, expected struct ThemeAuthor with 3 elements at line 1 column 12",
                "trailing characters at line 1 column 24",
                "invalid type: integer `5`, expected a string at line 1 column 19",
                "invalid type: boolean `true`, expected a string at line 1 column 25",
            ]
        );
    }

    #[test]
    fn should_report_the_serde_text_of_a_version_a_theme_or_a_duplicate_author() {
        let inputs = [
            r#"{"version":1}"#,
            r#"{"version":["x"]}"#,
            r#"{"theme":"x"}"#,
            r#"{"theme":[1]}"#,
            r#"{"author":{},"author":{}}"#,
        ];

        let texts = error_texts(&inputs);

        assert_eq!(
            texts,
            [
                "invalid type: integer `1`, expected a string at line 1 column 12",
                "invalid type: sequence, expected a string at line 1 column 11",
                "invalid type: string \"x\", expected a map at line 1 column 12",
                "invalid type: sequence, expected a map at line 1 column 9",
                "duplicate field `author` at line 1 column 21",
            ]
        );
    }

    #[test]
    fn should_report_the_first_colour_key_that_is_not_a_string() {
        let inputs = [
            r#"{"shade3":5}"#,
            r#"{"shade9":null,"accent1":1,"accent0":2}"#,
            r#"{"shadeX":true}"#,
            r#"{"accent":1}"#,
        ];

        let texts = error_texts(&inputs);

        assert_eq!(
            texts,
            [
                "shade3 must be a string",
                "accent0 must be a string",
                "shadeX must be a string",
                "accent must be a string",
            ]
        );
    }

    #[test]
    fn should_report_broken_json_like_serde_does() {
        let inputs = ["not json", "", "{} x", "\u{feff}{}"];

        let texts = error_texts(&inputs);

        assert_eq!(
            texts,
            [
                "expected ident at line 1 column 2",
                "EOF while parsing a value at line 1 column 0",
                "trailing characters at line 1 column 4",
                "expected value at line 1 column 1",
            ]
        );
    }

    #[test]
    fn should_accept_unknown_keys_and_unparsable_colours() {
        let inputs = [
            "{}",
            r#"{"version":"2.0"}"#,
            r#"{"shade0":"red"}"#,
            r#"{"author":["Ann","@t","gh"]}"#,
        ];

        let has_author =
            inputs.map(|input| parse_theme_definition(input).map(|d| d.author().is_some()));

        assert_eq!(
            format!("{has_author:?}"),
            "[Ok(false), Ok(false), Ok(false), Ok(true)]"
        );
    }

    #[test]
    fn should_map_a_fig_theme_to_its_roles() {
        let value = json!({"version":"1.0","theme":{"backgroundColor":"#010101","textColor":"#020202","matchBackgroundColor":"#030303","selection":{"backgroundColor":"#040404","textColor":"#050505","matchBackgroundColor":"#060606"},"description":{"textColor":"#070707","borderColor":"#080808"}}});

        let palette = theme_palette_from_json(&value);

        assert_eq!(
            palette,
            ThemePalette::fig(FigPaletteParts {
                main_bg: Some((1, 1, 1)),
                main_text: Some((2, 2, 2)),
                matching_bg: Some((3, 3, 3)),
                selected_bg: Some((4, 4, 4)),
                selected_text: Some((5, 5, 5)),
                selected_matching_bg: Some((6, 6, 6)),
                description_text: Some((7, 7, 7)),
                description_border: Some((8, 8, 8)),
            })
        );
    }

    #[test]
    fn colours_and_themes_theme_json() {
        let v1: Value = serde_json::from_str(
            r##"{"version":"1.0","theme":{"backgroundColor":"#000","selection":{"backgroundColor":"rgb(1, 2, 3)"},
                 "description":{"textColor":"nope"}}}"##,
        )
        .unwrap();
        let t = theme_palette_from_json(&v1).theme();
        assert_eq!((t.main_bg(), t.selected_bg()), ((0, 0, 0), (1, 2, 3)));
        assert_eq!(
            (t.main_text(), t.description_text()),
            (Theme::dark().main_text(), Theme::dark().description_text())
        );
        let b16: Value =
            serde_json::from_str(r##"{"shade0":"#101010","accent6":"#123456"}"##).unwrap();
        let t = theme_palette_from_json(&b16).theme();
        assert_eq!(
            (
                t.main_bg(),
                t.matching_bg(),
                t.selected_matching_bg(),
                t.main_text()
            ),
            ((16, 16, 16), (40, 40, 40), (18, 52, 86), (232, 232, 232))
        );
    }

    /// The eight colours of a theme in field order: main_bg, main_text, matching_bg, selected_bg, selected_text,
    /// selected_matching_bg, description_text, description_border.
    fn roles(t: &Theme) -> [Rgb; 8] {
        [
            t.main_bg(),
            t.main_text(),
            t.matching_bg(),
            t.selected_bg(),
            t.selected_text(),
            t.selected_matching_bg(),
            t.description_text(),
            t.description_border(),
        ]
    }

    #[test]
    fn should_draw_a_json_value_that_is_not_a_fig_theme_with_the_base16_filler() {
        let filler = [
            (24, 24, 24),
            (232, 232, 232),
            (40, 40, 40),
            (56, 56, 56),
            (248, 248, 248),
            (186, 139, 175),
            (216, 216, 216),
            (40, 40, 40),
        ];
        let not_fig = [
            json!({}),
            json!([1]),
            json!("x"),
            json!(null),
            json!(5),
            json!({"version": "1.0"}),
            json!({"version": 1, "theme": {}}),
            json!({"version": "1.0", "theme": "x"}),
        ];

        for value in not_fig {
            assert_eq!(roles(&theme_palette_from_json(&value).theme()), filler, "{value}");
        }
    }

    #[test]
    fn should_fall_back_per_colour_for_a_fig_theme_with_missing_or_unparsable_colours() {
        let fig = json!({
            "version": "1.0",
            "theme": {
                "backgroundColor": "#000",
                "matchBackgroundColor": "red",
                "selection": {"backgroundColor": 5, "textColor": null, "matchBackgroundColor": "rgb(1,2)"},
                "description": {"textColor": "#102030"}
            }
        });
        let dark = [
            (48, 48, 48),
            (180, 180, 180),
            (95, 89, 56),
            (30, 90, 199),
            (253, 253, 253),
            (106, 142, 218),
            (180, 180, 180),
            (65, 65, 65),
        ];

        let theme = theme_palette_from_json(&fig).theme();

        assert_eq!(
            roles(&theme),
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
        // a Fig theme without a single colour is dark, not the base16 filler
        let empty = json!({"version": "1.0", "theme": {}});
        assert_eq!(roles(&theme_palette_from_json(&empty).theme()), dark);
    }

    #[test]
    fn should_fall_back_per_colour_for_a_base16_theme_with_missing_or_unparsable_colours() {
        let base16 = json!({
            "shade0": "#101010",
            "shade1": "#0a0b0c",
            "shade5": 5,
            "shade6": null,
            "shade7": "red",
            "accent6": "rgb(1,2)",
            "shade3": "#ffffff",
            "accent0": "#ffffff"
        });

        let theme = theme_palette_from_json(&base16).theme();

        assert_eq!(
            roles(&theme),
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
}
