use crate::settings::domain::repositories::settings_repository::SettingsRepository;
use crate::settings::domain::settings_error::SettingsError;
use crate::settings::domain::value_objects::setting_key_pattern::SettingKeyPattern;
use std::rc::Rc;

/// `tabsmith settings --delete PATTERN`: removes every saved setting whose key matches the wildcard pattern.
pub(crate) struct DeleteSettingsUseCase {
    settings: Rc<dyn SettingsRepository>,
}

impl DeleteSettingsUseCase {
    pub(crate) fn new(settings: Rc<dyn SettingsRepository>) -> Self {
        DeleteSettingsUseCase { settings }
    }

    /// `announce` is told the matching keys, in key order.
    pub(crate) fn execute(
        &self,
        pattern: &str,
        announce: impl FnOnce(&[String]),
    ) -> Result<(), SettingsError> {
        let settings = self.settings.load()?;
        let pattern = SettingKeyPattern::new(pattern);
        let keys: Vec<String> = settings
            .entries()
            .map(|(key, _)| key)
            .filter(|key| pattern.matches(key))
            .map(String::from)
            .collect();
        announce(&keys);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::domain::repositories::settings_repository::InMemorySettingsRepository;
    use crate::settings::domain::value_objects::setting_number::SettingNumber;
    use crate::settings::domain::value_objects::setting_value::SettingValue;
    use crate::settings::domain::value_objects::settings::Settings;

    /// The settings `{"autocomplete.theme": "dark", "autocomplete.width": 40, "inline.enabled": true}`.
    fn three_settings() -> Rc<InMemorySettingsRepository> {
        let repository = Rc::new(InMemorySettingsRepository::default());
        let settings = Settings::default()
            .with("autocomplete.theme", SettingValue::Text("dark".to_string()))
            .with(
                "autocomplete.width",
                SettingValue::Number(SettingNumber::from(40u64)),
            )
            .with("inline.enabled", SettingValue::Bool(true));
        repository.save(&settings).unwrap();
        repository
    }

    /// The keys a delete of `pattern` matches in [`three_settings`], in the order the command lists them.
    fn matching_keys(pattern: &str) -> Vec<String> {
        let mut matched = Vec::new();
        let _ = DeleteSettingsUseCase::new(three_settings())
            .execute(pattern, |keys| matched = keys.to_vec());
        matched
    }

    #[test]
    fn settings_delete_wildcards() {
        assert_eq!(
            matching_keys("autocomplete.*"),
            ["autocomplete.theme", "autocomplete.width"]
        );
        assert_eq!(matching_keys("inline.enabled"), ["inline.enabled"]);
        assert!(matching_keys("nope*").is_empty());
    }
}
