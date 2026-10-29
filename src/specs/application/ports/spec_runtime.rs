use crate::specs::domain::spec_error::SpecError;
use crate::specs::domain::value_objects::generator_context::GeneratorContext;
use crate::specs::domain::value_objects::spec_function_call::SpecFunctionCall;
use crate::specs::domain::value_objects::spec_function_result::SpecFunctionResult;
use std::time::Duration;
#[cfg(test)]
use crate::specs::domain::value_objects::spec_function_ref::SpecFunctionRef;
#[cfg(test)]
use std::cell::RefCell;
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

/// Answers calls with the answers it was given, and records them, for tests.
#[cfg(test)]
#[derive(Default)]
pub(crate) struct InMemorySpecRuntime {
    answers: HashMap<SpecFunctionRef, Answer>,
    calls: RefCell<Vec<SpecFunctionCall>>,
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

    /// The calls received so far, in order.
    pub(crate) fn calls(&self) -> Vec<SpecFunctionCall> {
        self.calls.borrow().clone()
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
        self.calls.borrow_mut().push(call.clone());
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

    #[test]
    fn should_record_calls_in_order() {
        let runtime = InMemorySpecRuntime::default()
            .answering(0, |_| Ok(SpecFunctionResult::QueryTerm(None)))
            .answering(1, |_| Ok(SpecFunctionResult::Trigger(true)));
        let query_term = SpecFunctionCall::QueryTerm {
            function: 0,
            search_term: "ma".into(),
        };
        let trigger = SpecFunctionCall::Trigger {
            function: 1,
            new_token: "a/".into(),
            old_token: "a".into(),
        };

        let _ = runtime.call(trigger.clone(), None, Duration::from_secs(5));
        let _ = runtime.call(query_term.clone(), None, Duration::from_secs(5));

        assert_eq!(runtime.calls(), vec![trigger, query_term]);
    }
}
