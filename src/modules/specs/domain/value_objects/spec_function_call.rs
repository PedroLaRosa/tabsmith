use crate::modules::specs::domain::value_objects::spec_function_kind::SpecFunctionKind;
use crate::modules::specs::domain::value_objects::spec_function_ref::SpecFunctionRef;
use crate::modules::specs::domain::value_objects::template_suggestions::TemplateSuggestions;

/// A typed call into a spec function: which function, and the arguments its kind passes.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum SpecFunctionCall {
    /// A generator's `getQueryTerm(searchTerm)`.
    QueryTerm {
        function: SpecFunctionRef,
        search_term: String,
    },
    /// A generator's `trigger(newToken, oldToken)`.
    Trigger {
        function: SpecFunctionRef,
        new_token: String,
        old_token: String,
    },
    /// A function alias `(token, executeShellCommand)`.
    Alias {
        function: SpecFunctionRef,
        token: String,
    },
    /// A function `loadSpec(token, executeShellCommand)`.
    LoadSpec {
        function: SpecFunctionRef,
        token: String,
    },
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

    pub(crate) fn kind(&self) -> SpecFunctionKind {
        match self {
            SpecFunctionCall::QueryTerm { .. } => SpecFunctionKind::QueryTerm,
            SpecFunctionCall::Trigger { .. } => SpecFunctionKind::Trigger,
            SpecFunctionCall::Alias { .. } => SpecFunctionKind::Alias,
            SpecFunctionCall::LoadSpec { .. } => SpecFunctionKind::LoadSpec,
            SpecFunctionCall::GenerateSpecCacheKey { .. } => SpecFunctionKind::GenerateSpecCacheKey,
            SpecFunctionCall::GenerateSpec { .. } => SpecFunctionKind::GenerateSpec,
            SpecFunctionCall::Script { .. } => SpecFunctionKind::Script,
            SpecFunctionCall::Custom { .. } => SpecFunctionKind::Custom,
            SpecFunctionCall::PostProcess { .. } => SpecFunctionKind::PostProcess,
            SpecFunctionCall::FilterTemplateSuggestions { .. } => {
                SpecFunctionKind::FilterTemplateSuggestions
            }
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

    #[test]
    fn should_tell_the_kind_of_each_call() {
        let tokens = || vec!["git".to_string()];
        let calls = vec![
            SpecFunctionCall::QueryTerm {
                function: 0,
                search_term: "ma".into(),
            },
            SpecFunctionCall::Trigger {
                function: 1,
                new_token: "a/".into(),
                old_token: "a".into(),
            },
            SpecFunctionCall::Alias {
                function: 2,
                token: "co".into(),
            },
            SpecFunctionCall::LoadSpec {
                function: 3,
                token: "run".into(),
            },
            SpecFunctionCall::GenerateSpecCacheKey {
                function: 4,
                tokens: tokens(),
            },
            SpecFunctionCall::GenerateSpec {
                function: 5,
                tokens: tokens(),
            },
            SpecFunctionCall::Script {
                function: 6,
                tokens: tokens(),
            },
            SpecFunctionCall::Custom {
                function: 7,
                tokens: tokens(),
            },
            SpecFunctionCall::PostProcess {
                function: 8,
                stdout: String::new(),
                tokens: tokens(),
            },
            SpecFunctionCall::FilterTemplateSuggestions {
                function: 9,
                suggestions: TemplateSuggestions::Listed(Vec::new()),
            },
        ];

        let kinds: Vec<SpecFunctionKind> = calls.iter().map(SpecFunctionCall::kind).collect();

        assert_eq!(
            kinds,
            vec![
                SpecFunctionKind::QueryTerm,
                SpecFunctionKind::Trigger,
                SpecFunctionKind::Alias,
                SpecFunctionKind::LoadSpec,
                SpecFunctionKind::GenerateSpecCacheKey,
                SpecFunctionKind::GenerateSpec,
                SpecFunctionKind::Script,
                SpecFunctionKind::Custom,
                SpecFunctionKind::PostProcess,
                SpecFunctionKind::FilterTemplateSuggestions,
            ]
        );
    }
}
