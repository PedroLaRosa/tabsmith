use crate::dropdown::domain::dropdown_error::DropdownError;
use crate::dropdown::domain::value_objects::theme_definition::ThemeDefinition;
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
}

/// Themes kept in memory, for tests. `Default` is a folder-less repository with no theme.
#[cfg(test)]
#[derive(Default)]
pub(crate) struct InMemoryThemeRepository {
    folder: PathBuf,
    themes: BTreeMap<String, Result<ThemeDefinition, String>>,
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dropdown::domain::value_objects::theme_author::ThemeAuthor;

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
}
