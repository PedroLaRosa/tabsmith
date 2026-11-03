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

    /// `announce` is told the matching keys, in key order; a pattern that matches none is an error.
    pub(crate) fn execute(
        &self,
        pattern: &str,
        announce: impl FnOnce(&[String]),
    ) -> Result<(), SettingsError> {
        let settings = self.settings.load()?;
        let key_pattern = SettingKeyPattern::new(pattern);
        let keys: Vec<String> = settings
            .entries()
            .map(|(key, _)| key)
            .filter(|key| key_pattern.matches(key))
            .map(String::from)
            .collect();
        if keys.is_empty() {
            return Err(SettingsError::NoMatchingSettings(pattern.to_string()));
        }
        announce(&keys);
        let remaining = keys
            .iter()
            .fold(settings, |settings, key| settings.without(key));
        self.settings.save(&remaining)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::domain::repositories::settings_repository::{
        InMemorySettingsRepository, UnreadableSettingsRepository,
    };
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

    #[test]
    fn should_remove_every_setting_the_pattern_matches() {
        let repository = three_settings();
        let use_case = DeleteSettingsUseCase::new(repository.clone());

        use_case.execute("autocomplete.*", |_| {}).unwrap();

        let remaining: Vec<String> = repository
            .load()
            .unwrap()
            .entries()
            .map(|(key, _)| key.to_string())
            .collect();
        assert_eq!(remaining, ["inline.enabled"]);
    }

    /// `settings -d nope.key` fails with `No settings found matching nope.key` before listing any removal.
    #[test]
    fn should_refuse_a_pattern_that_matches_no_setting() {
        let use_case = DeleteSettingsUseCase::new(three_settings());
        let mut announced = false;

        let result = use_case.execute("nope*", |_| announced = true);

        assert!(
            matches!(&result, Err(SettingsError::NoMatchingSettings(pattern)) if pattern == "nope*")
                && !announced
        );
    }

    /// The command prints `Removing ...` before it saves, so the message shows even when the save then fails.
    #[test]
    fn should_announce_the_keys_while_they_are_still_saved() {
        let repository = three_settings();
        let use_case = DeleteSettingsUseCase::new(repository.clone());
        let mut still_saved = false;

        use_case
            .execute("inline.enabled", |_| {
                still_saved = repository.load().unwrap().get("inline.enabled").is_some()
            })
            .unwrap();

        assert!(still_saved);
    }

    /// `settings -d KEY` on a corrupted settings.json refuses instead of overwriting it.
    #[test]
    fn should_not_save_a_deletion_over_settings_that_cannot_be_loaded() {
        let repository = Rc::new(UnreadableSettingsRepository::default());
        let use_case = DeleteSettingsUseCase::new(repository.clone());

        let result = use_case.execute("*", |_| {});

        assert_eq!(
            (result.is_err(), repository.save_attempted()),
            (true, false)
        );
    }
}
