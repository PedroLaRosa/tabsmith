use crate::settings::domain::value_objects::setting_definition::SettingDefinition;

/// `default` is a JSON literal, or empty when the key has none (templates like `<name>`).
static CATALOG: &[SettingDefinition] = &[
    SettingDefinition::new(
        "autocomplete.disable",
        "bool",
        "false",
        "Never show the dropdown (ghost text unaffected)",
    ),
    SettingDefinition::new(
        "autocomplete.insertSpaceAutomatically",
        "bool",
        "true",
        "Append a space after an inserted subcommand/option that takes an argument",
    ),
    SettingDefinition::new(
        "autocomplete.immediatelyExecuteAfterSpace",
        "bool",
        "false",
        "With an empty search term, prepend an \"Immediately execute\" row",
    ),
    SettingDefinition::new(
        "autocomplete.scrollWrapAround",
        "bool",
        "false",
        "Up/Down wrap around the list",
    ),
    SettingDefinition::new(
        "autocomplete.alwaysSuggestCurrentToken",
        "bool",
        "false",
        "Prepend a row that executes the typed token",
    ),
    SettingDefinition::new(
        "autocomplete.fuzzySearch",
        "bool",
        "false",
        "Fuzzy instead of prefix filtering",
    ),
    SettingDefinition::new(
        "autocomplete.preferVerboseSuggestions",
        "bool",
        "false",
        "Insert the longest alias of an option/subcommand (--message over -m)",
    ),
    SettingDefinition::new(
        "autocomplete.onlyShowOnTab",
        "bool",
        "false",
        "Never auto-open the menu; Tab opens it",
    ),
    SettingDefinition::new(
        "autocomplete.disableForCommands",
        "string[]",
        "[]",
        "Commands that never get completions",
    ),
    SettingDefinition::new(
        "autocomplete.sortMethod",
        "\"most recent\"|\"alphabetical\"",
        "\"most recent\"",
        "Boost recently used suggestions unless alphabetical",
    ),
    SettingDefinition::new(
        "autocomplete.scriptTimeout",
        "number (ms)",
        if cfg!(windows) { "20000" } else { "5000" },
        "Generator script timeout floor",
    ),
    SettingDefinition::new(
        "autocomplete.immediatelyRunDangerousCommands",
        "bool",
        "false",
        "Allow the execute row for dangerous exact matches",
    ),
    SettingDefinition::new(
        "autocomplete.firstTokenCompletion",
        "bool",
        "false",
        "Complete the command name itself",
    ),
    SettingDefinition::new(
        "autocomplete.hideAutoExecuteSuggestion",
        "bool",
        "false",
        "Never show execute rows",
    ),
    SettingDefinition::new(
        "autocomplete.navigateToHistory",
        "bool",
        "false",
        "Up on the first row toggles history mode instead of hiding",
    ),
    SettingDefinition::new(
        "autocomplete.alwaysShowDescription",
        "bool",
        "false",
        "Keep the description pop-out open",
    ),
    SettingDefinition::new(
        "autocomplete.theme",
        "string",
        "\"dark\"",
        "dark, light, system, or a file stem in ~/.tabsmith/themes",
    ),
    SettingDefinition::new(
        "autocomplete.width",
        "number (columns)",
        "40",
        "Menu width, clamped to the terminal width",
    ),
    SettingDefinition::new(
        "autocomplete.height",
        "number (rows)",
        "7",
        "Max visible list rows",
    ),
    SettingDefinition::new(
        "autocomplete.developerMode",
        "bool",
        "false",
        "Load specs from devCompletionsFolder first; disable spec caches",
    ),
    SettingDefinition::new(
        "autocomplete.devCompletionsFolder",
        "string|null",
        "null",
        "Dev spec folder",
    ),
    SettingDefinition::new(
        "autocomplete.keybindings.<keystroke>",
        "string",
        "",
        "Action id or \"ignore\"",
    ),
    SettingDefinition::new(
        "beta.history.mode",
        "\"off\"|\"history_only\"|\"show\"",
        "\"off\"",
        "History rows in the dropdown",
    ),
    SettingDefinition::new("inline.enabled", "bool", "true", "Ghost text on/off"),
    SettingDefinition::new(
        "inline.source",
        "string",
        "\"history+specs\"",
        "history, specs, history+specs or an inline.profiles name",
    ),
    SettingDefinition::new(
        "inline.profiles.<name>",
        "object",
        "",
        "LLM profile {provider, model, base_url?, api_key_env?, max_tokens?, effort?}",
    ),
    SettingDefinition::new(
        "inline.debounceMs",
        "number",
        "300",
        "Quiet time before an LLM request (ms)",
    ),
    SettingDefinition::new(
        "inline.historyCount",
        "number",
        "49",
        "History lines sent to the LLM",
    ),
];

/// Every documented setting (SPEC §3.3).
pub(crate) fn catalog() -> &'static [SettingDefinition] {
    CATALOG
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;

    #[test]
    fn catalog_keys_unique_and_defaults_valid() {
        let mut keys: Vec<_> = catalog().iter().map(|e| e.key()).collect();
        let n = keys.len();
        keys.sort();
        keys.dedup();
        assert_eq!(keys.len(), n);
        for e in catalog() {
            assert!(
                e.default().is_empty() || serde_json::from_str::<Value>(e.default()).is_ok(),
                "{}",
                e.key()
            );
        }
        for k in [
            "autocomplete.theme",
            "autocomplete.height",
            "beta.history.mode",
            "inline.source",
            "inline.historyCount",
        ] {
            assert!(catalog().iter().any(|e| e.key() == k), "{k}");
        }
    }
}
