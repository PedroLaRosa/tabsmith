use crate::dropdown::domain::value_objects::theme_author;
use crate::dropdown::domain::value_objects::theme_definition::ThemeDefinition;
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

#[cfg(test)]
mod tests {
    use super::*;

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
}
