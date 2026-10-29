use crate::specs::domain::value_objects::spec_function_ref::SpecFunctionRef;
use crate::specs::domain::value_objects::template_suggestions::TemplateSuggestions;

/// A typed call into a spec function: which function, and the arguments its kind passes.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum SpecFunctionCall {
    /// A generator's `getQueryTerm(searchTerm)`.
    QueryTerm { function: SpecFunctionRef, search_term: String },
    /// A generator's `trigger(newToken, oldToken)`.
    Trigger {
        function: SpecFunctionRef,
        new_token: String,
        old_token: String,
    },
    /// A function alias `(token, executeShellCommand)`.
    Alias { function: SpecFunctionRef, token: String },
    /// A function `loadSpec(token, executeShellCommand)`.
    LoadSpec { function: SpecFunctionRef, token: String },
    /// A `generateSpec` cache-key function, given `{ tokens }` and no `executeShellCommand`.
    GenerateSpecCacheKey {
        function: SpecFunctionRef,
        tokens: Vec<String>,
    },
    /// `generateSpec(tokens, executeShellCommand)`.
    GenerateSpec {
        function: SpecFunctionRef,
        tokens: Vec<String>,
    },
    /// A generator's function `script(tokens)`.
    Script {
        function: SpecFunctionRef,
        tokens: Vec<String>,
    },
    /// A generator's `custom(tokens, executeShellCommand, generatorContext)`.
    Custom {
        function: SpecFunctionRef,
        tokens: Vec<String>,
    },
    /// A generator's `postProcess(stdout, tokens)`.
    PostProcess {
        function: SpecFunctionRef,
        stdout: String,
        tokens: Vec<String>,
    },
    /// A generator's `filterTemplateSuggestions(suggestions)`.
    FilterTemplateSuggestions {
        function: SpecFunctionRef,
        suggestions: TemplateSuggestions,
    },
}

impl SpecFunctionCall {
    pub(crate) fn function(&self) -> SpecFunctionRef {
        match self {
            SpecFunctionCall::QueryTerm { function, .. }
            | SpecFunctionCall::Trigger { function, .. }
            | SpecFunctionCall::Alias { function, .. }
            | SpecFunctionCall::LoadSpec { function, .. }
            | SpecFunctionCall::GenerateSpecCacheKey { function, .. }
            | SpecFunctionCall::GenerateSpec { function, .. }
            | SpecFunctionCall::Script { function, .. }
            | SpecFunctionCall::Custom { function, .. }
            | SpecFunctionCall::PostProcess { function, .. }
            | SpecFunctionCall::FilterTemplateSuggestions { function, .. } => *function,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn should_name_the_function_it_calls() {
        let call = SpecFunctionCall::PostProcess {
            function: 7,
            stdout: "main\n".into(),
            tokens: vec!["git".into(), "checkout".into()],
        };

        let function = call.function();

        assert_eq!(function, 7);
    }
}
