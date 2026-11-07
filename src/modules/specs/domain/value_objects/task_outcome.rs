use crate::modules::specs::domain::value_objects::spec_value::SpecValue;

/// The raw result of a started task: the value it gave, or its error text.
pub(crate) type TaskOutcome = Result<SpecValue, String>;
