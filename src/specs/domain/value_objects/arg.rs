use crate::specs::domain::value_objects::filter_strategy::FilterStrategy;
use crate::specs::domain::value_objects::generator::Generator;
use crate::specs::domain::value_objects::spec_load_directive::SpecLoadDirective;
use crate::specs::domain::value_objects::spec_suggestion::SpecSuggestion;
use crate::specs::domain::value_objects::text_or_function::TextOrFunction;

#[derive(Clone, Debug, Default)]
pub(crate) struct Arg {
    pub(crate) name: Option<String>,
    pub(crate) description: Option<String>,
    pub(crate) is_optional: bool,
    pub(crate) is_variadic: bool,
    pub(crate) is_dangerous: bool,
    pub(crate) is_command: bool,
    pub(crate) is_script: bool,
    /// `isModule: "python/"`.
    pub(crate) is_module: Option<String>,
    pub(crate) suggest_current_token: Option<bool>,
    /// `debounce`: true → 200 ms, number → ms.
    pub(crate) debounce: Option<u64>,
    pub(crate) options_can_break_variadic: Option<bool>,
    pub(crate) suggestions: Vec<SpecSuggestion>,
    pub(crate) generators: Vec<Generator>,
    pub(crate) load_spec: Option<SpecLoadDirective>,
    pub(crate) filter_strategy: FilterStrategy,
    /// `parserDirectives.alias` (git aliases).
    pub(crate) alias_directive: Option<TextOrFunction>,
    pub(crate) default: Option<String>,
}
