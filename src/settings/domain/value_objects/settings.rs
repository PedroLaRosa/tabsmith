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
        let settings = settings_with(&[(AUTOCOMPLETE_THEME, SettingValue::Text("light".to_string()))]);

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
}
