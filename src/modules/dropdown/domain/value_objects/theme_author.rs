/// Who wrote a custom theme, as its file credits them; every part is optional.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ThemeAuthor {
    name: Option<String>,
    twitter: Option<String>,
    github: Option<String>,
}

impl ThemeAuthor {
    pub(crate) fn new(
        name: Option<String>,
        twitter: Option<String>,
        github: Option<String>,
    ) -> Self {
        Self {
            name,
            twitter,
            github,
        }
    }

    pub(crate) fn name(&self) -> Option<&str> {
        self.name.as_deref()
    }

    pub(crate) fn twitter(&self) -> Option<&str> {
        self.twitter.as_deref()
    }

    pub(crate) fn github(&self) -> Option<&str> {
        self.github.as_deref()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn should_hold_the_optional_parts_of_a_theme_author() {
        let named = ThemeAuthor::new(Some("Ann".to_string()), None, Some("ann".to_string()));
        let tweeting = ThemeAuthor::new(None, Some("@ann".to_string()), None);

        let parts = [&named, &tweeting].map(|a| (a.name(), a.twitter(), a.github()));

        assert_eq!(
            parts,
            [(Some("Ann"), None, Some("ann")), (None, Some("@ann"), None)]
        );
    }
}
