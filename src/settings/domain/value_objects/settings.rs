use crate::settings::domain::services::settings_catalog;
use crate::settings::domain::value_objects::setting_value::SettingValue;
use std::collections::BTreeMap;
use std::str::FromStr;
use std::sync::Arc;

/// Immutable snapshot of the user's settings, cheap to clone and shared across threads.
#[derive(Clone, Default, Debug)]
pub(crate) struct Settings(Arc<BTreeMap<String, SettingValue>>);

impl Settings {
    pub(crate) fn new(values: BTreeMap<String, SettingValue>) -> Self {
        Settings(Arc::new(values))
    }

    /// The value the user set; never a catalog default.
    pub(crate) fn get(&self, key: &str) -> Option<&SettingValue> {
        self.0.get(key)
    }

    /// The user's bool, else the catalog default; `false` when the catalog has no bool for `key`.
    pub(crate) fn bool(&self, key: &str) -> bool {
        self.get(key)
            .and_then(SettingValue::as_bool)
            .or_else(|| catalog_default(key))
            .unwrap_or_default()
    }

    /// The user's string, else the catalog default when it is a string; `None` for `null` (devCompletionsFolder).
    pub(crate) fn string(&self, key: &str) -> Option<&str> {
        self.get(key)
            .and_then(SettingValue::as_str)
            .or_else(|| catalog_default_string(key))
    }

    /// The user's integer (a float such as `10.0` is none), else the catalog default; `0` when the catalog has none.
    pub(crate) fn i64(&self, key: &str) -> i64 {
        self.get(key)
            .and_then(SettingValue::as_i64)
            .or_else(|| catalog_default(key))
            .unwrap_or_default()
    }

    /// The user's number (integers too), else the catalog default; `0.0` when the catalog has none.
    pub(crate) fn f64(&self, key: &str) -> f64 {
        self.get(key)
            .and_then(SettingValue::as_f64)
            .or_else(|| catalog_default(key))
            .unwrap_or_default()
    }

    /// The strings of the user's list (other items dropped), else empty.
    /// ponytail: empty is the catalog's only list default (`[]`); parse list literals once another default exists.
    pub(crate) fn str_list(&self, key: &str) -> Vec<String> {
        self.get(key)
            .and_then(SettingValue::as_list)
            .map(|items| {
                items
                    .iter()
                    .filter_map(SettingValue::as_str)
                    .map(String::from)
                    .collect()
            })
            .unwrap_or_default()
    }

    /// The user's keys with the given prefix, prefix stripped, in key order (e.g. `autocomplete.keybindings.`).
    pub(crate) fn with_prefix<'a>(
        &'a self,
        prefix: &'a str,
    ) -> impl Iterator<Item = (&'a str, &'a SettingValue)> + 'a {
        self.0
            .iter()
            .filter_map(move |(key, value)| key.strip_prefix(prefix).map(|rest| (rest, value)))
    }
}

/// ponytail: the catalog's string literals hold no escapes, so dropping the quotes is the JSON string.
fn catalog_default_string(key: &str) -> Option<&'static str> {
    settings_catalog::default(key)?
        .strip_prefix('"')?
        .strip_suffix('"')
}

/// The catalog's JSON default literal for `key`, read as a `T`.
fn catalog_default<T: FromStr>(key: &str) -> Option<T> {
    settings_catalog::default(key)?.parse().ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::domain::services::settings_catalog::*;
    use crate::settings::domain::value_objects::setting_number::SettingNumber;

    fn settings_with(entries: &[(&str, SettingValue)]) -> Settings {
        Settings::new(
            entries
                .iter()
                .map(|(key, value)| (key.to_string(), value.clone()))
                .collect(),
        )
    }

    #[test]
    fn should_give_the_value_the_user_set_for_a_key() {
        let settings = settings_with(&[(
            "autocomplete.theme",
            SettingValue::Text("light".to_string()),
        )]);

        let value = settings.get("autocomplete.theme");

        assert_eq!(value, Some(&SettingValue::Text("light".to_string())));
    }

    #[test]
    fn should_read_a_bool_the_user_set() {
        let settings = settings_with(&[(AUTOCOMPLETE_DISABLE, SettingValue::Bool(true))]);

        let disabled = settings.bool(AUTOCOMPLETE_DISABLE);

        assert!(disabled);
    }

    #[test]
    fn should_fall_back_to_the_catalog_default_for_a_bool_the_user_did_not_set() {
        let settings = Settings::default();

        let enabled = settings.bool(INLINE_ENABLED);

        assert!(enabled);
    }

    /// `settings inline.enabled '"false"'` stores text, which leaves ghost text on.
    #[test]
    fn should_fall_back_to_the_catalog_default_for_a_bool_of_the_wrong_type() {
        let settings = settings_with(&[(INLINE_ENABLED, SettingValue::Text("false".to_string()))]);

        let enabled = settings.bool(INLINE_ENABLED);

        assert!(enabled);
    }

    #[test]
    fn should_read_a_string_the_user_set() {
        let settings =
            settings_with(&[(AUTOCOMPLETE_THEME, SettingValue::Text("light".to_string()))]);

        let theme = settings.string(AUTOCOMPLETE_THEME);

        assert_eq!(theme, Some("light"));
    }

    #[test]
    fn should_fall_back_to_the_catalog_default_for_a_string_the_user_did_not_set() {
        let settings = Settings::default();

        let theme = settings.string(AUTOCOMPLETE_THEME);

        assert_eq!(theme, Some("dark"));
    }

    /// `inline.source: 5` resolves to the default `history+specs` source.
    #[test]
    fn should_fall_back_to_the_catalog_default_for_a_string_of_the_wrong_type() {
        let five = SettingNumber::from(5u64);
        let settings = settings_with(&[(INLINE_SOURCE, SettingValue::Number(five))]);

        let source = settings.string(INLINE_SOURCE);

        assert_eq!(source, Some("history+specs"));
    }

    /// An unset `autocomplete.devCompletionsFolder` means no dev folder, as before the catalog gave defaults.
    #[test]
    fn should_have_no_string_for_an_unset_key_whose_catalog_default_is_null() {
        let settings = Settings::default();

        let folder = settings.string(AUTOCOMPLETE_DEV_COMPLETIONS_FOLDER);

        assert_eq!(folder, None);
    }

    #[test]
    fn should_read_an_integer_the_user_set() {
        let two = SettingNumber::from(2u64);
        let settings = settings_with(&[(INLINE_HISTORY_COUNT, SettingValue::Number(two))]);

        let count = settings.i64(INLINE_HISTORY_COUNT);

        assert_eq!(count, 2);
    }

    /// `inline.historyCount: 10.0` sends 49 history lines: a float is no integer.
    #[test]
    fn should_fall_back_to_the_catalog_default_for_an_integer_written_as_a_float() {
        let ten = SettingNumber::from_f64(10.0).unwrap();
        let settings = settings_with(&[(INLINE_HISTORY_COUNT, SettingValue::Number(ten))]);

        let count = settings.i64(INLINE_HISTORY_COUNT);

        assert_eq!(count, 49);
    }

    #[test]
    fn should_fall_back_to_the_catalog_default_for_an_integer_the_user_did_not_set() {
        let settings = Settings::default();

        let debounce = settings.i64(INLINE_DEBOUNCE_MS);

        assert_eq!(debounce, 300);
    }

    #[test]
    fn should_fall_back_to_the_catalog_default_for_an_integer_above_the_i64_range() {
        let huge = SettingNumber::from(i64::MAX as u64 + 1);
        let settings = settings_with(&[(INLINE_DEBOUNCE_MS, SettingValue::Number(huge))]);

        let debounce = settings.i64(INLINE_DEBOUNCE_MS);

        assert_eq!(debounce, 300);
    }

    /// `settings autocomplete.height 5` stores the integer 5, which the menu reads as 5 rows.
    #[test]
    fn should_read_an_integer_the_user_set_as_a_float() {
        let five = SettingNumber::from(5u64);
        let settings = settings_with(&[(AUTOCOMPLETE_HEIGHT, SettingValue::Number(five))]);

        let height = settings.f64(AUTOCOMPLETE_HEIGHT);

        assert_eq!(height, 5.0);
    }

    #[test]
    fn should_fall_back_to_the_catalog_default_for_a_number_the_user_did_not_set() {
        let settings = Settings::default();

        let height = settings.f64(AUTOCOMPLETE_HEIGHT);

        assert_eq!(height, 7.0);
    }

    /// The engine reads `autocomplete.scriptTimeout` as a float and the JS runtime as an integer, so `7500.0`
    /// is 7500 ms for one and the default for the other.
    #[test]
    fn should_read_a_float_as_a_number_but_fall_back_to_the_default_when_read_as_an_integer() {
        let timeout = SettingNumber::from_f64(7500.0).unwrap();
        let settings =
            settings_with(&[(AUTOCOMPLETE_SCRIPT_TIMEOUT, SettingValue::Number(timeout))]);

        let read = (
            settings.f64(AUTOCOMPLETE_SCRIPT_TIMEOUT),
            settings.i64(AUTOCOMPLETE_SCRIPT_TIMEOUT),
        );

        assert_eq!(read, (7500.0, if cfg!(windows) { 20000 } else { 5000 }));
    }

    #[test]
    fn should_fall_back_to_the_catalog_default_for_a_number_of_the_wrong_type() {
        let settings = settings_with(&[(AUTOCOMPLETE_WIDTH, SettingValue::Text("60".to_string()))]);

        let width = settings.f64(AUTOCOMPLETE_WIDTH);

        assert_eq!(width, 40.0);
    }

    /// `autocomplete.disableForCommands: ["git", 5, "cd"]` disables git and cd and ignores the 5.
    #[test]
    fn should_read_the_strings_of_a_list_the_user_set_and_drop_the_rest() {
        let five = SettingValue::Number(SettingNumber::from(5u64));
        let commands = SettingValue::List(vec![
            SettingValue::Text("git".to_string()),
            five,
            SettingValue::Text("cd".to_string()),
        ]);
        let settings = settings_with(&[(AUTOCOMPLETE_DISABLE_FOR_COMMANDS, commands)]);

        let disabled = settings.str_list(AUTOCOMPLETE_DISABLE_FOR_COMMANDS);

        assert_eq!(disabled, ["git", "cd"]);
    }

    #[test]
    fn should_read_a_value_that_is_not_a_list_as_an_empty_list() {
        let settings = settings_with(&[(
            AUTOCOMPLETE_DISABLE_FOR_COMMANDS,
            SettingValue::Text("git".to_string()),
        )]);

        let disabled = settings.str_list(AUTOCOMPLETE_DISABLE_FOR_COMMANDS);

        assert!(disabled.is_empty(), "{disabled:?}");
    }

    /// Two aliases of one keystroke: the alphabetically later binding is applied last and wins.
    #[test]
    fn should_give_the_keys_under_a_prefix_without_it_in_sorted_order() {
        let ignore = SettingValue::Text("ignore".to_string());
        let settings = settings_with(&[
            ("autocomplete.keybindings.tab", ignore.clone()),
            ("autocomplete.keybindings.control+r", ignore.clone()),
            (AUTOCOMPLETE_THEME, ignore.clone()),
        ]);

        let bindings: Vec<_> = settings.with_prefix("autocomplete.keybindings.").collect();

        assert_eq!(bindings, [("control+r", &ignore), ("tab", &ignore)]);
    }

    /// `settings inline.profiles. '{..}'` shows up as a profile with an empty name.
    #[test]
    fn should_give_a_key_equal_to_the_prefix_as_the_empty_key() {
        let settings = settings_with(&[("inline.profiles.", SettingValue::Null)]);

        let profiles: Vec<_> = settings.with_prefix("inline.profiles.").collect();

        assert_eq!(profiles, [("", &SettingValue::Null)]);
    }

    /// The catalog's `inline.profiles.<name>` row documents the template; it is no profile.
    #[test]
    fn should_give_no_catalog_template_row_under_a_prefix() {
        let settings = Settings::default();

        let profiles = settings.with_prefix("inline.profiles.").count();

        assert_eq!(profiles, 0);
    }

    /// The getters read the catalog's literals without serde_json, so they must agree with serde_json's reading:
    /// a literal with an escape, a fraction or a non-empty list would show up here.
    #[test]
    fn should_read_every_catalog_default_the_way_serde_json_reads_its_literal() {
        let settings = Settings::default();
        let read_by_serde_json = |literal: &str| {
            let json: serde_json::Value = serde_json::from_str(literal).unwrap();
            let strings: Vec<String> = json
                .as_array()
                .map(|items| {
                    items
                        .iter()
                        .filter_map(|item| item.as_str().map(String::from))
                        .collect()
                })
                .unwrap_or_default();
            (
                json.as_bool().unwrap_or_default(),
                json.as_str().map(String::from),
                json.as_i64().unwrap_or_default(),
                json.as_f64().unwrap_or_default(),
                strings,
            )
        };

        let mismatches: Vec<_> = catalog()
            .iter()
            .filter(|definition| !definition.default().is_empty())
            .filter(|definition| {
                let key = definition.key();
                let read = (
                    settings.bool(key),
                    settings.string(key).map(String::from),
                    settings.i64(key),
                    settings.f64(key),
                    settings.str_list(key),
                );
                read != read_by_serde_json(definition.default())
            })
            .map(|definition| definition.key())
            .collect();

        assert!(mismatches.is_empty(), "{mismatches:?}");
    }
}
