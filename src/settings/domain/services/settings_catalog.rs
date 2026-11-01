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

/// The raw JSON default literal `settings list --all` shows for `key`; `None` for a template row, which has none.
pub(crate) fn default(key: &str) -> Option<&'static str> {
    CATALOG
        .iter()
        .find(|definition| definition.key() == key)
        .map(SettingDefinition::default)
        .filter(|literal| !literal.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::domain::value_objects::settings::Settings;
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

    #[test]
    fn should_give_the_default_literal_of_a_documented_key() {
        let key = AUTOCOMPLETE_SORT_METHOD;

        let literal = default(key);

        assert_eq!(literal, Some("\"most recent\""));
    }

    #[test]
    fn should_have_no_default_for_a_template_row() {
        let key = "inline.profiles.<name>";

        let literal = default(key);

        assert_eq!(literal, None);
    }

    #[test]
    fn should_not_take_a_concrete_key_for_the_template_row_it_fits() {
        let key = "autocomplete.keybindings.control+r";

        let literal = default(key);

        assert_eq!(literal, None);
    }

    #[test]
    fn should_have_no_default_for_a_key_it_does_not_document() {
        let key = "no.such.key";

        let literal = default(key);

        assert_eq!(literal, None);
    }

    /// The Phase 0 output capture runs on macOS only, so this pins the Windows default too.
    #[test]
    fn should_default_the_script_timeout_to_20000_ms_on_windows_and_5000_elsewhere() {
        let key = AUTOCOMPLETE_SCRIPT_TIMEOUT;

        let literal = default(key);

        assert_eq!(literal, Some(if cfg!(windows) { "20000" } else { "5000" }));
    }

    /// dropdown/domain/entities/menu.rs max_rows: `f64("autocomplete.height", 7.0)`.
    #[test]
    fn should_default_the_menu_height_to_7_rows() {
        let settings = Settings::default();

        let read = settings.f64(AUTOCOMPLETE_HEIGHT);

        assert_eq!(read, 7.0);
    }

    /// overlay_renderer.rs (`menu_patches`): `f64("autocomplete.width", 40.0)`.
    #[test]
    fn should_default_the_menu_width_to_40_columns() {
        let settings = Settings::default();

        let read = settings.f64(AUTOCOMPLETE_WIDTH);

        assert_eq!(read, 40.0);
    }

    /// completion generator_trigger_policy.rs script_timeout: `f64(.., if cfg!(windows) { 20000.0 } else { 5000.0 })`.
    #[test]
    fn should_default_the_engine_script_timeout_to_20000_ms_on_windows_and_5000_elsewhere() {
        let settings = Settings::default();

        let read = settings.f64(AUTOCOMPLETE_SCRIPT_TIMEOUT);

        assert_eq!(read, if cfg!(windows) { 20000.0 } else { 5000.0 });
    }

    /// specs quickjs_spec_runtime.rs script_timeout: `i64(.., if cfg!(windows) { 20000 } else { 5000 })`.
    #[test]
    fn should_default_the_js_script_timeout_to_20000_ms_on_windows_and_5000_elsewhere() {
        let settings = Settings::default();

        let read = settings.i64(AUTOCOMPLETE_SCRIPT_TIMEOUT);

        assert_eq!(read, if cfg!(windows) { 20000 } else { 5000 });
    }

    /// request_llm_ghost_text_use_case.rs llm_history: `i64("inline.historyCount", 49)`.
    #[test]
    fn should_default_the_llm_history_to_49_lines() {
        let settings = Settings::default();

        let read = settings.i64(INLINE_HISTORY_COUNT);

        assert_eq!(read, 49);
    }

    /// terminal_session/domain/entities/terminal_session.rs: `i64("inline.debounceMs", 300)`.
    #[test]
    fn should_default_the_llm_debounce_to_300_ms() {
        let settings = Settings::default();

        let read = settings.i64(INLINE_DEBOUNCE_MS);

        assert_eq!(read, 300);
    }

    /// get_inline_status_use_case.rs, doctor.rs, suggest_local_ghost_text_use_case.rs and terminal_session/domain/entities/terminal_session.rs: `bool("inline.enabled", true)`.
    #[test]
    fn should_default_ghost_text_to_enabled() {
        let settings = Settings::default();

        let read = settings.bool(INLINE_ENABLED);

        assert!(read);
    }

    /// completion_result_builder.rs: `bool("autocomplete.insertSpaceAutomatically", true)`.
    #[test]
    fn should_default_to_inserting_a_space_automatically() {
        let settings = Settings::default();

        let read = settings.bool(AUTOCOMPLETE_INSERT_SPACE_AUTOMATICALLY);

        assert!(read);
    }

    /// doctor.rs, dropdown/domain/entities/menu.rs and terminal_session/domain/entities/terminal_session.rs: `bool("autocomplete.disable", false)`.
    #[test]
    fn should_default_autocomplete_to_not_disabled() {
        let settings = Settings::default();

        let read = (
            settings.bool(AUTOCOMPLETE_DISABLE),
            default(AUTOCOMPLETE_DISABLE),
        );

        assert_eq!(read, (false, Some("false")));
    }

    /// doctor.rs, command_line_parser.rs, embedded_spec_bundle.rs and quickjs_spec_repository.rs:
    /// `bool("autocomplete.developerMode", false)`.
    #[test]
    fn should_default_developer_mode_to_off() {
        let settings = Settings::default();

        let read = (
            settings.bool(AUTOCOMPLETE_DEVELOPER_MODE),
            default(AUTOCOMPLETE_DEVELOPER_MODE),
        );

        assert_eq!(read, (false, Some("false")));
    }

    /// completion_session.rs: `bool("autocomplete.fuzzySearch", false)`.
    #[test]
    fn should_default_fuzzy_search_to_off() {
        let settings = Settings::default();

        let read = (
            settings.bool(AUTOCOMPLETE_FUZZY_SEARCH),
            default(AUTOCOMPLETE_FUZZY_SEARCH),
        );

        assert_eq!(read, (false, Some("false")));
    }

    /// complete_command_line_use_case.rs: `bool("autocomplete.firstTokenCompletion", false)`.
    #[test]
    fn should_default_first_token_completion_to_off() {
        let settings = Settings::default();

        let read = (
            settings.bool(AUTOCOMPLETE_FIRST_TOKEN_COMPLETION),
            default(AUTOCOMPLETE_FIRST_TOKEN_COMPLETION),
        );

        assert_eq!(read, (false, Some("false")));
    }

    /// completion_result_builder.rs: `bool("autocomplete.hideAutoExecuteSuggestion", false)`.
    #[test]
    fn should_default_to_showing_auto_execute_suggestions() {
        let settings = Settings::default();

        let read = (
            settings.bool(AUTOCOMPLETE_HIDE_AUTO_EXECUTE_SUGGESTION),
            default(AUTOCOMPLETE_HIDE_AUTO_EXECUTE_SUGGESTION),
        );

        assert_eq!(read, (false, Some("false")));
    }

    /// completion_result_builder.rs, dropdown/domain/entities/menu.rs and terminal_session/domain/entities/terminal_session.rs: `bool("autocomplete.onlyShowOnTab", false)`.
    #[test]
    fn should_default_to_opening_the_menu_without_tab() {
        let settings = Settings::default();

        let read = (
            settings.bool(AUTOCOMPLETE_ONLY_SHOW_ON_TAB),
            default(AUTOCOMPLETE_ONLY_SHOW_ON_TAB),
        );

        assert_eq!(read, (false, Some("false")));
    }

    /// completion_result_builder.rs: `bool("autocomplete.immediatelyExecuteAfterSpace", false)`.
    #[test]
    fn should_default_to_no_execute_row_after_a_space() {
        let settings = Settings::default();

        let read = (
            settings.bool(AUTOCOMPLETE_IMMEDIATELY_EXECUTE_AFTER_SPACE),
            default(AUTOCOMPLETE_IMMEDIATELY_EXECUTE_AFTER_SPACE),
        );

        assert_eq!(read, (false, Some("false")));
    }

    /// completion_result_builder.rs: `bool("autocomplete.immediatelyRunDangerousCommands", false)`.
    #[test]
    fn should_default_to_no_execute_row_for_dangerous_commands() {
        let settings = Settings::default();

        let read = (
            settings.bool(AUTOCOMPLETE_IMMEDIATELY_RUN_DANGEROUS_COMMANDS),
            default(AUTOCOMPLETE_IMMEDIATELY_RUN_DANGEROUS_COMMANDS),
        );

        assert_eq!(read, (false, Some("false")));
    }

    /// completion_result_builder.rs: `bool("autocomplete.alwaysSuggestCurrentToken", false)`.
    #[test]
    fn should_default_to_not_suggesting_the_current_token() {
        let settings = Settings::default();

        let read = (
            settings.bool(AUTOCOMPLETE_ALWAYS_SUGGEST_CURRENT_TOKEN),
            default(AUTOCOMPLETE_ALWAYS_SUGGEST_CURRENT_TOKEN),
        );

        assert_eq!(read, (false, Some("false")));
    }

    /// completion_result_builder.rs: `bool("autocomplete.preferVerboseSuggestions", false)`.
    #[test]
    fn should_default_to_short_suggestions() {
        let settings = Settings::default();

        let read = (
            settings.bool(AUTOCOMPLETE_PREFER_VERBOSE_SUGGESTIONS),
            default(AUTOCOMPLETE_PREFER_VERBOSE_SUGGESTIONS),
        );

        assert_eq!(read, (false, Some("false")));
    }

    /// dropdown/domain/entities/menu.rs: `bool("autocomplete.alwaysShowDescription", false)`.
    #[test]
    fn should_default_the_description_pop_out_to_closed() {
        let settings = Settings::default();

        let read = (
            settings.bool(AUTOCOMPLETE_ALWAYS_SHOW_DESCRIPTION),
            default(AUTOCOMPLETE_ALWAYS_SHOW_DESCRIPTION),
        );

        assert_eq!(read, (false, Some("false")));
    }

    /// dropdown/domain/entities/menu.rs: `bool("autocomplete.scrollWrapAround", false)`.
    #[test]
    fn should_default_the_menu_to_not_wrap_around() {
        let settings = Settings::default();

        let read = (
            settings.bool(AUTOCOMPLETE_SCROLL_WRAP_AROUND),
            default(AUTOCOMPLETE_SCROLL_WRAP_AROUND),
        );

        assert_eq!(read, (false, Some("false")));
    }

    /// dropdown/domain/entities/menu.rs: `bool("autocomplete.navigateToHistory", false)`.
    #[test]
    fn should_default_to_hiding_the_menu_instead_of_navigating_to_history() {
        let settings = Settings::default();

        let read = (
            settings.bool(AUTOCOMPLETE_NAVIGATE_TO_HISTORY),
            default(AUTOCOMPLETE_NAVIGATE_TO_HISTORY),
        );

        assert_eq!(read, (false, Some("false")));
    }

    /// theme_resolver.rs: `string("autocomplete.theme").unwrap_or("dark")`; the theme command prints `dark` too.
    #[test]
    fn should_default_the_theme_to_dark() {
        let settings = Settings::default();

        let read = settings.string(AUTOCOMPLETE_THEME);

        assert_eq!(read, Some("dark"));
    }

    /// ghost_text_source_resolver.rs and doctor.rs: `string("inline.source").unwrap_or("history+specs")`.
    #[test]
    fn should_default_the_ghost_text_source_to_history_then_specs() {
        let settings = Settings::default();

        let read = settings.string(INLINE_SOURCE);

        assert_eq!(read, Some("history+specs"));
    }

    /// completion_result_builder.rs: `string("beta.history.mode").unwrap_or("off")`.
    #[test]
    fn should_default_the_dropdown_history_mode_to_off() {
        let settings = Settings::default();

        let read = settings.string(BETA_HISTORY_MODE);

        assert_eq!(read, Some("off"));
    }

    /// completion_result_builder.rs ranks by recency unless
    /// `string("autocomplete.sortMethod") == Some("alphabetical")`; an unset key was `None` there and is
    /// `most recent` now, and both rank by recency.
    #[test]
    fn should_default_the_sort_method_to_most_recent() {
        let settings = Settings::default();

        let read = settings.string(AUTOCOMPLETE_SORT_METHOD);

        assert_eq!(read, Some("most recent"));
    }

    /// spec_lookup.rs (`SpecLookup::disables`): `str_list("autocomplete.disableForCommands")` is empty when unset.
    #[test]
    fn should_default_to_completing_every_command() {
        let settings = Settings::default();

        let read = (
            settings.str_list(AUTOCOMPLETE_DISABLE_FOR_COMMANDS),
            default(AUTOCOMPLETE_DISABLE_FOR_COMMANDS),
        );

        assert_eq!(read, (Vec::<String>::new(), Some("[]")));
    }
}
