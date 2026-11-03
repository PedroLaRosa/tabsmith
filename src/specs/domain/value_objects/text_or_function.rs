use crate::specs::domain::value_objects::spec_function_ref::SpecFunctionRef;

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum TextOrFunction {
    Str(String),
    Fn(SpecFunctionRef),
}
