use crate::specs::domain::value_objects::generator_cache_policy::GeneratorCachePolicy;
use crate::specs::domain::value_objects::generator_script::GeneratorScript;
use crate::specs::domain::value_objects::generator_trigger::GeneratorTrigger;
use crate::specs::domain::value_objects::spec_function_ref::SpecFunctionRef;
use crate::specs::domain::value_objects::text_or_function::TextOrFunction;

#[derive(Clone, Debug, Default)]
pub(crate) struct Generator {
    /// `filepaths` | `folders` | `history` | `help` (from `template`).
    pub(crate) template: Vec<String>,
    pub(crate) script: Option<GeneratorScript>,
    pub(crate) post_process: Option<SpecFunctionRef>,
    pub(crate) split_on: Option<String>,
    pub(crate) custom: Option<SpecFunctionRef>,
    pub(crate) trigger: Option<GeneratorTrigger>,
    pub(crate) get_query_term: Option<TextOrFunction>,
    pub(crate) cache: Option<GeneratorCachePolicy>,
    pub(crate) script_timeout: Option<u64>,
    pub(crate) filter_template_suggestions: Option<SpecFunctionRef>,
}
