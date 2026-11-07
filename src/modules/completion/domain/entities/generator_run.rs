use crate::modules::completion::domain::value_objects::generator_command::GeneratorCommand;
use crate::modules::completion::domain::value_objects::generator_outcome::GeneratorOutcome;
use crate::modules::completion::domain::value_objects::generator_step::GeneratorStep;
use crate::modules::specs::domain::value_objects::arg::Arg;
use crate::modules::specs::domain::value_objects::custom_generator_result::CustomGeneratorResult;
use crate::modules::specs::domain::value_objects::generator::Generator;
use crate::modules::specs::domain::value_objects::generator_context::GeneratorContext;
use crate::modules::specs::domain::value_objects::script_resolution::ScriptResolution;
use crate::modules::specs::domain::value_objects::spec_function_call::SpecFunctionCall;
use crate::modules::specs::domain::value_objects::spec_suggestion::SpecSuggestion;
use crate::modules::specs::domain::value_objects::spec_value::SpecValue;
use crate::modules::specs::domain::value_objects::task_id::TaskId;
use crate::modules::specs::domain::value_objects::template_suggestions::TemplateSuggestions;
use std::rc::Rc;
use std::time::Instant;

/// One generator run waiting on a `SpecRuntime` task.
#[derive(Clone)]
pub(crate) struct GeneratorRun {
    slot: usize,
    generation: u64,
    /// Stale-while-revalidate refresh: its failure leaves the slot alone.
    background: bool,
    arg: Rc<Arg>,
    index: usize,
    context: GeneratorContext,
    tokens: Vec<String>,
    help: Vec<SpecSuggestion>,
    step: GeneratorStep,
    task: TaskId,
    /// Cache key this run's task result is stored under (owner of an in-flight fetch).
    store: Option<String>,
    deadline: Instant,
}

impl GeneratorRun {
    /// A foreground run of generator `index` of `arg` (slot `index`) fired as `generation`, before any task.
    pub(crate) fn new(
        index: usize,
        generation: u64,
        arg: Rc<Arg>,
        context: GeneratorContext,
        tokens: Vec<String>,
        help: Vec<SpecSuggestion>,
        deadline: Instant,
    ) -> Self {
        GeneratorRun {
            slot: index,
            generation,
            background: false,
            arg,
            index,
            context,
            tokens,
            help,
            step: GeneratorStep::Debounce,
            task: 0,
            store: None,
            deadline,
        }
    }

    pub(crate) fn slot(&self) -> usize {
        self.slot
    }

    pub(crate) fn generation(&self) -> u64 {
        self.generation
    }

    pub(crate) fn is_background(&self) -> bool {
        self.background
    }

    pub(crate) fn arg(&self) -> &Rc<Arg> {
        &self.arg
    }

    pub(crate) fn index(&self) -> usize {
        self.index
    }

    pub(crate) fn context(&self) -> &GeneratorContext {
        &self.context
    }

    pub(crate) fn tokens(&self) -> &[String] {
        &self.tokens
    }

    pub(crate) fn help(&self) -> &[SpecSuggestion] {
        &self.help
    }

    pub(crate) fn step(&self) -> GeneratorStep {
        self.step
    }

    pub(crate) fn task(&self) -> TaskId {
        self.task
    }

    pub(crate) fn store_key(&self) -> Option<&str> {
        self.store.as_deref()
    }

    pub(crate) fn deadline(&self) -> Instant {
        self.deadline
    }

    /// The run now waits on `task`, which finishes `step`.
    pub(crate) fn wait_on(&mut self, task: TaskId, step: GeneratorStep) {
        self.task = task;
        self.step = step;
    }

    /// The run owns the fetch of `key`: its task's value is stored under it.
    pub(crate) fn store_result_as(&mut self, key: String) {
        self.store = Some(key);
    }

    pub(crate) fn take_store(&mut self) -> Option<String> {
        self.store.take()
    }

    /// A copy that refreshes a stale cached value in the background: its failure leaves the slot alone.
    pub(crate) fn background_copy(&self) -> GeneratorRun {
        let mut bg = self.clone();
        bg.background = true;
        bg
    }

    /// A template generator's rows, `listed` then the help rows: through its `filterTemplateSuggestions`, or settled.
    pub(crate) fn template_command(&self, generator: &Generator, listed: Vec<SpecSuggestion>) -> GeneratorCommand {
        let mut items = listed;
        items.extend(self.help().iter().cloned()); // `history` template: [] (F6)
        match generator.filter_template_suggestions() {
            Some(f) => GeneratorCommand::StartCall(
                SpecFunctionCall::FilterTemplateSuggestions {
                    function: f,
                    suggestions: TemplateSuggestions::Listed(items),
                },
                self.context().clone(),
            ),
            None => GeneratorCommand::Settle(GeneratorOutcome::Done(items)),
        }
    }

    /// An exec's `stdout`: split on `splitOn`, which takes precedence over `postProcess`; neither → [].
    pub(crate) fn exec_command(&self, generator: &Generator, stdout: &str) -> GeneratorCommand {
        if let Some(sep) = generator.split_on() {
            let t = stdout.trim();
            if t.is_empty() {
                return GeneratorCommand::Settle(GeneratorOutcome::Done(Vec::new()));
            }
            return GeneratorCommand::Settle(GeneratorOutcome::Done(
                t.split(sep.as_str())
                    .map(|s| SpecSuggestion::from_script_output(s, self.context().is_dangerous()))
                    .collect(),
            ));
        }
        let Some(pp) = generator.post_process() else {
            return GeneratorCommand::Settle(GeneratorOutcome::Done(Vec::new()));
        };
        GeneratorCommand::StartCall(
            SpecFunctionCall::PostProcess {
                function: pp,
                stdout: stdout.to_string(),
                tokens: self.tokens().to_vec(),
            },
            self.context().clone(),
        )
    }

    /// A custom generator's read `result`: a templated one goes to `filterTemplateSuggestions` as its raw `value`.
    pub(crate) fn custom_command(
        &self,
        generator: &Generator,
        value: SpecValue,
        result: CustomGeneratorResult,
    ) -> GeneratorCommand {
        match generator.filter_template_suggestions() {
            Some(f) if result.is_templated() => GeneratorCommand::StartCall(
                SpecFunctionCall::FilterTemplateSuggestions {
                    function: f,
                    suggestions: TemplateSuggestions::Returned(value),
                },
                self.context().clone(),
            ),
            _ => GeneratorCommand::Settle(GeneratorOutcome::Done(result.suggestions().clone())), // undefined → []
        }
    }

    /// A script's `resolution`: the exec to start, its timeout raised to the largest of `floor` (the settings' script
    /// timeout), the generator's `scriptTimeout` and the request's own; nothing to run → []; bad input → a failure.
    pub(crate) fn script_command(
        &self,
        generator: &Generator,
        resolution: ScriptResolution,
        floor: u64,
    ) -> GeneratorCommand {
        let request = match resolution {
            ScriptResolution::Ready { request, .. } => request,
            ScriptResolution::Invalid(e) => {
                return GeneratorCommand::Settle(GeneratorOutcome::Fail(format!("bad script input: {e}")))
            }
            ScriptResolution::Skip => return GeneratorCommand::Settle(GeneratorOutcome::Done(Vec::new())), // falsy → []
        };
        let timeout = floor
            .max(generator.script_timeout().unwrap_or(0))
            .max(request.timeout().unwrap_or(0));
        GeneratorCommand::StartExec(request.with_timeout(Some(timeout)), self.context().clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::specs::domain::value_objects::exec_request::ExecRequest;
    use std::collections::{BTreeMap, HashMap};

    fn run_of(generator: &Generator, help: Vec<SpecSuggestion>) -> GeneratorRun {
        let arg = Rc::new(Arg::default().with_generators(vec![generator.clone()]));
        let context = GeneratorContext::new("/repo".into(), HashMap::new(), "zsh".into(), String::new(), false);
        let tokens = vec!["git".to_string(), "checkout".to_string()];
        GeneratorRun::new(0, 1, arg, context, tokens, help, Instant::now())
    }

    fn row(name: &str) -> SpecSuggestion {
        SpecSuggestion::default().with_names(vec![name.into()])
    }

    #[test]
    fn should_list_templates_then_help_rows() {
        let generator = Generator::default().with_template(vec!["filepaths".into()]);
        let run = run_of(&generator, vec![row("h")]);

        let command = run.template_command(&generator, vec![row("a"), row("b")]);

        let GeneratorCommand::Settle(GeneratorOutcome::Done(rows)) = command else {
            panic!("expected the rows to settle, got {command:?}");
        };
        assert_eq!(rows, vec![row("a"), row("b"), row("h")]);
    }

    #[test]
    fn should_filter_template_rows_through_filter_template_suggestions() {
        let generator = Generator::default()
            .with_template(vec!["filepaths".into()])
            .with_filter_template_suggestions(Some(3));
        let run = run_of(&generator, vec![row("h")]);

        let command = run.template_command(&generator, vec![row("a"), row("b")]);

        let GeneratorCommand::StartCall(call, context) = command else {
            panic!("expected a filter call, got {command:?}");
        };
        let filter = SpecFunctionCall::FilterTemplateSuggestions {
            function: 3,
            suggestions: TemplateSuggestions::Listed(vec![row("a"), row("b"), row("h")]),
        };
        assert_eq!((call, context.cwd()), (filter, "/repo"));
    }

    #[test]
    fn should_split_trimmed_stdout_into_untrimmed_rows() {
        let generator = Generator::default().with_split_on(Some("\n".into()));
        let run = run_of(&generator, Vec::new());

        let command = run.exec_command(&generator, " a \n\n b \n");

        let GeneratorCommand::Settle(GeneratorOutcome::Done(rows)) = command else {
            panic!("expected the split rows to settle, got {command:?}");
        };
        let script_rows = ["a ", "", " b"].map(|text| SpecSuggestion::from_script_output(text, false));
        assert_eq!(rows, script_rows);
    }

    #[test]
    fn should_post_process_stdout_when_no_split_is_set() {
        let generator = Generator::default().with_post_process(Some(5));
        let run = run_of(&generator, Vec::new());

        let command = run.exec_command(&generator, "x\n");

        let GeneratorCommand::StartCall(call, _) = command else {
            panic!("expected a post-process call, got {command:?}");
        };
        let post_process = SpecFunctionCall::PostProcess {
            function: 5,
            stdout: "x\n".into(),
            tokens: vec!["git".into(), "checkout".into()],
        };
        assert_eq!(call, post_process);
    }

    #[test]
    fn should_settle_with_no_rows_without_split_or_post_process() {
        let generator = Generator::default();
        let run = run_of(&generator, Vec::new());

        let command = run.exec_command(&generator, "main\n");

        let GeneratorCommand::Settle(GeneratorOutcome::Done(rows)) = command else {
            panic!("expected no rows to settle, got {command:?}");
        };
        assert_eq!(rows, Vec::new());
    }

    fn custom_value(name: &str) -> SpecValue {
        let item = BTreeMap::from([("name".to_string(), SpecValue::Text(name.into()))]);
        SpecValue::List(vec![SpecValue::Object(item)])
    }

    #[test]
    fn should_filter_a_templated_custom_result_with_its_raw_value() {
        let generator = Generator::default().with_custom(Some(1)).with_filter_template_suggestions(Some(3));
        let run = run_of(&generator, Vec::new());
        let templated = CustomGeneratorResult::new(true, vec![row("main")]);

        let command = run.custom_command(&generator, custom_value("main"), templated);

        let GeneratorCommand::StartCall(call, _) = command else {
            panic!("expected a filter call, got {command:?}");
        };
        let filter = SpecFunctionCall::FilterTemplateSuggestions {
            function: 3,
            suggestions: TemplateSuggestions::Returned(custom_value("main")),
        };
        assert_eq!(call, filter);
    }

    #[test]
    fn should_settle_an_untemplated_custom_result_with_its_rows() {
        let generator = Generator::default().with_custom(Some(1)).with_filter_template_suggestions(Some(3));
        let run = run_of(&generator, Vec::new());
        let untemplated = CustomGeneratorResult::new(false, vec![row("main")]);

        let command = run.custom_command(&generator, custom_value("main"), untemplated);

        let GeneratorCommand::Settle(GeneratorOutcome::Done(rows)) = command else {
            panic!("expected the rows to settle, got {command:?}");
        };
        assert_eq!(rows, vec![row("main")]);
    }

    fn git_branch(timeout: Option<u64>) -> ExecRequest {
        ExecRequest::new("git".into(), vec!["branch".into()], Some("/repo".into()), HashMap::new(), timeout)
    }

    #[test]
    fn should_fail_bad_script_input_with_its_serde_text() {
        let generator = Generator::default();
        let run = run_of(&generator, Vec::new());
        let invalid = ScriptResolution::Invalid("missing field `command`".into());

        let command = run.script_command(&generator, invalid, 5000);

        let GeneratorCommand::Settle(GeneratorOutcome::Fail(reason)) = command else {
            panic!("expected the run to fail, got {command:?}");
        };
        assert_eq!(reason, "bad script input: missing field `command`");
    }

    #[test]
    fn should_raise_the_exec_timeout_to_the_largest_of_floor_generator_and_request() {
        // (floor, generator scriptTimeout, request timeout): each is the largest once; a missing one counts as 0
        let cases = [(9000, Some(5000), Some(7000)), (5000, Some(9000), None), (5000, None, Some(9000))];

        for (floor, generator_timeout, request_timeout) in cases {
            let generator = Generator::default().with_script_timeout(generator_timeout);
            let run = run_of(&generator, Vec::new());
            let ready = ScriptResolution::Ready {
                request: git_branch(request_timeout),
                cache_key_source: "git branch".into(),
            };

            let command = run.script_command(&generator, ready, floor);

            let GeneratorCommand::StartExec(request, _) = command else {
                panic!("expected an exec, got {command:?}");
            };
            assert_eq!(request, git_branch(request_timeout).with_timeout(Some(9000)));
        }
    }
}
