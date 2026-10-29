use crate::completion::domain::value_objects::generator_key::GeneratorKey;
use crate::completion::domain::value_objects::suggestion_query::SuggestionQuery;
use crate::specs::domain::entities::subcommand::Subcommand;
use crate::specs::domain::value_objects::arg::Arg;
use crate::specs::domain::value_objects::spec_option::SpecOption;
use crate::specs::domain::value_objects::spec_suggestion::SpecSuggestion;
use crate::specs::domain::value_objects::suggestion_kind::SuggestionKind;
use std::rc::Rc;

/// A suggestion row inside the pipeline (a spec's `SpecSuggestion` + internal fields), before it becomes a `Suggestion`.
#[derive(Clone, Debug)]
pub(crate) struct SuggestionCandidate {
    names: Vec<String>,
    display_name: Option<String>,
    insert_value: Option<String>,
    description: Option<String>,
    kind: SuggestionKind,
    original_kind: Option<SuggestionKind>,
    /// 0 = unset (`0` and unset are treated alike).
    priority: f64,
    hidden: bool,
    is_dangerous: bool,
    /// Args of the subcommand/option (auto-execute rule, args hint, dedupe).
    args: Vec<Rc<Arg>>,
    should_add_space: bool,
    separator: Option<String>,
    query: SuggestionQuery,
    generator: Option<GeneratorKey>,
    template_type: Option<String>,
    /// Spec `icon` (emoji or `fig://icon?type=`), resolved by the renderer.
    icon: Option<String>,
}

impl SuggestionCandidate {
    pub(crate) fn new(name: impl Into<String>, kind: SuggestionKind) -> SuggestionCandidate {
        SuggestionCandidate {
            names: vec![name.into()],
            display_name: None,
            insert_value: None,
            description: None,
            kind,
            original_kind: None,
            priority: 0.0,
            hidden: false,
            is_dangerous: false,
            args: Vec::new(),
            should_add_space: false,
            separator: None,
            query: SuggestionQuery::Term,
            generator: None,
            template_type: None,
            icon: None,
        }
    }

    pub(crate) fn first(&self) -> &str {
        self.names.first().map(String::as_str).unwrap_or("")
    }

    pub(crate) fn from_sub(s: &Subcommand) -> SuggestionCandidate {
        let mandatory = s.args().first().is_some_and(|a| !a.is_optional());
        SuggestionCandidate {
            names: s.names().clone(),
            display_name: s.display_name().clone(),
            insert_value: s.insert_value().clone(),
            description: s.description().clone(),
            icon: s.icon().clone(),
            priority: s.priority().unwrap_or(0.0),
            hidden: s.hidden(),
            is_dangerous: s.is_dangerous(),
            args: s.args().clone(),
            // SPEC §6.7.1 should_add_space
            should_add_space: mandatory
                || s.requires_subcommand().unwrap_or(!s.subcommands().is_empty()),
            ..SuggestionCandidate::new("", SuggestionKind::Subcommand)
        }
    }

    pub(crate) fn from_opt(o: &SpecOption, separators: &[String]) -> SuggestionCandidate {
        let separator = if o.args().first().is_some_and(|a| a.is_optional()) {
            None
        } else {
            match o.requires_separator() {
                Some(s) if !s.is_empty() => Some(s.clone()),
                Some(_) => Some(separators.first().cloned().unwrap_or_else(|| "=".into())),
                None => o.requires_equals().then(|| "=".to_string()),
            }
        };
        SuggestionCandidate {
            names: o.names().clone(),
            display_name: o.display_name().clone(),
            insert_value: o.insert_value().clone(),
            description: o.description().clone(),
            icon: o.icon().clone(),
            priority: o.priority().unwrap_or(0.0),
            hidden: o.hidden(),
            is_dangerous: o.is_dangerous(),
            args: o.args().clone(),
            should_add_space: o.args().first().is_some_and(|a| !a.is_optional()),
            separator,
            ..SuggestionCandidate::new("", SuggestionKind::Option)
        }
    }

    /// Static/additional/generator suggestion (`type` defaults to `arg`).
    pub(crate) fn from_suggestion(
        s: &SpecSuggestion,
        dangerous: bool,
        generator: Option<GeneratorKey>,
        query: SuggestionQuery,
    ) -> SuggestionCandidate {
        SuggestionCandidate {
            names: s.names().clone(),
            display_name: s.display_name().clone(),
            insert_value: s.insert_value().clone(),
            description: s.description().clone(),
            priority: s.priority().unwrap_or(0.0),
            hidden: s.hidden(),
            is_dangerous: s.is_dangerous() || dangerous,
            query,
            generator,
            template_type: s.template_type().clone(),
            icon: s.icon().clone(),
            ..SuggestionCandidate::new("", s.kind().unwrap_or(SuggestionKind::Arg))
        }
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

    pub(crate) fn kind(&self) -> SuggestionKind {
        self.kind
    }

    pub(crate) fn original_kind(&self) -> Option<SuggestionKind> {
        self.original_kind
    }

    pub(crate) fn priority(&self) -> f64 {
        self.priority
    }

    pub(crate) fn hidden(&self) -> bool {
        self.hidden
    }

    pub(crate) fn is_dangerous(&self) -> bool {
        self.is_dangerous
    }

    pub(crate) fn args(&self) -> &Vec<Rc<Arg>> {
        &self.args
    }

    pub(crate) fn should_add_space(&self) -> bool {
        self.should_add_space
    }

    pub(crate) fn separator(&self) -> &Option<String> {
        &self.separator
    }

    pub(crate) fn query(&self) -> &SuggestionQuery {
        &self.query
    }

    pub(crate) fn generator(&self) -> Option<GeneratorKey> {
        self.generator
    }

    pub(crate) fn template_type(&self) -> &Option<String> {
        &self.template_type
    }

    pub(crate) fn icon(&self) -> &Option<String> {
        &self.icon
    }

    pub(crate) fn with_names(mut self, names: Vec<String>) -> Self {
        self.names = names;
        self
    }

    pub(crate) fn with_insert_value(mut self, insert_value: Option<String>) -> Self {
        self.insert_value = insert_value;
        self
    }

    pub(crate) fn with_description(mut self, description: Option<String>) -> Self {
        self.description = description;
        self
    }

    pub(crate) fn with_kind(mut self, kind: SuggestionKind) -> Self {
        self.kind = kind;
        self
    }

    pub(crate) fn with_original_kind(mut self, original_kind: Option<SuggestionKind>) -> Self {
        self.original_kind = original_kind;
        self
    }

    pub(crate) fn with_priority(mut self, priority: f64) -> Self {
        self.priority = priority;
        self
    }

    #[cfg(test)]
    pub(crate) fn with_display_name(mut self, display_name: Option<String>) -> Self {
        self.display_name = display_name;
        self
    }

    #[cfg(test)]
    pub(crate) fn with_is_dangerous(mut self, is_dangerous: bool) -> Self {
        self.is_dangerous = is_dangerous;
        self
    }

    #[cfg(test)]
    pub(crate) fn with_args(mut self, args: Vec<Rc<Arg>>) -> Self {
        self.args = args;
        self
    }

    #[cfg(test)]
    pub(crate) fn with_should_add_space(mut self, should_add_space: bool) -> Self {
        self.should_add_space = should_add_space;
        self
    }

    #[cfg(test)]
    pub(crate) fn with_separator(mut self, separator: Option<String>) -> Self {
        self.separator = separator;
        self
    }

    #[cfg(test)]
    pub(crate) fn with_query(mut self, query: SuggestionQuery) -> Self {
        self.query = query;
        self
    }

    #[cfg(test)]
    pub(crate) fn with_template_type(mut self, template_type: Option<String>) -> Self {
        self.template_type = template_type;
        self
    }

    /// In-place writes for the loops that rewrite rows where they stand (`update_priorities`, `option_chain`).
    pub(crate) fn set_names(&mut self, names: Vec<String>) {
        self.names = names;
    }

    pub(crate) fn set_insert_value(&mut self, insert_value: Option<String>) {
        self.insert_value = insert_value;
    }

    pub(crate) fn set_priority(&mut self, priority: f64) {
        self.priority = priority;
    }

    pub(crate) fn set_query(&mut self, query: SuggestionQuery) {
        self.query = query;
    }
}
