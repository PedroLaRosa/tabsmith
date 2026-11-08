use crate::modules::dropdown::domain::value_objects::theme_author::ThemeAuthor;

/// A custom theme file that passed the strict check of `theme NAME`. Only its author is kept, for the banner: the
/// drawing reads the colours by itself.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ThemeDefinition {
    author: Option<ThemeAuthor>,
}

impl ThemeDefinition {
    pub(crate) fn new(author: Option<ThemeAuthor>) -> Self {
        Self { author }
    }

    pub(crate) fn author(&self) -> Option<&ThemeAuthor> {
        self.author.as_ref()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn should_hold_the_author_of_a_theme_file_when_it_names_one() {
        let author = ThemeAuthor::new(Some("Ann".to_string()), None, None);

        let credited = ThemeDefinition::new(Some(author.clone()));
        let anonymous = ThemeDefinition::new(None);

        assert_eq!(
            (credited.author(), anonymous.author()),
            (Some(&author), None)
        );
    }
}
