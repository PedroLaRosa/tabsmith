use crate::specs::application::ports::interrupted::Interrupted;
use crate::specs::domain::spec_error::SpecError;
use crate::specs::domain::value_objects::exec_request::ExecRequest;
use crate::specs::domain::value_objects::generator_context::GeneratorContext;
use crate::specs::domain::value_objects::spec_function_call::SpecFunctionCall;
use crate::specs::domain::value_objects::spec_function_result::SpecFunctionResult;
use crate::specs::domain::value_objects::task_id::TaskId;
use crate::specs::domain::value_objects::task_outcome::TaskOutcome;
use std::time::{Duration, Instant};
#[cfg(test)]
use crate::specs::domain::value_objects::spec_function_ref::SpecFunctionRef;
#[cfg(test)]
use std::cell::{Cell, RefCell};
#[cfg(test)]
use std::collections::HashMap;

/// Runs spec functions, processes and sleeps as tasks, and waits for them.
pub(crate) trait SpecRuntime {
    /// Starts a spec function call. It cannot fail: a failure to start becomes the task's result.
    fn start(&self, call: SpecFunctionCall, context: Option<&GeneratorContext>) -> TaskId;
    /// Starts a process, run with the context's directory and environment.
    fn start_exec(&self, request: ExecRequest, context: &GeneratorContext) -> TaskId;
    /// Starts a task that finishes, with null, after `ms` milliseconds (a generator's debounce).
    fn start_sleep(&self, ms: u64) -> TaskId;
    /// Waits until every task finished or `deadline`, and gives one outcome per task: `None` while it runs. A
    /// finished task's outcome is given once; an unknown or forgotten task gives `Some(Err("unknown task {t}"))`.
    /// `Interrupted` when a newer request is waiting; no task is dropped then.
    fn wait(
        &self,
        tasks: &[TaskId],
        deadline: Instant,
    ) -> Result<Vec<Option<TaskOutcome>>, Interrupted>;
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
    next_task: Cell<TaskId>,
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

    fn start_task(&self) -> TaskId {
        let task = self.next_task.get();
        self.next_task.set(task + 1);
        task
    }
}

#[cfg(test)]
impl SpecRuntime for InMemorySpecRuntime {
    fn start(&self, _call: SpecFunctionCall, _context: Option<&GeneratorContext>) -> TaskId {
        self.start_task()
    }

    fn start_exec(&self, _request: ExecRequest, _context: &GeneratorContext) -> TaskId {
        self.start_task()
    }

    fn start_sleep(&self, _ms: u64) -> TaskId {
        self.start_task()
    }

    fn wait(
        &self,
        tasks: &[TaskId],
        _deadline: Instant,
    ) -> Result<Vec<Option<TaskOutcome>>, Interrupted> {
        Ok(tasks.iter().map(|_| None).collect())
    }

    fn call(
        &self,
        call: SpecFunctionCall,
        _context: Option<&GeneratorContext>,
        _timeout: Duration,
    ) -> Result<SpecFunctionResult, SpecError> {
        self.calls.borrow_mut().push(call.clone());
        match self.answers.get(&call.function()) {
            Some(answer) => answer(&call),
            None => Err(SpecError::Invalid("unknown fn".into())),
        }
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

    #[test]
    fn should_reject_an_unknown_function() {
        let runtime = InMemorySpecRuntime::default();
        let call = SpecFunctionCall::Alias {
            function: 3,
            token: "co".into(),
        };

        let result = runtime.call(call, None, Duration::from_secs(5));

        assert!(matches!(result, Err(SpecError::Invalid(text)) if text == "unknown fn"));
    }

    #[test]
    fn should_give_none_for_a_running_task() {
        let runtime = InMemorySpecRuntime::default();
        let script = SpecFunctionCall::Script {
            function: 0,
            tokens: vec!["git".into()],
        };
        let tasks = [
            runtime.start(script, None),
            runtime.start_exec(ExecRequest::default(), &GeneratorContext::default()),
            runtime.start_sleep(100),
        ];

        let outcomes = runtime.wait(&tasks, Instant::now());

        assert_eq!(outcomes, Ok(vec![None, None, None]));
    }
}
