use crate::dropdown::application::theme_selection::ThemeSelection;
use crate::dropdown::domain::dropdown_error::DropdownError;
use crate::dropdown::domain::repositories::theme_repository::ThemeRepository;
use crate::dropdown::domain::value_objects::theme_choice::ThemeChoice;
use crate::settings::domain::repositories::settings_repository::SettingsRepository;
use crate::settings::domain::services::settings_catalog::AUTOCOMPLETE_THEME;
use crate::settings::domain::value_objects::setting_value::SettingValue;
use std::rc::Rc;

/// `tabsmith theme NAME`: saves the theme as `autocomplete.theme`, a theme file taking precedence over a built-in
/// theme of the same name.
pub(crate) struct SelectThemeUseCase {
    settings: Rc<dyn SettingsRepository>,
    themes: Rc<dyn ThemeRepository>,
}

impl SelectThemeUseCase {
    pub(crate) fn new(
        settings: Rc<dyn SettingsRepository>,
        themes: Rc<dyn ThemeRepository>,
    ) -> Self {
        SelectThemeUseCase { settings, themes }
    }

    /// The settings are loaded first, so unreadable settings hide an unknown or invalid theme. `announce` is told
    /// the selection before it is saved, so the message shows even when the save then fails. The name is saved as
    /// typed.
    pub(crate) fn execute(
        &self,
        name: &str,
        announce: impl FnOnce(&ThemeSelection),
    ) -> Result<(), DropdownError> {
        let settings = self.settings.load()?;
        let selection = match self.themes.find(name)? {
            Some(definition) => ThemeSelection::Custom(definition),
            None => match ThemeChoice::built_in(name) {
                Some(choice) => ThemeSelection::BuiltIn(choice),
                None => {
                    return Err(DropdownError::ThemeNotFound {
                        name: name.to_string(),
                        folder: self.themes.folder()?,
                    })
                }
            },
        };
        announce(&selection);
        self.settings
            .save(&settings.with(AUTOCOMPLETE_THEME, SettingValue::Text(name.to_string())))?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dropdown::domain::repositories::theme_repository::InMemoryThemeRepository;
    use crate::dropdown::domain::value_objects::theme_author::ThemeAuthor;
    use crate::dropdown::domain::value_objects::theme_definition::ThemeDefinition;
    use crate::settings::domain::repositories::settings_repository::{
        InMemorySettingsRepository, UnreadableSettingsRepository,
    };
    use crate::settings::domain::settings_error::SettingsError;
    use crate::settings::domain::value_objects::setting_number::SettingNumber;
    use crate::settings::domain::value_objects::settings::Settings;
    use std::path::Path;

    fn by(name: &str) -> ThemeDefinition {
        ThemeDefinition::new(Some(ThemeAuthor::new(Some(name.to_string()), None, None)))
    }

    /// Settings `{"autocomplete.theme": "dark", "autocomplete.width": 50}`, already saved.
    fn saved_settings() -> Rc<InMemorySettingsRepository> {
        let repository = Rc::new(InMemorySettingsRepository::default());
        let settings = Settings::default()
            .with(AUTOCOMPLETE_THEME, SettingValue::Text("dark".to_string()))
            .with(
                "autocomplete.width",
                SettingValue::Number(SettingNumber::from(50u64)),
            );
        repository.save(&settings).unwrap();
        repository
    }

    fn themes() -> InMemoryThemeRepository {
        InMemoryThemeRepository::default().with_folder("/data/themes")
    }

    fn saved_theme(settings: &InMemorySettingsRepository) -> Option<SettingValue> {
        settings.load().unwrap().get(AUTOCOMPLETE_THEME).cloned()
    }

    /// The result of selecting `name`, and what was announced (nothing when it failed first).
    fn select(
        settings: &Rc<InMemorySettingsRepository>,
        themes: InMemoryThemeRepository,
        name: &str,
    ) -> (Result<(), DropdownError>, Option<ThemeSelection>) {
        let mut announced = None;
        let result = SelectThemeUseCase::new(settings.clone(), Rc::new(themes))
            .execute(name, |selection| announced = Some(selection.clone()));
        (result, announced)
    }

    /// Settings that load but cannot be saved, like a read-only data directory.
    struct UnwritableSettingsRepository;

    impl SettingsRepository for UnwritableSettingsRepository {
        fn load(&self) -> Result<Settings, SettingsError> {
            Ok(Settings::default())
        }

        fn save(&self, _settings: &Settings) -> Result<(), SettingsError> {
            Err(SettingsError::Storage(
                "Permission denied (os error 13)".into(),
            ))
        }

        fn last_modified(&self) -> Option<std::time::SystemTime> {
            None
        }
    }

    #[test]
    fn should_select_a_built_in_theme_and_keep_the_other_settings() {
        let settings = saved_settings();

        let (result, announced) = select(&settings, themes(), "light");

        let width = settings.load().unwrap().get("autocomplete.width").cloned();
        assert_eq!(
            (result.is_ok(), announced, saved_theme(&settings), width),
            (
                true,
                Some(ThemeSelection::BuiltIn(ThemeChoice::Light)),
                Some(SettingValue::Text("light".to_string())),
                Some(SettingValue::Number(SettingNumber::from(50u64)))
            )
        );
    }

    #[test]
    fn should_select_a_custom_theme_before_a_built_in_of_the_same_name() {
        let settings = saved_settings();

        let (result, announced) = select(&settings, themes().with_theme("dark", by("Ann")), "dark");

        assert_eq!(
            (result.is_ok(), announced),
            (true, Some(ThemeSelection::Custom(by("Ann"))))
        );
    }

    #[test]
    fn should_not_take_an_empty_name_as_a_built_in() {
        let settings = saved_settings();

        let (result, announced) = select(&settings, themes(), "");

        assert_eq!(
            (
                result.unwrap_err().to_string(),
                announced,
                saved_theme(&settings)
            ),
            (
                "'' does not exist in /data/themes".to_string(),
                None,
                Some(SettingValue::Text("dark".to_string()))
            )
        );
    }

    #[test]
    fn should_refuse_an_unknown_theme_naming_the_folder() {
        let settings = saved_settings();

        let (result, announced) = select(&settings, themes(), "nope");

        assert_eq!(
            (result.unwrap_err().to_string(), announced),
            ("'nope' does not exist in /data/themes".to_string(), None)
        );
    }

    #[test]
    fn should_refuse_an_invalid_theme_file_and_save_nothing() {
        let settings = saved_settings();
        let themes = themes().with_invalid_theme("light", "expected value at line 1 column 1");

        let (result, announced) = select(&settings, themes, "light");

        let path = Path::new("/data/themes").join("light.json");
        assert_eq!(
            (
                result.unwrap_err().to_string(),
                announced,
                saved_theme(&settings)
            ),
            (
                format!(
                    "invalid theme file {}: expected value at line 1 column 1",
                    path.display()
                ),
                None,
                Some(SettingValue::Text("dark".to_string()))
            )
        );
    }

    #[test]
    fn should_load_the_settings_before_it_looks_for_the_theme() {
        let settings = Rc::new(UnreadableSettingsRepository::default());
        let themes: Rc<InMemoryThemeRepository> =
            Rc::new(themes().with_invalid_theme("light", "expected value at line 1 column 1"));
        let use_case = SelectThemeUseCase::new(settings.clone(), themes);

        let results = ["nope", "light"].map(|name| use_case.execute(name, |_| {}));

        assert_eq!(
            (
                results
                    .iter()
                    .all(|result| matches!(result, Err(DropdownError::Settings(_)))),
                settings.save_attempted()
            ),
            (true, false)
        );
    }

    #[test]
    fn should_announce_the_selection_before_it_saves() {
        let settings = saved_settings();
        let mut saved_while_announcing = None;

        let result = SelectThemeUseCase::new(settings.clone(), Rc::new(themes()))
            .execute("light", |_| saved_while_announcing = saved_theme(&settings));

        assert_eq!(
            (
                result.is_ok(),
                saved_while_announcing,
                saved_theme(&settings)
            ),
            (
                true,
                Some(SettingValue::Text("dark".to_string())),
                Some(SettingValue::Text("light".to_string()))
            )
        );
    }

    #[test]
    fn should_save_the_name_as_typed() {
        let settings = saved_settings();

        let (result, _) = select(
            &settings,
            themes().with_theme("My Theme", by("Ann")),
            "My Theme",
        );

        assert_eq!(
            (result.is_ok(), saved_theme(&settings)),
            (true, Some(SettingValue::Text("My Theme".to_string())))
        );
    }

    #[test]
    fn should_report_a_save_failure_after_announcing() {
        let mut announced = false;

        let result =
            SelectThemeUseCase::new(Rc::new(UnwritableSettingsRepository), Rc::new(themes()))
                .execute("light", |_| announced = true);

        assert_eq!(
            (result.unwrap_err().to_string(), announced),
            ("Permission denied (os error 13)".to_string(), true)
        );
    }
}
