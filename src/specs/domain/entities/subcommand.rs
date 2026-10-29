use crate::specs::domain::value_objects::arg::Arg;
use crate::specs::domain::value_objects::filter_strategy::FilterStrategy;
use crate::specs::domain::value_objects::parser_directives::ParserDirectives;
use crate::specs::domain::value_objects::spec_function_ref::SpecFunctionRef;
use crate::specs::domain::value_objects::spec_load_directive::SpecLoadDirective;
use crate::specs::domain::value_objects::spec_option::SpecOption;
use crate::specs::domain::value_objects::spec_suggestion::SpecSuggestion;
use crate::specs::domain::value_objects::text_or_function::TextOrFunction;
use std::collections::HashMap;
use std::rc::Rc;

#[derive(Clone, Debug, Default)]
pub(crate) struct Subcommand {
    pub(crate) names: Vec<String>,
    pub(crate) description: Option<String>,
    pub(crate) display_name: Option<String>,
    pub(crate) insert_value: Option<String>,
    pub(crate) icon: Option<String>,
    pub(crate) priority: Option<f64>,
    pub(crate) hidden: bool,
    pub(crate) is_dangerous: bool,
    pub(crate) subcommands: Vec<Rc<Subcommand>>,
    /// Every alias → index into `subcommands`.
    pub(crate) sub_index: HashMap<String, usize>,
    /// Non-persistent options.
    pub(crate) options: Vec<Rc<SpecOption>>,
    /// Options declared `isPersistent` on this level.
    pub(crate) persistent: Vec<Rc<SpecOption>>,
    /// Every option name → (is_persistent, index into `options`/`persistent`).
    pub(crate) opt_index: HashMap<String, (bool, usize)>,
    pub(crate) args: Vec<Rc<Arg>>,
    pub(crate) load_spec: Option<SpecLoadDirective>,
    pub(crate) generate_spec: Option<SpecFunctionRef>,
    pub(crate) generate_spec_cache_key: Option<TextOrFunction>,
    pub(crate) directives: ParserDirectives,
    /// True if `parserDirectives` was given explicitly at this level (for inheritance).
    pub(crate) has_directives: bool,
    pub(crate) additional_suggestions: Vec<SpecSuggestion>,
    pub(crate) filter_strategy: FilterStrategy,
    pub(crate) requires_subcommand: Option<bool>,
}

impl Subcommand {
    /// Rebuild `sub_index`/`opt_index` from the lists (later wins; options win over persistent, as in
    /// `options[t] || persistentOptions[t]`).
    pub(crate) fn reindex(&mut self) {
        self.sub_index.clear();
        for (i, s) in self.subcommands.iter().enumerate() {
            for n in &s.names {
                self.sub_index.insert(n.clone(), i);
            }
        }
        self.opt_index.clear();
        for (persistent, list) in [(true, &self.persistent), (false, &self.options)] {
            for (i, o) in list.iter().enumerate() {
                for n in &o.names {
                    self.opt_index.insert(n.clone(), (persistent, i));
                }
            }
        }
    }
}
