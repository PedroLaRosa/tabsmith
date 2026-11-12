use crate::modules::specs::domain::value_objects::exec_request::ExecRequest;

/// What a generator's script turned into.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum ScriptResolution {
    /// Nothing to run (a falsy value, an empty array or string, a number, a bool): the generator suggests nothing.
    Skip,
    /// The script is not an exec input, as the serde text; the caller prefixes `bad script input: `.
    Invalid(String),
    /// The process to run, and the text a cache key is built from (the input before its timeout is set).
    Ready {
        request: ExecRequest,
        cache_key_source: String,
    },
}
