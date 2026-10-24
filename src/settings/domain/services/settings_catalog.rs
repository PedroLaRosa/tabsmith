use crate::settings::domain::value_objects::setting_definition::SettingDefinition;

/// The documented keys, in catalog order; the template rows (`<keystroke>`, `<name>`) have none.
pub(crate) const AUTOCOMPLETE_DISABLE: &str = "autocomplete.disable";
pub(crate) const AUTOCOMPLETE_INSERT_SPACE_AUTOMATICALLY: &str =
    "autocomplete.insertSpaceAutomatically";
pub(crate) const AUTOCOMPLETE_IMMEDIATELY_EXECUTE_AFTER_SPACE: &str =
    "autocomplete.immediatelyExecuteAfterSpace";
pub(crate) const AUTOCOMPLETE_SCROLL_WRAP_AROUND: &str = "autocomplete.scrollWrapAround";
pub(crate) const AUTOCOMPLETE_ALWAYS_SUGGEST_CURRENT_TOKEN: &str =
    "autocomplete.alwaysSuggestCurrentToken";
pub(crate) const AUTOCOMPLETE_FUZZY_SEARCH: &str = "autocomplete.fuzzySearch";
pub(crate) const AUTOCOMPLETE_PREFER_VERBOSE_SUGGESTIONS: &str =
    "autocomplete.preferVerboseSuggestions";
pub(crate) const AUTOCOMPLETE_ONLY_SHOW_ON_TAB: &str = "autocomplete.onlyShowOnTab";
pub(crate) const AUTOCOMPLETE_DISABLE_FOR_COMMANDS: &str = "autocomplete.disableForCommands";
pub(crate) const AUTOCOMPLETE_SORT_METHOD: &str = "autocomplete.sortMethod";
pub(crate) const AUTOCOMPLETE_SCRIPT_TIMEOUT: &str = "autocomplete.scriptTimeout";
pub(crate) const AUTOCOMPLETE_IMMEDIATELY_RUN_DANGEROUS_COMMANDS: &str =
    "autocomplete.immediatelyRunDangerousCommands";
pub(crate) const AUTOCOMPLETE_FIRST_TOKEN_COMPLETION: &str = "autocomplete.firstTokenCompletion";
pub(crate) const AUTOCOMPLETE_HIDE_AUTO_EXECUTE_SUGGESTION: &str =
    "autocomplete.hideAutoExecuteSuggestion";
pub(crate) const AUTOCOMPLETE_NAVIGATE_TO_HISTORY: &str = "autocomplete.navigateToHistory";
pub(crate) const AUTOCOMPLETE_ALWAYS_SHOW_DESCRIPTION: &str = "autocomplete.alwaysShowDescription";
pub(crate) const AUTOCOMPLETE_THEME: &str = "autocomplete.theme";
pub(crate) const AUTOCOMPLETE_WIDTH: &str = "autocomplete.width";
pub(crate) const AUTOCOMPLETE_HEIGHT: &str = "autocomplete.height";
pub(crate) const AUTOCOMPLETE_DEVELOPER_MODE: &str = "autocomplete.developerMode";
pub(crate) const AUTOCOMPLETE_DEV_COMPLETIONS_FOLDER: &str = "autocomplete.devCompletionsFolder";
pub(crate) const BETA_HISTORY_MODE: &str = "beta.history.mode";
pub(crate) const INLINE_ENABLED: &str = "inline.enabled";
pub(crate) const INLINE_SOURCE: &str = "inline.source";
pub(crate) const INLINE_DEBOUNCE_MS: &str = "inline.debounceMs";
pub(crate) const INLINE_HISTORY_COUNT: &str = "inline.historyCount";

/// `default` is a JSON literal, or empty when the key has none (templates like `<name>`).
static CATALOG: &[SettingDefinition] = &[
    SettingDefinition::new(
        AUTOCOMPLETE_DISABLE,
        "bool",
        "false",
        "Never show the dropdown (ghost text unaffected)",
    ),
    SettingDefinition::new(
        AUTOCOMPLETE_INSERT_SPACE_AUTOMATICALLY,
        "bool",
        "true",
        "Append a space after an inserted subcommand/option that takes an argument",
    ),
    SettingDefinition::new(
        AUTOCOMPLETE_IMMEDIATELY_EXECUTE_AFTER_SPACE,
        "bool",
        "false",
        "With an empty search term, prepend an \"Immediately execute\" row",
    ),
    SettingDefinition::new(
        AUTOCOMPLETE_SCROLL_WRAP_AROUND,
        "bool",
        "false",
        "Up/Down wrap around the list",
    ),
    SettingDefinition::new(
        AUTOCOMPLETE_ALWAYS_SUGGEST_CURRENT_TOKEN,
        "bool",
        "false",
        "Prepend a row that executes the typed token",
    ),
    SettingDefinition::new(
        AUTOCOMPLETE_FUZZY_SEARCH,
        "bool",
        "false",
        "Fuzzy instead of prefix filtering",
    ),
    SettingDefinition::new(
        AUTOCOMPLETE_PREFER_VERBOSE_SUGGESTIONS,
        "bool",
        "false",
        "Insert the longest alias of an option/subcommand (--message over -m)",
    ),
    SettingDefinition::new(
        AUTOCOMPLETE_ONLY_SHOW_ON_TAB,
        "bool",
        "false",
        "Never auto-open the menu; Tab opens it",
    ),
    SettingDefinition::new(
        AUTOCOMPLETE_DISABLE_FOR_COMMANDS,
        "string[]",
        "[]",
        "Commands that never get completions",
    ),
    SettingDefinition::new(
        AUTOCOMPLETE_SORT_METHOD,
        "\"most recent\"|\"alphabetical\"",
        "\"most recent\"",
        "Boost recently used suggestions unless alphabetical",
    ),
    SettingDefinition::new(
        AUTOCOMPLETE_SCRIPT_TIMEOUT,
        "number (ms)",
        if cfg!(windows) { "20000" } else { "5000" },
        "Generator script timeout floor",
    ),
    SettingDefinition::new(
        AUTOCOMPLETE_IMMEDIATELY_RUN_DANGEROUS_COMMANDS,
        "bool",
        "false",
        "Allow the execute row for dangerous exact matches",
    ),
    SettingDefinition::new(
        AUTOCOMPLETE_FIRST_TOKEN_COMPLETION,
        "bool",
        "false",
        "Complete the command name itself",
    ),
    SettingDefinition::new(
        AUTOCOMPLETE_HIDE_AUTO_EXECUTE_SUGGESTION,
        "bool",
        "false",
        "Never show execute rows",
    ),
    SettingDefinition::new(
        AUTOCOMPLETE_NAVIGATE_TO_HISTORY,
        "bool",
        "false",
        "Up on the first row toggles history mode instead of hiding",
    ),
    SettingDefinition::new(
        AUTOCOMPLETE_ALWAYS_SHOW_DESCRIPTION,
        "bool",
        "false",
        "Keep the description pop-out open",
    ),
    SettingDefinition::new(
        AUTOCOMPLETE_THEME,
        "string",
        "\"dark\"",
        "dark, light, system, or a file stem in ~/.tabsmith/themes",
    ),
    SettingDefinition::new(
        AUTOCOMPLETE_WIDTH,
        "number (columns)",
        "40",
        "Menu width, clamped to the terminal width",
    ),
    SettingDefinition::new(
        AUTOCOMPLETE_HEIGHT,
        "number (rows)",
        "7",
        "Max visible list rows",
    ),
    SettingDefinition::new(
        AUTOCOMPLETE_DEVELOPER_MODE,
        "bool",
        "false",
        "Load specs from devCompletionsFolder first; disable spec caches",
    ),
    SettingDefinition::new(
        AUTOCOMPLETE_DEV_COMPLETIONS_FOLDER,
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
        BETA_HISTORY_MODE,
        "\"off\"|\"history_only\"|\"show\"",
        "\"off\"",
        "History rows in the dropdown",
    ),
    SettingDefinition::new(INLINE_ENABLED, "bool", "true", "Ghost text on/off"),
    SettingDefinition::new(
        INLINE_SOURCE,
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
        INLINE_DEBOUNCE_MS,
        "number",
        "300",
        "Quiet time before an LLM request (ms)",
    ),
    SettingDefinition::new(
        INLINE_HISTORY_COUNT,
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
