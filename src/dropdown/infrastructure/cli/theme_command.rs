//! `tabsmith theme`: its clap arguments, the handler and the banner (SPEC §2.4).

use crate::dropdown::application::current_theme::CurrentTheme;
use crate::dropdown::application::theme_selection::ThemeSelection;
use crate::dropdown::domain::value_objects::theme_definition::ThemeDefinition;
use crate::dropdown::infrastructure::dropdown_module::DropdownModule;
use crate::settings::domain::value_objects::setting_number::SettingNumber;
use crate::settings::domain::value_objects::setting_value::SettingValue;
use crate::shared::infrastructure::terminal_style::{bold, stdout_tty};
use anyhow::Result;
use clap::Args;
use serde_json::Value;

#[derive(Args)]
pub(crate) struct ThemeArgs {
    /// List all available themes
    #[arg(long, conflicts_with_all = ["folder", "theme"])]
    list: bool,
    /// Print the theme directory path
    #[arg(long, conflicts_with = "theme")]
    folder: bool,
    /// Theme name: dark, light, system, or a custom theme
    theme: Option<String>,
}

/// Runs `theme`. The themes folder comes first, so an unknown home fails even `--list`, before any settings load.
pub(crate) fn run_theme_command(args: ThemeArgs, module: &DropdownModule) -> Result<i32> {
    let folder = module.themes.folder()?;
    if args.folder {
        std::fs::create_dir_all(&folder)?;
        println!("{}", folder.display());
        return Ok(0);
    }
    if args.list {
        module
            .list_themes
            .execute()
            .iter()
            .for_each(|t| println!("{t}"));
        return Ok(0);
    }
    let Some(name) = args.theme else {
        println!("{}", render_current_theme(&module.show_theme.execute()?));
        return Ok(0);
    };
    let color = stdout_tty();
    module
        .select_theme
        .execute(&name, |selection| match selection {
            ThemeSelection::Custom(definition) => {
                print!("{}", theme_banner(&name, definition, color))
            }
            ThemeSelection::BuiltIn(_) => println!("› Switching to theme '{}'", bold(&name, color)),
        })?;
    Ok(0)
}

/// The saved name as it is, or a saved value that is not text as the settings command prints it.
fn render_current_theme(current: &CurrentTheme) -> String {
    match current {
        CurrentTheme::Named(name) => name.clone(),
        CurrentTheme::NotText(value) => format!("{:#}", setting_value_to_json(value)),
    }
}

fn theme_banner(name: &str, definition: &ThemeDefinition, color: bool) -> String {
    let mut out = format!("\n› Switching to theme '{}'", bold(name, color));
    if let Some(author) = definition.author() {
        if let Some(n) = author.name() {
            out += &format!(" by {}", bold(n, color));
        }
        if let Some(t) = author.twitter() {
            out += &format!("\n  🐦 {t}");
        }
        if let Some(g) = author.github() {
            out += &format!("\n  💻 github.com/{g}");
        }
    }
    out + "\n\n"
}

/// The settings adapter holds the same conversion; the architecture rules bar importing it from here.
fn setting_value_to_json(value: &SettingValue) -> Value {
    match value {
        SettingValue::Null => Value::Null,
        SettingValue::Bool(value) => Value::Bool(*value),
        SettingValue::Number(number) => json_number(number),
        SettingValue::Text(text) => Value::String(text.clone()),
        SettingValue::List(items) => {
            Value::Array(items.iter().map(setting_value_to_json).collect())
        }
        SettingValue::Object(fields) => Value::Object(
            fields
                .iter()
                .map(|(key, value)| (key.clone(), setting_value_to_json(value)))
                .collect(),
        ),
    }
}

fn json_number(number: &SettingNumber) -> Value {
    number
        .as_u64()
        .map(Value::from)
        .or_else(|| number.as_i64().map(Value::from))
        .or_else(|| number.as_f64().map(Value::from))
        .expect("every setting number reads as an f64")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dropdown::infrastructure::adapters::theme_json_mapper::parse_theme_definition;
    use crate::settings::infrastructure::adapters::setting_value_json;
    use std::collections::BTreeMap;

    fn number(value: impl Into<SettingNumber>) -> SettingValue {
        SettingValue::Number(value.into())
    }

    fn float(value: f64) -> SettingValue {
        SettingValue::Number(SettingNumber::from_f64(value).unwrap())
    }

    fn text(value: &str) -> SettingValue {
        SettingValue::Text(value.to_string())
    }

    fn object(fields: Vec<(&str, SettingValue)>) -> SettingValue {
        SettingValue::Object(
            fields
                .into_iter()
                .map(|(key, value)| (key.to_string(), value))
                .collect::<BTreeMap<_, _>>(),
        )
    }

    fn banner(theme_file: &str, color: bool) -> String {
        let definition = parse_theme_definition(theme_file).unwrap();
        theme_banner("ocean", &definition, color)
    }

    #[test]
    fn theme_file_parsing_banner() {
        let f = parse_theme_definition(
            r##"{"author":{"name":"Ann","github":"ann"},"version":"1.0","shade0":"#000000"}"##,
        )
        .unwrap();
        assert_eq!(
            theme_banner("ocean", &f, false),
            "\n› Switching to theme 'ocean' by Ann\n  💻 github.com/ann\n\n"
        );
        assert_eq!(
            theme_banner("ocean", &parse_theme_definition("{}").unwrap(), false),
            "\n› Switching to theme 'ocean'\n\n"
        );
    }

    #[test]
    fn should_print_a_non_text_setting_value_like_the_settings_command() {
        let values = [
            SettingValue::Null,
            number(5u64),
            float(5.0),
            number(-3i64),
            SettingValue::List(vec![text("a"), number(1u64)]),
            object(vec![
                ("b", number(1u64)),
                ("a", SettingValue::List(vec![number(1u64), number(2u64)])),
            ]),
        ];

        let shown = values.map(|value| render_current_theme(&CurrentTheme::NotText(value)));

        assert_eq!(
            shown,
            [
                "null",
                "5",
                "5.0",
                "-3",
                "[\n  \"a\",\n  1\n]",
                "{\n  \"a\": [\n    1,\n    2\n  ],\n  \"b\": 1\n}",
            ]
        );
    }

    #[test]
    fn should_convert_every_setting_value_like_the_settings_json_adapter() {
        let corpus = [
            SettingValue::Null,
            SettingValue::Bool(true),
            SettingValue::Bool(false),
            number(0u64),
            number(5u64),
            number(u64::MAX),
            number(-3i64),
            number(i64::MIN),
            float(5.0),
            float(0.1),
            float(1.7976931348623157e308),
            text("dark"),
            SettingValue::List(vec![text("a"), number(1u64), SettingValue::List(vec![])]),
            object(vec![
                ("b", number(1u64)),
                (
                    "a",
                    SettingValue::List(vec![float(2.5), SettingValue::Null]),
                ),
            ]),
        ];

        let converted = corpus.each_ref().map(setting_value_to_json);

        let by_the_settings_adapter = corpus
            .each_ref()
            .map(setting_value_json::setting_value_to_json);
        assert_eq!(converted, by_the_settings_adapter);
    }

    #[test]
    fn should_write_the_author_lines_of_the_banner() {
        let text = banner(
            r#"{"author":{"name":"Ann","twitter":"@ann","github":"ann"}}"#,
            false,
        );

        assert_eq!(
            text,
            "\n› Switching to theme 'ocean' by Ann\n  🐦 @ann\n  💻 github.com/ann\n\n"
        );
    }

    #[test]
    fn should_write_only_the_theme_name_when_the_theme_has_no_author() {
        let text = banner("{}", false);

        assert_eq!(text, "\n› Switching to theme 'ocean'\n\n");
    }

    #[test]
    fn should_bold_the_theme_name_and_the_author_name_on_a_terminal() {
        let text = banner(r#"{"author":{"name":"Ann","twitter":"@ann"}}"#, true);

        assert_eq!(
            text,
            "\n› Switching to theme '\x1b[1mocean\x1b[0m' by \x1b[1mAnn\x1b[0m\n  🐦 @ann\n\n"
        );
    }
}
