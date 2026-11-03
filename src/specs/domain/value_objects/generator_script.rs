use crate::specs::domain::value_objects::spec_function_ref::SpecFunctionRef;
use crate::specs::domain::value_objects::spec_value::SpecValue;

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum GeneratorScript {
    /// `["git", "branch"]`
    Argv(Vec<String>),
    /// Legacy string form (`sh -c`).
    Shell(String),
    /// `script(tokens) -> string[] | string | {command,args,...}`.
    Fn(SpecFunctionRef),
    /// `{command, args, cwd?, env?, timeout?}` object form.
    Object(SpecValue),
}
