use crate::specs::domain::value_objects::filter_strategy::FilterStrategy;
use crate::specs::domain::value_objects::generator::Generator;
use crate::specs::domain::value_objects::spec_load_directive::SpecLoadDirective;
use crate::specs::domain::value_objects::spec_suggestion::SpecSuggestion;
use crate::specs::domain::value_objects::text_or_function::TextOrFunction;

#[derive(Clone, Debug, Default)]
pub(crate) struct Arg {
    name: Option<String>,
    description: Option<String>,
    is_optional: bool,
    is_variadic: bool,
    is_dangerous: bool,
    is_command: bool,
    is_script: bool,
    /// `isModule: "python/"`.
    is_module: Option<String>,
    suggest_current_token: Option<bool>,
    /// `debounce`: true → 200 ms, number → ms.
    debounce: Option<u64>,
    options_can_break_variadic: Option<bool>,
    suggestions: Vec<SpecSuggestion>,
    generators: Vec<Generator>,
    load_spec: Option<SpecLoadDirective>,
    filter_strategy: FilterStrategy,
    /// `parserDirectives.alias` (git aliases).
    alias_directive: Option<TextOrFunction>,
    default: Option<String>,
}

impl Arg {
    pub(crate) fn name(&self) -> &Option<String> {
        &self.name
    }

    pub(crate) fn description(&self) -> &Option<String> {
        &self.description
    }

    pub(crate) fn is_optional(&self) -> bool {
        self.is_optional
    }

    pub(crate) fn is_variadic(&self) -> bool {
        self.is_variadic
    }

    pub(crate) fn is_dangerous(&self) -> bool {
        self.is_dangerous
    }

    pub(crate) fn is_command(&self) -> bool {
        self.is_command
    }

    pub(crate) fn is_script(&self) -> bool {
        self.is_script
    }

    pub(crate) fn is_module(&self) -> &Option<String> {
        &self.is_module
    }

    pub(crate) fn suggest_current_token(&self) -> Option<bool> {
        self.suggest_current_token
    }

    pub(crate) fn debounce(&self) -> Option<u64> {
        self.debounce
    }

    pub(crate) fn options_can_break_variadic(&self) -> Option<bool> {
        self.options_can_break_variadic
    }

    pub(crate) fn suggestions(&self) -> &Vec<SpecSuggestion> {
        &self.suggestions
    }

    pub(crate) fn generators(&self) -> &Vec<Generator> {
        &self.generators
    }

    pub(crate) fn load_spec(&self) -> &Option<SpecLoadDirective> {
        &self.load_spec
    }

    pub(crate) fn filter_strategy(&self) -> FilterStrategy {
        self.filter_strategy
    }

    pub(crate) fn alias_directive(&self) -> &Option<TextOrFunction> {
        &self.alias_directive
    }

    #[cfg(test)]
    pub(crate) fn default_value(&self) -> &Option<String> {
        &self.default
    }

    pub(crate) fn with_name(mut self, value: Option<String>) -> Self {
        self.name = value;
        self
    }

    pub(crate) fn with_description(mut self, value: Option<String>) -> Self {
        self.description = value;
        self
    }

    pub(crate) fn with_is_optional(mut self, value: bool) -> Self {
        self.is_optional = value;
        self
    }

    pub(crate) fn with_is_variadic(mut self, value: bool) -> Self {
        self.is_variadic = value;
        self
    }

    pub(crate) fn with_is_dangerous(mut self, value: bool) -> Self {
        self.is_dangerous = value;
        self
    }

    pub(crate) fn with_is_command(mut self, value: bool) -> Self {
        self.is_command = value;
        self
    }

    pub(crate) fn with_is_script(mut self, value: bool) -> Self {
        self.is_script = value;
        self
    }

    pub(crate) fn with_is_module(mut self, value: Option<String>) -> Self {
        self.is_module = value;
        self
    }

    pub(crate) fn with_suggest_current_token(mut self, value: Option<bool>) -> Self {
        self.suggest_current_token = value;
        self
    }

    pub(crate) fn with_debounce(mut self, value: Option<u64>) -> Self {
        self.debounce = value;
        self
    }

    pub(crate) fn with_options_can_break_variadic(mut self, value: Option<bool>) -> Self {
        self.options_can_break_variadic = value;
        self
    }

    pub(crate) fn with_suggestions(mut self, value: Vec<SpecSuggestion>) -> Self {
        self.suggestions = value;
        self
    }

    pub(crate) fn with_generators(mut self, value: Vec<Generator>) -> Self {
        self.generators = value;
        self
    }

    pub(crate) fn with_load_spec(mut self, value: Option<SpecLoadDirective>) -> Self {
        self.load_spec = value;
        self
    }

    pub(crate) fn with_filter_strategy(mut self, value: FilterStrategy) -> Self {
        self.filter_strategy = value;
        self
    }

    pub(crate) fn with_alias_directive(mut self, value: Option<TextOrFunction>) -> Self {
        self.alias_directive = value;
        self
    }

    pub(crate) fn with_default(mut self, value: Option<String>) -> Self {
        self.default = value;
        self
    }
}
