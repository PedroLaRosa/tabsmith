//! `tabsmith settings`: its clap arguments, the handler and the presenters (SPEC §2.3).
use crate::modules::settings::domain::value_objects::setting_definition::SettingDefinition;
use crate::modules::settings::infrastructure::adapters::setting_value_json::{
    parse_value, setting_value_from_json, setting_value_to_json, settings_to_json,
};
use crate::modules::settings::infrastructure::settings_module::SettingsModule;
use crate::modules::shared::infrastructure::output_format::OutputFormat;
use anyhow::{bail, Result};
use clap::{ArgGroup, Args, Subcommand};
use serde_json::{json, Map, Value};

#[derive(Args)]
#[command(subcommand_negates_reqs = true, args_conflicts_with_subcommands = true)]
#[command(group(ArgGroup::new("vals").requires("key").args(["value", "format"])))]
pub(crate) struct SettingsArgs {
    #[command(subcommand)]
    cmd: Option<SettingsCmd>,
    /// Key of the setting
    key: Option<String>,
    /// Value of the setting (JSON, or a plain string)
    value: Option<String>,
    /// Delete a setting (wildcards * and ? allowed)
    #[arg(long, short)]
    delete: bool,
    /// Output format
    #[arg(long, short, value_enum, default_value_t)]
    format: OutputFormat,
}

#[derive(Subcommand)]
enum SettingsCmd {
    /// Open the settings file
    Open,
    /// List settings
    List {
        /// List all available settings with descriptions
        #[arg(long)]
        all: bool,
        #[arg(long, short, value_enum, default_value_t)]
        format: OutputFormat,
    },
    #[command(hide = true)]
    All {
        #[arg(long, short, value_enum, default_value_t)]
        format: OutputFormat,
    },
}

/// Print `s` + newline unless it is empty (an empty plain list prints nothing).
fn emit(s: &str) {
    if !s.is_empty() {
        println!("{s}");
    }
}

/// `tabsmith settings ...`. A bare `tabsmith settings` calls `print_root_help`, which prints `tabsmith settings
/// --help` the way the root command does (with the global `-v`).
pub(crate) fn run_settings_command(
    args: SettingsArgs,
    module: &SettingsModule,
    print_root_help: impl FnOnce() -> Result<()>,
) -> Result<i32> {
    // Located before anything else, as always: with no home and no TABSMITH_SETTINGS_PATH every form fails here.
    module.locate_settings_file()?;
    match args.cmd {
        Some(SettingsCmd::Open) => {
            let exited_successfully = module.open_settings_file.execute()?;
            return Ok(if exited_successfully { 0 } else { 1 });
        }
        Some(SettingsCmd::List { all, format }) => {
            let map = settings_to_json(&module.list_settings.execute()?);
            emit(&if all {
                render_list_all(&map, module.list_setting_definitions.execute(), format)
            } else {
                render_list(&map, format)
            });
            return Ok(0);
        }
        Some(SettingsCmd::All { format }) => {
            let map = settings_to_json(&module.list_settings.execute()?);
            emit(&render_list(&map, format));
            return Ok(0);
        }
        None => {}
    }
    let Some(key) = args.key else {
        if args.delete {
            bail!("the argument '--delete' requires a <KEY>\n Usage: tabsmith settings --delete <KEY>");
        }
        // Same as `tabsmith settings --help`.
        print_root_help()?;
        return Ok(0);
    };
    match (args.value, args.delete) {
        (Some(_), true) => {
            // Loaded first, as always: settings that cannot be loaded are reported ahead of the conflict.
            module.get_setting.execute(&key)?;
            bail!("the argument '--delete' cannot be used with '[VALUE]'\n Usage: tabsmith settings --delete {key}")
        }
        (None, false) => {
            let value = module.get_setting.execute(&key)?;
            let value = value.as_ref().map(setting_value_to_json);
            println!("{}", render_get(&key, value.as_ref(), args.format)?)
        }
        (Some(value), false) => {
            let value = setting_value_from_json(parse_value(&value));
            module.set_setting.execute(&key, value)?;
        }
        (None, true) => {
            // Printed before the save, as always, so the list shows even when saving fails.
            module
                .delete_settings
                .execute(&key, |keys| println!("{}", removal_message(keys)))?;
        }
    }
    Ok(0)
}

/// `settings KEY`: plain prints strings raw and other values pretty; json formats print `null` when unset.
fn render_get(key: &str, value: Option<&Value>, format: OutputFormat) -> Result<String> {
    Ok(match (value, format) {
        (None, OutputFormat::Plain) => bail!("No value associated with {key}"),
        (None, _) => "null".to_string(),
        (Some(Value::String(s)), OutputFormat::Plain) => s.clone(),
        (Some(v), OutputFormat::Plain | OutputFormat::JsonPretty) => format!("{v:#}"),
        (Some(v), OutputFormat::Json) => v.to_string(),
    })
}

fn removal_message(keys: &[String]) -> String {
    match keys {
        [one] => format!("Removing {one:?}"),
        _ => std::iter::once("Removing:".to_string())
            .chain(keys.iter().map(|k| format!("  - {k}")))
            .collect::<Vec<_>>()
            .join("\n"),
    }
}

/// `settings list`: configured keys (sorted: serde_json's Map is a BTreeMap).
fn render_list(map: &Map<String, Value>, format: OutputFormat) -> String {
    format.render(
        || {
            map.iter()
                .map(|(k, v)| format!("{k} = {v}"))
                .collect::<Vec<_>>()
                .join("\n")
        },
        || Value::Object(map.clone()),
    )
}

/// `settings list --all`: every catalog key (SPEC §3.3), in catalog order, with its current value.
fn render_list_all(
    map: &Map<String, Value>,
    catalog: &[SettingDefinition],
    format: OutputFormat,
) -> String {
    format.render(
        || {
            let mut out = String::from("key\ttype\tdefault\tvalue");
            for e in catalog {
                let value = map.get(e.key()).map(Value::to_string).unwrap_or_default();
                out += &format!("\n{}\t{}\t{}\t{value}", e.key(), e.value_type(), e.default());
            }
            out
        },
        || {
            catalog
                .iter()
                .map(|e| {
                    json!({
                        "key": e.key(),
                        "type": e.value_type(),
                        "default": serde_json::from_str::<Value>(e.default()).unwrap_or(Value::Null),
                        "description": e.description(),
                        "value": map.get(e.key()),
                    })
                })
                .collect()
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::settings::domain::services::settings_catalog;
    use crate::modules::settings::infrastructure::settings_module::build_settings_module;
    use crate::modules::shared::application::ports::clock::FixedClock;
    use crate::modules::shared::domain::value_objects::data_directory::DataDirectory;
    use crate::modules::shared::infrastructure::shared_dependencies::SharedDependencies;
    use crate::modules::shared::infrastructure::testing::temp_dir::TempDir;
    use std::cell::Cell;
    use std::sync::Arc;

    fn map(v: Value) -> Map<String, Value> {
        v.as_object().unwrap().clone()
    }

    fn module_over(data_directory: DataDirectory) -> SettingsModule {
        build_settings_module(&SharedDependencies {
            data_directory,
            clock: Arc::new(FixedClock::new(0)),
        })
    }

    fn without_home() -> SettingsModule {
        module_over(DataDirectory::new(None, None))
    }

    fn settings_args(key: Option<&str>, value: Option<&str>, delete: bool) -> SettingsArgs {
        SettingsArgs {
            cmd: None,
            key: key.map(String::from),
            value: value.map(String::from),
            delete,
            format: OutputFormat::default(),
        }
    }

    /// The bare `settings` and `settings -d` lines of the Phase 0 capture: no home fails them before anything else.
    #[test]
    fn should_report_the_unknown_home_for_a_bare_settings_without_printing_the_help() {
        let help_printed = Cell::new(false);

        let error = run_settings_command(settings_args(None, None, false), &without_home(), || {
            help_printed.set(true);
            Ok(())
        })
        .unwrap_err();

        assert_eq!(
            (format!("{error:#}"), help_printed.get()),
            (
                "could not determine the home directory (set HOME or TABSMITH_HOME)".to_string(),
                false
            )
        );
    }

    #[test]
    fn should_report_the_unknown_home_ahead_of_a_delete_with_no_key() {
        let module = without_home();

        let error =
            run_settings_command(settings_args(None, None, true), &module, || Ok(())).unwrap_err();

        assert_eq!(
            format!("{error:#}"),
            "could not determine the home directory (set HOME or TABSMITH_HOME)"
        );
    }

    #[test]
    fn should_print_the_settings_help_for_a_bare_settings() {
        let temp_dir = TempDir::new("settings-command-help");
        let module = module_over(DataDirectory::new(Some(temp_dir.path().to_path_buf()), None));
        let help_printed = Cell::new(false);

        let exit_code = run_settings_command(settings_args(None, None, false), &module, || {
            help_printed.set(true);
            Ok(())
        })
        .unwrap();

        assert_eq!((exit_code, help_printed.get()), (0, true));
    }

    #[test]
    fn should_refuse_a_value_given_with_delete() {
        let temp_dir = TempDir::new("settings-command-value-with-delete");
        let module = module_over(DataDirectory::new(Some(temp_dir.path().to_path_buf()), None));

        let error =
            run_settings_command(settings_args(Some("k"), Some("v"), true), &module, || Ok(()))
                .unwrap_err();

        assert_eq!(
            format!("{error:#}"),
            "the argument '--delete' cannot be used with '[VALUE]'\n Usage: tabsmith settings --delete k"
        );
    }

    #[test]
    fn should_report_a_corrupted_settings_file_ahead_of_a_value_given_with_delete() {
        let temp_dir = TempDir::new("settings-command-conflict");
        let path = temp_dir.path().join("settings.json");
        std::fs::write(&path, "{oops").unwrap();
        let module = module_over(DataDirectory::new(Some(temp_dir.path().to_path_buf()), None));

        let error =
            run_settings_command(settings_args(Some("k"), Some("v"), true), &module, || Ok(()))
                .unwrap_err();

        assert_eq!(
            format!("{error:#}"),
            format!(
                "{} is corrupted: key must be a string at line 1 column 2",
                path.display()
            )
        );
    }

    #[test]
    fn settings_get_formats() {
        let s = json!("dark");
        let a = json!(["git", "npm"]);
        assert_eq!(
            render_get("k", Some(&s), OutputFormat::Plain).unwrap(),
            "dark"
        );
        assert_eq!(
            render_get("k", Some(&s), OutputFormat::Json).unwrap(),
            "\"dark\""
        );
        assert_eq!(
            render_get("k", Some(&a), OutputFormat::Json).unwrap(),
            "[\"git\",\"npm\"]"
        );
        assert_eq!(
            render_get("k", Some(&a), OutputFormat::Plain).unwrap(),
            "[\n  \"git\",\n  \"npm\"\n]"
        );
        assert_eq!(render_get("k", None, OutputFormat::Json).unwrap(), "null");
        assert_eq!(
            render_get("k", None, OutputFormat::Plain)
                .unwrap_err()
                .to_string(),
            "No value associated with k"
        );
    }

    /// The message half of `settings_delete_wildcards`; its matching half lives with `DeleteSettingsUseCase`.
    #[test]
    fn settings_delete_wildcards_removal_message() {
        assert_eq!(removal_message(&["a.b".into()]), "Removing \"a.b\"");
        assert_eq!(
            removal_message(&["a".into(), "b".into()]),
            "Removing:\n  - a\n  - b"
        );
    }

    #[test]
    fn settings_list_formats() {
        let m = map(json!({"b.key": [1], "a.key": "x"}));
        assert_eq!(
            render_list(&m, OutputFormat::Plain),
            "a.key = \"x\"\nb.key = [1]"
        );
        assert_eq!(
            render_list(&m, OutputFormat::Json),
            r#"{"a.key":"x","b.key":[1]}"#
        );
        assert_eq!(render_list(&Map::new(), OutputFormat::Plain), "");

        let m = map(json!({"autocomplete.theme": "light"}));
        let all = render_list_all(&m, settings_catalog::catalog(), OutputFormat::Plain);
        let lines: Vec<&str> = all.lines().collect();
        assert_eq!(lines[0], "key\ttype\tdefault\tvalue");
        assert_eq!(lines.len(), settings_catalog::catalog().len() + 1);
        assert!(lines.contains(&"autocomplete.theme\tstring\t\"dark\"\t\"light\""));
        assert!(lines.contains(&"autocomplete.width\tnumber (columns)\t40\t"));

        let v: Value = serde_json::from_str(&render_list_all(
            &m,
            settings_catalog::catalog(),
            OutputFormat::Json,
        ))
        .unwrap();
        let theme = v
            .as_array()
            .unwrap()
            .iter()
            .find(|e| e["key"] == "autocomplete.theme")
            .unwrap();
        assert_eq!(theme["default"], json!("dark"));
        assert_eq!(theme["value"], json!("light"));
        assert!(theme["description"].is_string());
        let width = v
            .as_array()
            .unwrap()
            .iter()
            .find(|e| e["key"] == "autocomplete.width")
            .unwrap();
        assert_eq!(
            (width["default"].clone(), width["value"].clone()),
            (json!(40), Value::Null)
        );
    }
}
