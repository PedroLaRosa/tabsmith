use crate::modules::dropdown::application::current_theme::CurrentTheme;
use crate::modules::dropdown::domain::dropdown_error::DropdownError;
use crate::modules::settings::domain::repositories::settings_repository::SettingsRepository;
use crate::modules::settings::domain::services::settings_catalog::AUTOCOMPLETE_THEME;
use std::rc::Rc;

/// `tabsmith theme` with no name: the saved `autocomplete.theme`.
pub(crate) struct ShowThemeUseCase {
    settings: Rc<dyn SettingsRepository>,
}

impl ShowThemeUseCase {
    pub(crate) fn new(settings: Rc<dyn SettingsRepository>) -> Self {
        ShowThemeUseCase { settings }
    }

    /// A saved value that is not text (`null` included) is given as it is: `Settings::string` would answer the
    /// default `dark` for it.
    pub(crate) fn execute(&self) -> Result<CurrentTheme, DropdownError> {
        let settings = self.settings.load()?;
        Ok(match settings.get(AUTOCOMPLETE_THEME) {
            Some(value) if value.as_str().is_none() => CurrentTheme::NotText(value.clone()),
            Some(_) | None => CurrentTheme::Named(
                settings
                    .string(AUTOCOMPLETE_THEME)
                    .unwrap_or_default()
                    .to_string(),
            ),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::settings::domain::repositories::settings_repository::{
        InMemorySettingsRepository, UnreadableSettingsRepository,
    };
    use crate::modules::settings::domain::value_objects::setting_number::SettingNumber;
    use crate::modules::settings::domain::value_objects::setting_value::SettingValue;
    use crate::modules::settings::domain::value_objects::settings::Settings;
    use std::collections::BTreeMap;

    /// What `theme` shows when `autocomplete.theme` is saved as `value` (or not saved at all).
    fn show_saved(value: Option<SettingValue>) -> CurrentTheme {
        let repository = Rc::new(InMemorySettingsRepository::default());
        let settings = match value {
            Some(value) => Settings::default().with(AUTOCOMPLETE_THEME, value),
            None => Settings::default(),
        };
        repository.save(&settings).unwrap();
        ShowThemeUseCase::new(repository).execute().unwrap()
    }

    fn text(value: &str) -> SettingValue {
        SettingValue::Text(value.to_string())
    }

    #[test]
    fn should_show_the_default_theme_when_none_is_set() {
        let shown = show_saved(None);

        assert_eq!(shown, CurrentTheme::Named("dark".to_string()));
    }

    #[test]
    fn should_show_a_text_theme_raw_including_an_empty_one() {
        let shown = ["light", "", "a\nb"].map(|name| show_saved(Some(text(name))));

        assert_eq!(
            shown,
            [
                CurrentTheme::Named("light".to_string()),
                CurrentTheme::Named(String::new()),
                CurrentTheme::Named("a\nb".to_string())
            ]
        );
    }

    #[test]
    fn should_show_a_value_that_is_not_text_unchanged() {
        let values = [
            SettingValue::Null,
            SettingValue::Number(SettingNumber::from(5u64)),
            SettingValue::List(vec![text("a")]),
            SettingValue::Object(BTreeMap::from([(
                "b".to_string(),
                SettingValue::Bool(true),
            )])),
        ];

        let shown = values.clone().map(|value| show_saved(Some(value)));

        assert_eq!(shown, values.map(CurrentTheme::NotText));
    }

    #[test]
    fn should_fail_to_show_over_settings_that_cannot_be_loaded() {
        let use_case = ShowThemeUseCase::new(Rc::new(UnreadableSettingsRepository::default()));

        let result = use_case.execute();

        assert!(matches!(result, Err(DropdownError::Settings(_))));
    }
}
