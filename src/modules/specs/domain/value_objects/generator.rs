use crate::modules::specs::domain::value_objects::generator_cache_policy::GeneratorCachePolicy;
use crate::modules::specs::domain::value_objects::generator_script::GeneratorScript;
use crate::modules::specs::domain::value_objects::generator_trigger::GeneratorTrigger;
use crate::modules::specs::domain::value_objects::spec_function_ref::SpecFunctionRef;
use crate::modules::specs::domain::value_objects::text_or_function::TextOrFunction;

#[derive(Clone, Debug, Default)]
pub(crate) struct Generator {
    /// `filepaths` | `folders` | `history` | `help` (from `template`).
    template: Vec<String>,
    script: Option<GeneratorScript>,
    post_process: Option<SpecFunctionRef>,
    split_on: Option<String>,
    custom: Option<SpecFunctionRef>,
    trigger: Option<GeneratorTrigger>,
    get_query_term: Option<TextOrFunction>,
    cache: Option<GeneratorCachePolicy>,
    script_timeout: Option<u64>,
    filter_template_suggestions: Option<SpecFunctionRef>,
}

impl Generator {
    pub(crate) fn template(&self) -> &Vec<String> {
        &self.template
    }

    pub(crate) fn script(&self) -> &Option<GeneratorScript> {
        &self.script
    }

    pub(crate) fn post_process(&self) -> Option<SpecFunctionRef> {
        self.post_process
    }

    pub(crate) fn split_on(&self) -> &Option<String> {
        &self.split_on
    }

    pub(crate) fn custom(&self) -> Option<SpecFunctionRef> {
        self.custom
    }

    pub(crate) fn trigger(&self) -> &Option<GeneratorTrigger> {
        &self.trigger
    }

    pub(crate) fn get_query_term(&self) -> &Option<TextOrFunction> {
        &self.get_query_term
    }

    pub(crate) fn cache(&self) -> &Option<GeneratorCachePolicy> {
        &self.cache
    }

    pub(crate) fn script_timeout(&self) -> Option<u64> {
        self.script_timeout
    }

    pub(crate) fn filter_template_suggestions(&self) -> Option<SpecFunctionRef> {
        self.filter_template_suggestions
    }

    pub(crate) fn with_template(mut self, value: Vec<String>) -> Self {
        self.template = value;
        self
    }

    pub(crate) fn with_script(mut self, value: Option<GeneratorScript>) -> Self {
        self.script = value;
        self
    }

    pub(crate) fn with_post_process(mut self, value: Option<SpecFunctionRef>) -> Self {
        self.post_process = value;
        self
    }

    pub(crate) fn with_split_on(mut self, value: Option<String>) -> Self {
        self.split_on = value;
        self
    }

    pub(crate) fn with_custom(mut self, value: Option<SpecFunctionRef>) -> Self {
        self.custom = value;
        self
    }

    pub(crate) fn with_trigger(mut self, value: Option<GeneratorTrigger>) -> Self {
        self.trigger = value;
        self
    }

    pub(crate) fn with_get_query_term(mut self, value: Option<TextOrFunction>) -> Self {
        self.get_query_term = value;
        self
    }

    pub(crate) fn with_cache(mut self, value: Option<GeneratorCachePolicy>) -> Self {
        self.cache = value;
        self
    }

    pub(crate) fn with_script_timeout(mut self, value: Option<u64>) -> Self {
        self.script_timeout = value;
        self
    }

    pub(crate) fn with_filter_template_suggestions(mut self, value: Option<SpecFunctionRef>) -> Self {
        self.filter_template_suggestions = value;
        self
    }
}
