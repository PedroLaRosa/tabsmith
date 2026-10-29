use crate::completion::application::completion_notice::CompletionNotice;
use crate::completion::application::generator_runner::GeneratorRunner;
use crate::completion::domain::entities::completion_session::CompletionSession;
use crate::completion::domain::services::completion_result_builder::build_completion_result;
use crate::completion::domain::value_objects::completion_phase::CompletionPhase;
use crate::completion::domain::value_objects::completion_result::CompletionResult;
use crate::history::domain::repositories::history_repository::HistoryRepository;
use std::rc::Rc;

/// Waits one slice on the running generators and replies again (phase `Generators`) when a slot of the current
/// argument changed while a request is served. The reply carries no ghost text (Q15); a newer request interrupting the
/// wait leaves no reply, and the runs, the cache and the tasks as they were.
pub(crate) struct ProgressGeneratorsUseCase {
    generators: Rc<GeneratorRunner>,
    history: Rc<dyn HistoryRepository>,
}

impl ProgressGeneratorsUseCase {
    pub(crate) fn new(generators: Rc<GeneratorRunner>, history: Rc<dyn HistoryRepository>) -> Self {
        ProgressGeneratorsUseCase { generators, history }
    }

    pub(crate) fn execute(
        &self,
        session: &mut CompletionSession,
        report: &mut dyn FnMut(CompletionNotice),
    ) -> Option<CompletionResult> {
        if self.generators.progress(session, report) == Some(true) && session.served().is_some() {
            return Some(build_completion_result(session, CompletionPhase::Generators, &self.history.recent()));
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::completion::application::test_fixtures::{request, CompletionDoubles};
    use crate::settings::domain::value_objects::settings::Settings;
    use crate::specs::application::ports::spec_runtime::InMemorySpecRuntime;
    use crate::specs::domain::entities::subcommand::Subcommand;
    use crate::specs::domain::repositories::spec_repository::InMemorySpecRepository;
    use crate::specs::domain::value_objects::arg::Arg;
    use crate::specs::domain::value_objects::generator::Generator;
    use crate::specs::domain::value_objects::spec_function_ref::SpecFunctionRef;
    use crate::specs::domain::value_objects::spec_index::SpecIndex;
    use crate::specs::domain::value_objects::spec_value::SpecValue;
    use std::rc::Rc;

    /// The custom function of the first generator.
    const F: SpecFunctionRef = 1;

    /// The spec of a command `tool` whose first argument is `argument`.
    fn tool(argument: Arg) -> InMemorySpecRepository {
        let tool = Rc::new(Subcommand::default().with_names(vec!["tool".into()]).with_args(vec![Rc::new(argument)]));
        InMemorySpecRepository::with(&[("tool", &tool)])
    }

    fn custom(function: SpecFunctionRef) -> Generator {
        Generator::default().with_custom(Some(function))
    }

    fn texts(values: &[&str]) -> SpecValue {
        SpecValue::List(values.iter().map(|value| SpecValue::Text(value.to_string())).collect())
    }

    /// The names of the rows, sorted: ranking is not what these tests pin.
    fn names(result: &CompletionResult) -> Vec<String> {
        let mut names: Vec<String> = result.suggestions().iter().map(|row| row.first_name().to_string()).collect();
        names.sort();
        names
    }

    fn owned(names: &[&str]) -> Vec<String> {
        names.iter().map(|name| name.to_string()).collect()
    }

    #[test]
    fn should_reply_again_with_the_rows_of_a_finished_generator() {
        let doubles = CompletionDoubles::new(
            tool(Arg::default().with_generators(vec![custom(F)])),
            InMemorySpecRuntime::default().completing(F, Ok(texts(&["main", "dev"]))),
        );
        let mut session = doubles.session(Settings::default());
        doubles
            .complete_command_line(SpecIndex::default())
            .execute(&mut session, &request(1, "tool "), &mut |_| {});

        let result = doubles.progress_generators().execute(&mut session, &mut |_| {}).unwrap();

        assert_eq!(
            (result.phase(), names(&result), result.is_loading()),
            (CompletionPhase::Generators, owned(&["dev", "main"]), false)
        );
    }
}
