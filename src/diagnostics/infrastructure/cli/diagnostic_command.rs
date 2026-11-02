use crate::diagnostics::domain::value_objects::diagnostic_report::DiagnosticReport;
use crate::diagnostics::domain::value_objects::diagnostic_value::DiagnosticValue;
use crate::diagnostics::infrastructure::diagnostics_module::DiagnosticsModule;
use crate::shared::infrastructure::output_format::OutputFormat;
use clap::Args;
use serde_json::{Map, Value};

#[derive(Args)]
pub(crate) struct DiagnosticArgs {
    #[arg(long, short, value_enum, default_value_t)]
    pub(crate) format: OutputFormat,
}

/// `debug diagnostic`: everything is gathered before the single print.
pub(crate) fn run_diagnostic_command(args: DiagnosticArgs, module: &DiagnosticsModule) -> anyhow::Result<i32> {
    let report = module.collect_diagnostic_report.execute();
    println!("{}", args.format.render(|| to_toml(&report), || json(&report)));
    Ok(0)
}

/// Hand-written TOML: JSON scalars are valid TOML values; rows without a value are omitted.
fn to_toml(report: &DiagnosticReport) -> String {
    let mut out = String::new();
    for section in report.sections() {
        out += &format!("[{}]\n", section.name());
        for (key, value) in section.rows() {
            if let Some(value) = toml_value(value) {
                out += &format!("{} = {value}\n", toml_key(key));
            }
        }
        out.push('\n');
    }
    out.trim_end().to_string()
}

fn toml_key(key: &str) -> String {
    let is_bare = key
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-');
    if is_bare {
        key.to_string()
    } else {
        Value::String(key.to_string()).to_string()
    }
}

fn toml_value(value: &DiagnosticValue) -> Option<String> {
    match value {
        DiagnosticValue::Text(text) => Some(Value::String(text.clone()).to_string()),
        DiagnosticValue::Flag(flag) => Some(flag.to_string()),
        DiagnosticValue::Missing => None,
    }
}

/// Built through `serde_json::Map` on purpose: its keys come out alphabetical and the missing rows stay as null.
fn json(report: &DiagnosticReport) -> Value {
    let sections: Map<String, Value> = report
        .sections()
        .iter()
        .map(|section| {
            let rows: Map<String, Value> = section
                .rows()
                .iter()
                .map(|(key, value)| (key.clone(), json_value(value)))
                .collect();
            (section.name().to_string(), Value::Object(rows))
        })
        .collect();
    Value::Object(sections)
}

fn json_value(value: &DiagnosticValue) -> Value {
    match value {
        DiagnosticValue::Text(text) => Value::String(text.clone()),
        DiagnosticValue::Flag(flag) => Value::Bool(*flag),
        DiagnosticValue::Missing => Value::Null,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diagnostics::domain::value_objects::diagnostic_section::DiagnosticSection;
    use crate::diagnostics::domain::value_objects::diagnostic_value::DiagnosticValue;
    use crate::shared::infrastructure::output_format::OutputFormat;
    use clap::Parser;

    #[derive(Parser)]
    struct DiagnosticOnly {
        #[command(flatten)]
        args: DiagnosticArgs,
    }

    fn text(value: &str) -> DiagnosticValue {
        DiagnosticValue::Text(value.to_string())
    }

    fn section(name: &str, rows: Vec<(&str, DiagnosticValue)>) -> DiagnosticSection {
        let rows = rows.into_iter().map(|(key, value)| (key.to_string(), value)).collect();
        DiagnosticSection::new(name.to_string(), rows)
    }

    fn report_of(sections: Vec<DiagnosticSection>) -> DiagnosticReport {
        DiagnosticReport::new(sections)
    }

    fn toml_of_one_section(rows: Vec<(&str, DiagnosticValue)>) -> String {
        to_toml(&report_of(vec![section("s", rows)]))
    }

    fn installation_report() -> DiagnosticReport {
        report_of(vec![
            section(
                "tabsmith",
                vec![("version", text("0.1.0")), ("os", text("linux x86_64"))],
            ),
            section(
                "environment",
                vec![
                    ("terminal", text("kitty")),
                    ("shell-version", DiagnosticValue::Missing),
                    ("in-ci", DiagnosticValue::Flag(false)),
                ],
            ),
            section("env-vars", vec![("SHELL", text("/bin/zsh")), ("COLORTERM", text("truecolor"))]),
        ])
    }

    #[test]
    fn should_render_the_sections_in_insertion_order_with_the_legacy_text() {
        let report = report_of(vec![
            section(
                "tabsmith",
                vec![("version", text("0.1.0")), ("missing", DiagnosticValue::Missing)],
            ),
            section(
                "env-vars",
                vec![("PATH", text("/a:\"b\"")), ("in-ci", DiagnosticValue::Flag(false))],
            ),
        ]);

        let rendered = to_toml(&report);

        assert_eq!(
            rendered,
            "[tabsmith]\nversion = \"0.1.0\"\n\n[env-vars]\nPATH = \"/a:\\\"b\\\"\"\nin-ci = false"
        );
    }

    #[test]
    fn diagnostic_toml() {
        let sections = vec![
            section(
                "tabsmith",
                vec![("version", text("0.1.0")), ("missing", DiagnosticValue::Missing)],
            ),
            section(
                "env-vars",
                vec![("PATH", text("/a:\"b\"")), ("in-ci", DiagnosticValue::Flag(false))],
            ),
        ];
        assert_eq!(
            to_toml(&report_of(sections)),
            "[tabsmith]\nversion = \"0.1.0\"\n\n[env-vars]\nPATH = \"/a:\\\"b\\\"\"\nin-ci = false"
        );
    }

    #[test]
    fn should_quote_a_key_that_is_not_bare() {
        let rendered = toml_of_one_section(vec![("a b", text("x"))]);

        assert_eq!(rendered, "[s]\n\"a b\" = \"x\"");
    }

    #[test]
    fn should_keep_letters_digits_underscores_and_dashes_bare_in_a_key() {
        let rendered = toml_of_one_section(vec![("Az_09-x", DiagnosticValue::Flag(true))]);

        assert_eq!(rendered, "[s]\nAz_09-x = true");
    }

    #[test]
    fn should_treat_an_empty_key_as_bare() {
        let rendered = toml_of_one_section(vec![("", text("x"))]);

        assert_eq!(rendered, "[s]\n = \"x\"");
    }

    #[test]
    fn should_print_an_empty_last_section_as_its_name_alone() {
        let report = report_of(vec![section("tabsmith", vec![("version", text("1"))]), section("env-vars", vec![])]);

        let rendered = to_toml(&report);

        assert_eq!(rendered, "[tabsmith]\nversion = \"1\"\n\n[env-vars]");
    }

    #[test]
    fn should_drop_the_missing_rows_from_the_toml() {
        let rendered = toml_of_one_section(vec![
            ("a", DiagnosticValue::Missing),
            ("b", text("1")),
            ("c", DiagnosticValue::Missing),
        ]);

        assert_eq!(rendered, "[s]\nb = \"1\"");
    }

    #[test]
    fn should_escape_quotes_backslashes_and_control_characters_in_a_string() {
        let rendered = toml_of_one_section(vec![("k", text("a\"b\\c\nd\te\u{1}"))]);

        assert_eq!(rendered, "[s]\nk = \"a\\\"b\\\\c\\nd\\te\\u0001\"");
    }

    #[test]
    fn should_leave_non_ascii_characters_and_slashes_unescaped_in_a_string() {
        let rendered = toml_of_one_section(vec![("k", text("/home/zoë"))]);

        assert_eq!(rendered, "[s]\nk = \"/home/zoë\"");
    }

    #[test]
    fn should_print_compact_json_with_alphabetical_keys_and_the_missing_rows_as_null() {
        let report = installation_report();

        let rendered = OutputFormat::Json.render(|| unreachable!("json asked"), || json(&report));

        assert_eq!(
            rendered,
            "{\"env-vars\":{\"COLORTERM\":\"truecolor\",\"SHELL\":\"/bin/zsh\"},\
             \"environment\":{\"in-ci\":false,\"shell-version\":null,\"terminal\":\"kitty\"},\
             \"tabsmith\":{\"os\":\"linux x86_64\",\"version\":\"0.1.0\"}}"
        );
    }

    #[test]
    fn should_indent_the_pretty_json_with_two_spaces() {
        let report = report_of(vec![section("tabsmith", vec![("version", text("1"))])]);

        let rendered = OutputFormat::JsonPretty.render(|| unreachable!("json asked"), || json(&report));

        assert_eq!(rendered, "{\n  \"tabsmith\": {\n    \"version\": \"1\"\n  }\n}");
    }

    #[test]
    fn should_render_toml_for_the_plain_format() {
        let report = report_of(vec![section("tabsmith", vec![("version", text("1"))])]);

        let rendered = OutputFormat::Plain.render(|| to_toml(&report), || json(&report));

        assert_eq!(rendered, "[tabsmith]\nversion = \"1\"");
    }

    #[test]
    fn should_default_the_format_to_plain_and_read_it_from_its_short_name() {
        let by_default = DiagnosticOnly::parse_from(["diagnostic"]);
        let by_short_name = DiagnosticOnly::parse_from(["diagnostic", "-f", "json-pretty"]);

        assert_eq!(
            (by_default.args.format, by_short_name.args.format),
            (OutputFormat::Plain, OutputFormat::JsonPretty)
        );
    }
}
