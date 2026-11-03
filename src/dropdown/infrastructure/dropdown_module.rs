use crate::dropdown::application::list_themes_use_case::ListThemesUseCase;
use crate::dropdown::application::select_theme_use_case::SelectThemeUseCase;
use crate::dropdown::application::show_theme_use_case::ShowThemeUseCase;
use crate::dropdown::domain::repositories::theme_repository::ThemeRepository;
use crate::dropdown::infrastructure::adapters::file_theme_repository::FileThemeRepository;
use crate::settings::domain::repositories::settings_repository::SettingsRepository;
use crate::shared::infrastructure::shared_dependencies::SharedDependencies;
use std::rc::Rc;

/// The use cases of the `theme` command, over the settings the composition root injects and the theme files of the
/// data directory. `themes` is also what the command asks for the themes folder.
pub(crate) struct DropdownModule {
    pub(crate) select_theme: SelectThemeUseCase,
    pub(crate) list_themes: ListThemesUseCase,
    pub(crate) show_theme: ShowThemeUseCase,
    pub(crate) themes: Rc<dyn ThemeRepository>,
}

/// Only builds: the themes folder is resolved when a command asks for it, so an unknown home fails only then.
pub(crate) fn build_dropdown_module(
    dependencies: &SharedDependencies,
    settings: Rc<dyn SettingsRepository>,
) -> DropdownModule {
    let themes: Rc<dyn ThemeRepository> = Rc::new(FileThemeRepository::new(
        dependencies.data_directory.clone(),
    ));
    DropdownModule {
        select_theme: SelectThemeUseCase::new(settings.clone(), themes.clone()),
        list_themes: ListThemesUseCase::new(themes.clone()),
        show_theme: ShowThemeUseCase::new(settings),
        themes,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::domain::repositories::settings_repository::InMemorySettingsRepository;
    use crate::settings::domain::services::settings_catalog::AUTOCOMPLETE_THEME;
    use crate::shared::application::ports::clock::FixedClock;
    use crate::shared::domain::value_objects::data_directory::DataDirectory;
    use crate::shared::infrastructure::testing::temp_dir::TempDir;
    use std::fs;
    use std::sync::Arc;

    /// The theme file is looked up under the data directory the module was built with, never one the env names.
    #[test]
    fn should_select_a_custom_theme_from_the_data_directory_it_was_built_with() {
        let temp_dir = TempDir::new("dropdown-module");
        fs::create_dir(temp_dir.path().join("themes")).unwrap();
        fs::write(temp_dir.path().join("themes/ocean.json"), "{}").unwrap();
        let settings = Rc::new(InMemorySettingsRepository::default());
        let module = build_dropdown_module(
            &SharedDependencies {
                data_directory: DataDirectory::new(Some(temp_dir.path().to_path_buf()), None),
                clock: Arc::new(FixedClock::new(0)),
            },
            settings.clone(),
        );

        module.select_theme.execute("ocean", |_| {}).unwrap();

        assert_eq!(
            settings.load().unwrap().string(AUTOCOMPLETE_THEME),
            Some("ocean")
        );
    }
}
