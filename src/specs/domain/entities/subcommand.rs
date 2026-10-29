use crate::specs::domain::value_objects::arg::Arg;
use crate::specs::domain::value_objects::filter_strategy::FilterStrategy;
use crate::specs::domain::value_objects::parser_directives::ParserDirectives;
use crate::specs::domain::value_objects::spec_function_ref::SpecFunctionRef;
use crate::specs::domain::value_objects::spec_load_directive::SpecLoadDirective;
use crate::specs::domain::value_objects::spec_option::SpecOption;
use crate::specs::domain::value_objects::spec_suggestion::SpecSuggestion;
use crate::specs::domain::value_objects::text_or_function::TextOrFunction;
use std::collections::{HashMap, HashSet};
use std::rc::Rc;

#[derive(Clone, Debug, Default)]
pub(crate) struct Subcommand {
    names: Vec<String>,
    description: Option<String>,
    display_name: Option<String>,
    insert_value: Option<String>,
    icon: Option<String>,
    priority: Option<f64>,
    hidden: bool,
    is_dangerous: bool,
    subcommands: Vec<Rc<Subcommand>>,
    /// Every alias → index into `subcommands`.
    sub_index: HashMap<String, usize>,
    /// Non-persistent options.
    options: Vec<Rc<SpecOption>>,
    /// Options declared `isPersistent` on this level.
    persistent: Vec<Rc<SpecOption>>,
    /// Every option name → (is_persistent, index into `options`/`persistent`).
    opt_index: HashMap<String, (bool, usize)>,
    args: Vec<Rc<Arg>>,
    load_spec: Option<SpecLoadDirective>,
    generate_spec: Option<SpecFunctionRef>,
    generate_spec_cache_key: Option<TextOrFunction>,
    directives: ParserDirectives,
    /// True if `parserDirectives` was given explicitly at this level (for inheritance).
    has_directives: bool,
    additional_suggestions: Vec<SpecSuggestion>,
    filter_strategy: FilterStrategy,
    requires_subcommand: Option<bool>,
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
                for n in o.names() {
                    self.opt_index.insert(n.clone(), (persistent, i));
                }
            }
        }
    }

    pub(crate) fn names(&self) -> &Vec<String> {
        &self.names
    }

    pub(crate) fn description(&self) -> &Option<String> {
        &self.description
    }

    pub(crate) fn display_name(&self) -> &Option<String> {
        &self.display_name
    }

    pub(crate) fn insert_value(&self) -> &Option<String> {
        &self.insert_value
    }

    pub(crate) fn icon(&self) -> &Option<String> {
        &self.icon
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

    pub(crate) fn subcommands(&self) -> &Vec<Rc<Subcommand>> {
        &self.subcommands
    }

    pub(crate) fn sub_index(&self) -> &HashMap<String, usize> {
        &self.sub_index
    }

    pub(crate) fn options(&self) -> &Vec<Rc<SpecOption>> {
        &self.options
    }

    pub(crate) fn persistent(&self) -> &Vec<Rc<SpecOption>> {
        &self.persistent
    }

    pub(crate) fn opt_index(&self) -> &HashMap<String, (bool, usize)> {
        &self.opt_index
    }

    pub(crate) fn args(&self) -> &Vec<Rc<Arg>> {
        &self.args
    }

    pub(crate) fn load_spec(&self) -> &Option<SpecLoadDirective> {
        &self.load_spec
    }

    pub(crate) fn generate_spec(&self) -> Option<SpecFunctionRef> {
        self.generate_spec
    }

    pub(crate) fn generate_spec_cache_key(&self) -> &Option<TextOrFunction> {
        &self.generate_spec_cache_key
    }

    pub(crate) fn directives(&self) -> &ParserDirectives {
        &self.directives
    }

    pub(crate) fn has_directives(&self) -> bool {
        self.has_directives
    }

    pub(crate) fn additional_suggestions(&self) -> &Vec<SpecSuggestion> {
        &self.additional_suggestions
    }

    pub(crate) fn filter_strategy(&self) -> FilterStrategy {
        self.filter_strategy
    }

    pub(crate) fn requires_subcommand(&self) -> Option<bool> {
        self.requires_subcommand
    }

    pub(crate) fn with_names(mut self, value: Vec<String>) -> Self {
        self.names = value;
        self
    }

    pub(crate) fn with_description(mut self, value: Option<String>) -> Self {
        self.description = value;
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

    pub(crate) fn with_icon(mut self, value: Option<String>) -> Self {
        self.icon = value;
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

    pub(crate) fn with_subcommands(mut self, value: Vec<Rc<Subcommand>>) -> Self {
        self.subcommands = value;
        self
    }

    pub(crate) fn with_options(mut self, value: Vec<Rc<SpecOption>>) -> Self {
        self.options = value;
        self
    }

    pub(crate) fn with_persistent(mut self, value: Vec<Rc<SpecOption>>) -> Self {
        self.persistent = value;
        self
    }

    pub(crate) fn with_args(mut self, value: Vec<Rc<Arg>>) -> Self {
        self.args = value;
        self
    }

    pub(crate) fn with_load_spec(mut self, value: Option<SpecLoadDirective>) -> Self {
        self.load_spec = value;
        self
    }

    pub(crate) fn with_generate_spec(mut self, value: Option<SpecFunctionRef>) -> Self {
        self.generate_spec = value;
        self
    }

    pub(crate) fn with_generate_spec_cache_key(mut self, value: Option<TextOrFunction>) -> Self {
        self.generate_spec_cache_key = value;
        self
    }

    pub(crate) fn with_directives(mut self, value: ParserDirectives) -> Self {
        self.directives = value;
        self.has_directives = true;
        self
    }

    pub(crate) fn with_additional_suggestions(mut self, value: Vec<SpecSuggestion>) -> Self {
        self.additional_suggestions = value;
        self
    }

    pub(crate) fn with_filter_strategy(mut self, value: FilterStrategy) -> Self {
        self.filter_strategy = value;
        self
    }

    pub(crate) fn with_requires_subcommand(mut self, value: Option<bool>) -> Self {
        self.requires_subcommand = value;
        self
    }

    /// The generateSpec merge (SPEC §6.4): this level with `generated` merged in by name, its args
    /// taken only when this level has none.
    pub(crate) fn merged_with_generated(&self, generated: &Subcommand) -> Subcommand {
        let mut c = self.clone();
        c.subcommands = merge(&c.subcommands, &generated.subcommands, |s| &s.names);
        c.options = merge(&c.options, &generated.options, |o| o.names());
        c.persistent = merge(&c.persistent, &generated.persistent, |o| o.names());
        if c.args.is_empty() {
            c.args = generated.args.clone();
        }
        c.reindex();
        c
    }
}

// Object-merge by name, generated wins (later entries win in `reindex`; fully shadowed ones are dropped).
fn merge<T>(base: &[Rc<T>], generator: &[Rc<T>], names: fn(&T) -> &[String]) -> Vec<Rc<T>> {
    let shadowed: HashSet<&String> = generator.iter().flat_map(|g| names(g)).collect();
    let kept = base
        .iter()
        .filter(|b| !names(b).iter().all(|n| shadowed.contains(n)));
    kept.chain(generator).cloned().collect()
}
