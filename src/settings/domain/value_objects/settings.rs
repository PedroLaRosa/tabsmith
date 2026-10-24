use crate::settings::domain::value_objects::setting_value::SettingValue;
use std::collections::BTreeMap;
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

    pub(crate) fn bool(&self, key: &str) -> bool {
        self.get(key)
            .and_then(SettingValue::as_bool)
            .unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::domain::services::settings_catalog::*;

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
}
