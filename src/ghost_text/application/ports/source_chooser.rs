use crate::ghost_text::domain::ghost_text_error::GhostTextError;
use crate::ghost_text::domain::value_objects::llm_profile::LlmProfile;
#[cfg(test)]
use std::cell::RefCell;

/// Asks the user to pick a built-in source or one of the given (valid) profiles.
pub(crate) trait SourceChooser {
    /// `Ok(None)` is the "None" row: the user wants no customization.
    fn choose(&self, profiles: &[(String, LlmProfile)]) -> Result<Option<String>, GhostTextError>;
}

/// Always picks the same answer and remembers the profile names it was offered, for tests.
#[cfg(test)]
pub(crate) struct FixedSourceChooser {
    choice: Option<String>,
    offered: RefCell<Vec<String>>,
}

#[cfg(test)]
impl FixedSourceChooser {
    pub(crate) fn choosing(choice: Option<&str>) -> Self {
        Self {
            choice: choice.map(String::from),
            offered: RefCell::new(Vec::new()),
        }
    }

    /// The profile names of every call, in order.
    pub(crate) fn offered(&self) -> Vec<String> {
        self.offered.borrow().clone()
    }
}

#[cfg(test)]
impl SourceChooser for FixedSourceChooser {
    fn choose(&self, profiles: &[(String, LlmProfile)]) -> Result<Option<String>, GhostTextError> {
        self.offered
            .borrow_mut()
            .extend(profiles.iter().map(|(name, _)| name.clone()));
        Ok(self.choice.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ghost_text::domain::value_objects::llm_profile::LlmProfileParts;
    use crate::ghost_text::domain::value_objects::llm_provider::LlmProvider;

    fn profile(name: &str) -> (String, LlmProfile) {
        let profile = LlmProfile::new(LlmProfileParts {
            name: name.to_string(),
            provider: LlmProvider::OpenAi,
            model: "gpt".to_string(),
            base_url: "https://api.openai.com/v1".to_string(),
            api_key_env: "OPENAI_API_KEY".to_string(),
            max_tokens: 64,
            effort: None,
        });
        (name.to_string(), profile)
    }

    #[test]
    fn should_choose_what_it_was_given() {
        let chooser = FixedSourceChooser::choosing(Some("history"));

        let chosen = chooser.choose(&[]).unwrap();

        assert_eq!(chosen.as_deref(), Some("history"));
    }

    #[test]
    fn should_remember_the_profiles_it_was_offered() {
        let chooser = FixedSourceChooser::choosing(None);

        chooser.choose(&[profile("a"), profile("b")]).unwrap();

        assert_eq!(chooser.offered(), ["a", "b"]);
    }
}
