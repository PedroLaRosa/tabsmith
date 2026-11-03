use crate::specs::domain::value_objects::arg::Arg;
use crate::specs::domain::value_objects::repeatability::Repeatability;
use std::rc::Rc;

#[derive(Clone, Debug, Default)]
pub(crate) struct SpecOption {
    names: Vec<String>,
    description: Option<String>,
    display_name: Option<String>,
    insert_value: Option<String>,
    icon: Option<String>,
    priority: Option<f64>,
    hidden: bool,
    is_dangerous: bool,
    args: Vec<Rc<Arg>>,
    is_persistent: bool,
    is_repeatable: Repeatability,
    requires_equals: bool,
    /// `requiresSeparator`: `Some("=")`/`Some(":")`; `true` → `Some("")` (= use first optionArgSeparator).
    requires_separator: Option<String>,
    exclusive_on: Vec<String>,
    depends_on: Vec<String>,
}

impl SpecOption {
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

    pub(crate) fn args(&self) -> &Vec<Rc<Arg>> {
        &self.args
    }

    pub(crate) fn is_persistent(&self) -> bool {
        self.is_persistent
    }

    pub(crate) fn is_repeatable(&self) -> Repeatability {
        self.is_repeatable
    }

    pub(crate) fn requires_equals(&self) -> bool {
        self.requires_equals
    }

    pub(crate) fn requires_separator(&self) -> &Option<String> {
        &self.requires_separator
    }

    pub(crate) fn exclusive_on(&self) -> &Vec<String> {
        &self.exclusive_on
    }

    pub(crate) fn depends_on(&self) -> &Vec<String> {
        &self.depends_on
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

    pub(crate) fn with_args(mut self, value: Vec<Rc<Arg>>) -> Self {
        self.args = value;
        self
    }

    pub(crate) fn with_is_persistent(mut self, value: bool) -> Self {
        self.is_persistent = value;
        self
    }

    pub(crate) fn with_is_repeatable(mut self, value: Repeatability) -> Self {
        self.is_repeatable = value;
        self
    }

    pub(crate) fn with_requires_equals(mut self, value: bool) -> Self {
        self.requires_equals = value;
        self
    }

    pub(crate) fn with_requires_separator(mut self, value: Option<String>) -> Self {
        self.requires_separator = value;
        self
    }

    pub(crate) fn with_exclusive_on(mut self, value: Vec<String>) -> Self {
        self.exclusive_on = value;
        self
    }

    pub(crate) fn with_depends_on(mut self, value: Vec<String>) -> Self {
        self.depends_on = value;
        self
    }
}
