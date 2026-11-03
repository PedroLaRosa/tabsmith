use crate::specs::domain::value_objects::spec_suggestion::SpecSuggestion;

/// A custom generator's suggestions, and whether they are templated: its first item has a `context` key, even null.
#[derive(Clone, Debug)]
pub(crate) struct CustomGeneratorResult {
    templated: bool,
    suggestions: Vec<SpecSuggestion>,
}

impl CustomGeneratorResult {
    pub(crate) fn new(templated: bool, suggestions: Vec<SpecSuggestion>) -> Self {
        CustomGeneratorResult {
            templated,
            suggestions,
        }
    }

    pub(crate) fn is_templated(&self) -> bool {
        self.templated
    }

    pub(crate) fn suggestions(&self) -> &Vec<SpecSuggestion> {
        &self.suggestions
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn should_keep_its_suggestions_and_whether_they_are_templated() {
        let suggestions = vec![SpecSuggestion::default().with_names(vec!["main".into()])];

        let result = CustomGeneratorResult::new(true, suggestions.clone());

        assert!(result.is_templated());
        assert_eq!(result.suggestions(), &suggestions);
    }
}
