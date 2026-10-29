use crate::specs::domain::spec_error::SpecError;
use crate::specs::domain::value_objects::generator_context::GeneratorContext;
use crate::specs::domain::value_objects::spec_function_call::SpecFunctionCall;
use crate::specs::domain::value_objects::spec_function_result::SpecFunctionResult;
use std::time::Duration;
#[cfg(test)]
use crate::specs::domain::value_objects::spec_function_ref::SpecFunctionRef;
#[cfg(test)]
use std::collections::HashMap;

/// Runs spec functions.
pub(crate) trait SpecRuntime {
    /// Calls a spec function and reads its result by `call.kind()`.
    fn call(
        &self,
        call: SpecFunctionCall,
        context: Option<&GeneratorContext>,
        timeout: Duration,
    ) -> Result<SpecFunctionResult, SpecError>;
}

#[cfg(test)]
type Answer = Box<dyn Fn(&SpecFunctionCall) -> Result<SpecFunctionResult, SpecError>>;

/// Answers calls with the answers it was given, for tests.
#[cfg(test)]
#[derive(Default)]
pub(crate) struct InMemorySpecRuntime {
    answers: HashMap<SpecFunctionRef, Answer>,
}

#[cfg(test)]
impl InMemorySpecRuntime {
    pub(crate) fn answering(
        mut self,
        function: SpecFunctionRef,
        answer: impl Fn(&SpecFunctionCall) -> Result<SpecFunctionResult, SpecError> + 'static,
    ) -> Self {
        self.answers.insert(function, Box::new(answer));
        self
    }
}

#[cfg(test)]
impl SpecRuntime for InMemorySpecRuntime {
    fn call(
        &self,
        call: SpecFunctionCall,
        _context: Option<&GeneratorContext>,
        _timeout: Duration,
    ) -> Result<SpecFunctionResult, SpecError> {
        (self.answers[&call.function()])(&call)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn should_answer_a_known_function() {
        let runtime = InMemorySpecRuntime::default().answering(2, |_| {
            Ok(SpecFunctionResult::Alias(Some("checkout".into())))
        });
        let call = SpecFunctionCall::Alias {
            function: 2,
            token: "co".into(),
        };

        let result = runtime.call(call, None, Duration::from_secs(5));

        assert!(
            matches!(result, Ok(SpecFunctionResult::Alias(Some(alias))) if alias == "checkout")
        );
    }
}
