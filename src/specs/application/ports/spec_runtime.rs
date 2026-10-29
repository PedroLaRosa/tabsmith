use crate::settings::domain::value_objects::settings::Settings;
use crate::specs::application::ports::interrupted::Interrupted;
use crate::specs::domain::spec_error::SpecError;
use crate::specs::domain::value_objects::exec_request::ExecRequest;
use crate::specs::domain::value_objects::generator_context::GeneratorContext;
use crate::specs::domain::value_objects::spec_function_call::SpecFunctionCall;
use crate::specs::domain::value_objects::spec_function_kind::SpecFunctionKind;
use crate::specs::domain::value_objects::spec_function_result::SpecFunctionResult;
use crate::specs::domain::value_objects::spec_value::SpecValue;
use crate::specs::domain::value_objects::task_id::TaskId;
use crate::specs::domain::value_objects::task_outcome::TaskOutcome;
use std::time::{Duration, Instant};
#[cfg(test)]
use crate::specs::domain::value_objects::spec_function_ref::SpecFunctionRef;
#[cfg(test)]
use crate::specs::infrastructure::adapters::fig_spec_mapper::{function_result, spec_value_to_json};
#[cfg(test)]
use serde_json::Value;
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
    /// Drops tasks the caller no longer waits for.
    fn forget(&self, tasks: &[TaskId]);
    /// Starts the call, waits for it until `timeout` and reads its result by `call.kind()`. It forgets the task when
    /// it times out or a newer request aborts it, not when the call failed.
    fn call(
        &self,
        call: SpecFunctionCall,
        context: Option<&GeneratorContext>,
        timeout: Duration,
    ) -> Result<SpecFunctionResult, SpecError>;
    /// Reads a value a task gave, or a kept one, the way a fresh result of `kind` is read: what `call` gives for it.
    fn read(
        &self,
        value: &SpecValue,
        kind: SpecFunctionKind,
        context: &GeneratorContext,
    ) -> SpecFunctionResult;
    /// The `stdout` text of an exec task's `{stdout, stderr, status}` value; empty when it has none.
    fn read_exec_stdout(&self, value: &SpecValue) -> String;
    /// Applies the settings the runtime reads (`autocomplete.scriptTimeout`).
    fn apply_settings(&self, settings: &Settings);
}

#[cfg(test)]
type Answer = Box<dyn Fn(&SpecFunctionCall) -> Result<SpecFunctionResult, SpecError>>;

/// Answers calls with the answers it was given, and records them, for tests. Its tasks never finish, and it
/// ignores settings.
// ponytail: tasks only run or are unknown; Phase 6.4 adds scripted task completion when the generator tests need it.
#[cfg(test)]
#[derive(Default)]
pub(crate) struct InMemorySpecRuntime {
    answers: HashMap<SpecFunctionRef, Answer>,
    calls: RefCell<Vec<SpecFunctionCall>>,
    completions: HashMap<SpecFunctionRef, TaskOutcome>,
    exec_completion: Option<TaskOutcome>,
    sleeps_finish: bool,
    /// The tasks it knows: `None` while one runs, the outcome a finished one gives.
    tasks: RefCell<HashMap<TaskId, Option<TaskOutcome>>>,
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

    /// Every task started for `function` finishes with `outcome`.
    pub(crate) fn completing(mut self, function: SpecFunctionRef, outcome: TaskOutcome) -> Self {
        self.completions.insert(function, outcome);
        self
    }

    /// Every process started finishes with `outcome`.
    pub(crate) fn completing_execs(mut self, outcome: TaskOutcome) -> Self {
        self.exec_completion = Some(outcome);
        self
    }

    /// Every sleep started finishes with null.
    pub(crate) fn finishing_sleeps(mut self) -> Self {
        self.sleeps_finish = true;
        self
    }

    /// The calls received so far, in order.
    pub(crate) fn calls(&self) -> Vec<SpecFunctionCall> {
        self.calls.borrow().clone()
    }

    fn start_task(&self, outcome: Option<TaskOutcome>) -> TaskId {
        let task = self.next_task.get();
        self.next_task.set(task + 1);
        self.tasks.borrow_mut().insert(task, outcome);
        task
    }
}

#[cfg(test)]
impl SpecRuntime for InMemorySpecRuntime {
    fn start(&self, call: SpecFunctionCall, _context: Option<&GeneratorContext>) -> TaskId {
        self.start_task(self.completions.get(&call.function()).cloned())
    }

    fn start_exec(&self, _request: ExecRequest, _context: &GeneratorContext) -> TaskId {
        self.start_task(self.exec_completion.clone())
    }

    fn start_sleep(&self, _ms: u64) -> TaskId {
        self.start_task(self.sleeps_finish.then_some(Ok(SpecValue::Null)))
    }

    fn wait(
        &self,
        tasks: &[TaskId],
        _deadline: Instant,
    ) -> Result<Vec<Option<TaskOutcome>>, Interrupted> {
        let mut known = self.tasks.borrow_mut();
        let outcome = |t: &TaskId| match known.get(t) {
            Some(None) => None,
            Some(Some(_)) => known.remove(t).flatten(),
            None => Some(Err(format!("unknown task {t}"))),
        };
        Ok(tasks.iter().map(outcome).collect())
    }

    fn forget(&self, tasks: &[TaskId]) {
        let mut known = self.tasks.borrow_mut();
        for t in tasks {
            known.remove(t);
        }
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

    fn read(
        &self,
        value: &SpecValue,
        kind: SpecFunctionKind,
        context: &GeneratorContext,
    ) -> SpecFunctionResult {
        function_result(kind, &spec_value_to_json(value), context)
    }

    fn read_exec_stdout(&self, value: &SpecValue) -> String {
        spec_value_to_json(value)
            .get("stdout")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string()
    }

    fn apply_settings(&self, _settings: &Settings) {}
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::specs::domain::value_objects::spec_suggestion::SpecSuggestion;

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

    #[test]
    fn should_give_unknown_for_a_forgotten_task() {
        let runtime = InMemorySpecRuntime::default();
        let running = runtime.start_sleep(100);
        let forgotten = runtime.start_sleep(100);
        runtime.forget(&[forgotten]);

        let outcomes = runtime.wait(&[running, forgotten], Instant::now());

        assert_eq!(
            outcomes,
            Ok(vec![None, Some(Err(format!("unknown task {forgotten}")))])
        );
    }

    #[test]
    fn should_decode_a_read_like_the_runtime() {
        let runtime = InMemorySpecRuntime::default();
        let lines = SpecValue::List(vec![SpecValue::Text("main".into()), SpecValue::Text("dev".into())]);
        let dangerous = GeneratorContext::new("/repo".into(), HashMap::new(), "zsh".into(), "".into(), true);
        let rows = vec![
            SpecSuggestion::from_script_output("main", true),
            SpecSuggestion::from_script_output("dev", true),
        ];

        let read = runtime.read(&lines, SpecFunctionKind::PostProcess, &dangerous);

        assert!(matches!(read, SpecFunctionResult::PostProcess(read_rows) if read_rows == rows));
    }

    #[test]
    fn should_finish_a_started_call_with_its_scripted_outcome_once() {
        let branches = SpecValue::List(vec![SpecValue::Text("main".into())]);
        let runtime = InMemorySpecRuntime::default().completing(0, Ok(branches.clone()));
        let script = SpecFunctionCall::Script {
            function: 0,
            tokens: vec!["git".into()],
        };
        let tasks = [runtime.start(script.clone(), None), runtime.start(script, None)];

        let first = runtime.wait(&tasks, Instant::now());
        let second = runtime.wait(&tasks, Instant::now());

        let unknown = |t: TaskId| Some(Err(format!("unknown task {t}")));
        assert_eq!(
            (first, second),
            (
                Ok(vec![Some(Ok(branches.clone())), Some(Ok(branches))]),
                Ok(vec![unknown(tasks[0]), unknown(tasks[1])])
            )
        );
    }

    #[test]
    fn should_finish_an_exec_with_its_scripted_outcome() {
        let stdout = SpecValue::Text("main\ndev".into());
        let output = SpecValue::Object([("stdout".to_string(), stdout)].into());
        let runtime = InMemorySpecRuntime::default().completing_execs(Ok(output.clone()));
        let tasks = [
            runtime.start_exec(ExecRequest::default(), &GeneratorContext::default()),
            runtime.start_exec(ExecRequest::default(), &GeneratorContext::default()),
        ];

        let outcomes = runtime.wait(&tasks, Instant::now());

        assert_eq!(outcomes, Ok(vec![Some(Ok(output.clone())), Some(Ok(output))]));
    }

    #[test]
    fn should_finish_a_sleep_with_null() {
        let runtime = InMemorySpecRuntime::default().finishing_sleeps();
        let debounce = runtime.start_sleep(100);

        let outcomes = runtime.wait(&[debounce], Instant::now());

        assert_eq!(outcomes, Ok(vec![Some(Ok(SpecValue::Null))]));
    }
}
