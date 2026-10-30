use crate::dropdown::domain::dropdown_error::DropdownError;
use crate::dropdown::domain::value_objects::theme_definition::ThemeDefinition;
use crate::dropdown::domain::value_objects::theme_palette::ThemePalette;
#[cfg(test)]
use std::cell::RefCell;
#[cfg(test)]
use std::collections::BTreeMap;
use std::path::PathBuf;

/// Where the custom themes are kept: a folder with one `NAME.json` file per theme.
pub(crate) trait ThemeRepository {
    /// The themes folder; fails when the data directory is unknown (no home).
    fn folder(&self) -> Result<PathBuf, DropdownError>;
    /// The theme file called `name`, read strictly: `None` when there is no readable file (no home included), an
    /// error when the file is readable but not a valid theme.
    fn find(&self, name: &str) -> Result<Option<ThemeDefinition>, DropdownError>;
    /// The name of every theme file, sorted, without repeats; empty when the folder cannot be listed.
    fn list(&self) -> Vec<String>;
    /// The theme file called `name`, read leniently for drawing (it never goes through `find`): `None` when there is
    /// no readable file, no home, or the text is not valid JSON; any other JSON value gives a palette. Never an
    /// error, never logged, read again at every call.
    fn find_palette(&self, name: &str) -> Option<ThemePalette>;
}

/// Themes kept in memory, for tests. `Default` is a folder-less repository with no theme.
#[cfg(test)]
#[derive(Default)]
pub(crate) struct InMemoryThemeRepository {
    folder: PathBuf,
    themes: BTreeMap<String, Result<ThemeDefinition, String>>,
    palettes: BTreeMap<String, ThemePalette>,
    asked_palettes: RefCell<Vec<String>>,
}

#[cfg(test)]
impl InMemoryThemeRepository {
    pub(crate) fn with_folder(mut self, folder: &str) -> Self {
        self.folder = PathBuf::from(folder);
        self
    }

    /// A valid theme file called `name`.
    pub(crate) fn with_theme(mut self, name: &str, theme: ThemeDefinition) -> Self {
        self.themes.insert(name.to_string(), Ok(theme));
        self
    }

    /// A theme file called `name` that is readable but not valid; `reason` is the parser's text.
    pub(crate) fn with_invalid_theme(mut self, name: &str, reason: &str) -> Self {
        self.themes
            .insert(name.to_string(), Err(reason.to_string()));
        self
    }

    /// The palette `find_palette` answers for `name`.
    pub(crate) fn with_palette(mut self, name: &str, palette: ThemePalette) -> Self {
        self.palettes.insert(name.to_string(), palette);
        self
    }

    /// The names `find_palette` was asked for, in order.
    pub(crate) fn asked_palettes(&self) -> Vec<String> {
        self.asked_palettes.borrow().clone()
    }
}

#[cfg(test)]
impl ThemeRepository for InMemoryThemeRepository {
    fn folder(&self) -> Result<PathBuf, DropdownError> {
        Ok(self.folder.clone())
    }

    fn find(&self, name: &str) -> Result<Option<ThemeDefinition>, DropdownError> {
        match self.themes.get(name) {
            Some(Ok(theme)) => Ok(Some(theme.clone())),
            Some(Err(reason)) => Err(DropdownError::InvalidThemeFile {
                path: self.folder.join(format!("{name}.json")),
                reason: reason.clone(),
            }),
            None => Ok(None),
        }
    }

    fn list(&self) -> Vec<String> {
        self.themes.keys().cloned().collect()
    }

    fn find_palette(&self, name: &str) -> Option<ThemePalette> {
        self.asked_palettes.borrow_mut().push(name.to_string());
        self.palettes.get(name).cloned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dropdown::domain::value_objects::theme_author::ThemeAuthor;
    use crate::dropdown::domain::value_objects::theme_palette::Base16PaletteParts;

    fn by(name: &str) -> ThemeDefinition {
        ThemeDefinition::new(Some(ThemeAuthor::new(Some(name.to_string()), None, None)))
    }

    #[test]
    fn should_answer_the_definition_it_was_given_for_a_name() {
        let themes = InMemoryThemeRepository::default().with_theme("zed", by("Ann"));

        let found = (themes.find("zed").unwrap(), themes.find("alpha").unwrap());

        assert_eq!(found, (Some(by("Ann")), None));
    }

    #[test]
    fn should_list_the_names_it_was_given_in_sorted_order() {
        let themes = InMemoryThemeRepository::default()
            .with_theme("zed", by("Ann"))
            .with_invalid_theme("broken", "expected value at line 1 column 1")
            .with_theme("alpha", by("Bob"));

        let names = themes.list();

        assert_eq!(names, ["alpha", "broken", "zed"]);
    }

    #[test]
    fn should_answer_the_palette_it_was_given_for_a_name() {
        let palette = ThemePalette::base16(Base16PaletteParts {
            shade0: Some((16, 16, 16)),
            shade1: None,
            shade2: None,
            shade5: None,
            shade6: None,
            shade7: None,
            accent6: None,
        });
        let themes = InMemoryThemeRepository::default().with_palette("zed", palette.clone());

        let found = (themes.find_palette("zed"), themes.find_palette("other"));

        assert_eq!(found, (Some(palette), None));
    }
}
