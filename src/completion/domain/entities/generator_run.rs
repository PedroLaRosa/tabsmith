use crate::completion::domain::value_objects::generator_command::GeneratorCommand;
use crate::completion::domain::value_objects::generator_outcome::GeneratorOutcome;
use crate::completion::domain::value_objects::generator_step::GeneratorStep;
use crate::specs::domain::value_objects::arg::Arg;
use crate::specs::domain::value_objects::generator::Generator;
use crate::specs::domain::value_objects::generator_context::GeneratorContext;
use crate::specs::domain::value_objects::spec_function_call::SpecFunctionCall;
use crate::specs::domain::value_objects::spec_suggestion::SpecSuggestion;
use crate::specs::domain::value_objects::task_id::TaskId;
use crate::specs::domain::value_objects::template_suggestions::TemplateSuggestions;
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

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
}
