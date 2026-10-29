use crate::specs::domain::value_objects::arg::Arg;
use crate::specs::domain::value_objects::repeatability::Repeatability;
use std::rc::Rc;

#[derive(Clone, Debug, Default)]
pub(crate) struct SpecOption {
    pub(crate) names: Vec<String>,
    pub(crate) description: Option<String>,
    pub(crate) display_name: Option<String>,
    pub(crate) insert_value: Option<String>,
    pub(crate) icon: Option<String>,
    pub(crate) priority: Option<f64>,
    pub(crate) hidden: bool,
    pub(crate) is_dangerous: bool,
    pub(crate) args: Vec<Rc<Arg>>,
    pub(crate) is_persistent: bool,
    pub(crate) is_repeatable: Repeatability,
    pub(crate) requires_equals: bool,
    /// `requiresSeparator`: `Some("=")`/`Some(":")`; `true` → `Some("")` (= use first optionArgSeparator).
    pub(crate) requires_separator: Option<String>,
    pub(crate) exclusive_on: Vec<String>,
    pub(crate) depends_on: Vec<String>,
}
