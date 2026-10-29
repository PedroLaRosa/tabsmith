use crate::specs::domain::value_objects::suggestion_kind::SuggestionKind;

/// A static suggestion, `additionalSuggestions` entry or generator result, before filtering.
#[derive(Clone, Debug, Default)]
pub(crate) struct SpecSuggestion {
    names: Vec<String>,
    display_name: Option<String>,
    insert_value: Option<String>,
    description: Option<String>,
    /// `None` → caller decides (static arg suggestions/generator results default to Arg).
    kind: Option<SuggestionKind>,
    priority: Option<f64>,
    hidden: bool,
    is_dangerous: bool,
    /// `context.templateType` (filepaths/folders/help).
    template_type: Option<String>,
    icon: Option<String>,
}

impl SpecSuggestion {
    /// Script-generator string → `{type:"arg", name, insertValue, isDangerous}`.
    pub(crate) fn from_script_output(text: &str, is_dangerous: bool) -> SpecSuggestion {
        SpecSuggestion::default()
            .with_names(vec![text.to_string()])
            .with_insert_value(Some(text.to_string()))
            .with_kind(Some(SuggestionKind::Arg))
            .with_is_dangerous(is_dangerous)
    }

    pub(crate) fn names(&self) -> &Vec<String> {
        &self.names
    }

    pub(crate) fn display_name(&self) -> &Option<String> {
        &self.display_name
    }

    pub(crate) fn insert_value(&self) -> &Option<String> {
        &self.insert_value
    }

    pub(crate) fn description(&self) -> &Option<String> {
        &self.description
    }

    pub(crate) fn kind(&self) -> Option<SuggestionKind> {
        self.kind
    }

    pub(crate) fn priority(&self) -> Option<f64> {
        self.priority
    }

    pub(crate) fn hidden(&self) -> bool {
        self.hidden
    }

    pub(crate) fn is_dangerous(&self) -> bool {
        self.is_dangerous
    }

    pub(crate) fn template_type(&self) -> &Option<String> {
        &self.template_type
    }

    pub(crate) fn icon(&self) -> &Option<String> {
        &self.icon
    }

    pub(crate) fn with_names(mut self, value: Vec<String>) -> Self {
        self.names = value;
        self
    }

    pub(crate) fn with_display_name(mut self, value: Option<String>) -> Self {
        self.display_name = value;
        self
    }

    pub(crate) fn with_insert_value(mut self, value: Option<String>) -> Self {
        self.insert_value = value;
        self
    }

    pub(crate) fn with_description(mut self, value: Option<String>) -> Self {
        self.description = value;
        self
    }

    pub(crate) fn with_kind(mut self, value: Option<SuggestionKind>) -> Self {
        self.kind = value;
        self
    }

    pub(crate) fn with_priority(mut self, value: Option<f64>) -> Self {
        self.priority = value;
        self
    }

    pub(crate) fn with_hidden(mut self, value: bool) -> Self {
        self.hidden = value;
        self
    }

    pub(crate) fn with_is_dangerous(mut self, value: bool) -> Self {
        self.is_dangerous = value;
        self
    }

    pub(crate) fn with_template_type(mut self, value: Option<String>) -> Self {
        self.template_type = value;
        self
    }

    pub(crate) fn with_icon(mut self, value: Option<String>) -> Self {
        self.icon = value;
        self
    }
}
