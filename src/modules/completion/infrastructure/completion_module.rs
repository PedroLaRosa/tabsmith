use crate::modules::completion::application::apply_completion_settings_use_case::ApplyCompletionSettingsUseCase;
use crate::modules::completion::application::apply_session_context_use_case::ApplySessionContextUseCase;
use crate::modules::completion::application::command_line_parser::CommandLineParser;
use crate::modules::completion::application::complete_command_line_use_case::CompleteCommandLineUseCase;
use crate::modules::completion::application::generator_runner::GeneratorRunner;
use crate::modules::completion::application::progress_generators_use_case::ProgressGeneratorsUseCase;
use crate::modules::completion::application::record_suggestion_insertion_use_case::RecordSuggestionInsertionUseCase;
use crate::modules::completion::application::reset_generators_use_case::ResetGeneratorsUseCase;
use crate::modules::completion::application::toggle_fuzzy_search_use_case::ToggleFuzzySearchUseCase;
use crate::modules::completion::domain::repositories::recency_repository::RecencyRepository;
use crate::modules::completion::infrastructure::adapters::fs_directory_lister::FsDirectoryLister;
use crate::modules::completion::infrastructure::adapters::path_executable_catalog::PathExecutableCatalog;
use crate::modules::completion::infrastructure::adapters::state_file_recency_repository::StateFileRecencyRepository;
use crate::modules::history::domain::repositories::history_repository::HistoryRepository;
use crate::modules::shared::infrastructure::shared_dependencies::SharedDependencies;
use crate::modules::specs::application::ports::spec_runtime::SpecRuntime;
use crate::modules::specs::domain::repositories::spec_repository::SpecRepository;
use crate::modules::specs::domain::value_objects::spec_index::SpecIndex;
use std::rc::Rc;

/// The completion use cases over the spec repository and runtime, the file system, the PATH and the recency kept in
/// `state.json`, and that recency repository: the engine loads it once and flushes it when it stops.
pub(crate) struct CompletionModule {
    pub(crate) complete_command_line: CompleteCommandLineUseCase,
    pub(crate) progress_generators: ProgressGeneratorsUseCase,
    pub(crate) reset_generators: ResetGeneratorsUseCase,
    pub(crate) record_suggestion_insertion: RecordSuggestionInsertionUseCase,
    pub(crate) apply_completion_settings: ApplyCompletionSettingsUseCase,
    pub(crate) apply_session_context: ApplySessionContextUseCase,
    pub(crate) toggle_fuzzy_search: ToggleFuzzySearchUseCase,
    pub(crate) recency: Rc<dyn RecencyRepository>,
}

/// Only builds: it reads and writes no file. One parser and one generator runner are shared by the use cases that
/// hold them. Everything it returns holds `Rc`s, so it is built on the thread that uses it (QuickJS is !Send).
pub(crate) fn build_completion_module(
    dependencies: &SharedDependencies,
    specs: Rc<dyn SpecRepository>,
    runtime: Rc<dyn SpecRuntime>,
    history: Rc<dyn HistoryRepository>,
    index: SpecIndex,
) -> CompletionModule {
    let parser = Rc::new(CommandLineParser::new(specs, runtime.clone()));
    let generators = Rc::new(GeneratorRunner::new(
        runtime.clone(),
        Rc::new(FsDirectoryLister),
        dependencies.clock.clone(),
    ));
    let recency: Rc<dyn RecencyRepository> =
        Rc::new(StateFileRecencyRepository::new(dependencies.data_directory.state_file().ok()));
    CompletionModule {
        complete_command_line: CompleteCommandLineUseCase::new(
            parser.clone(),
            runtime.clone(),
            generators.clone(),
            Rc::new(PathExecutableCatalog),
            history.clone(),
            dependencies.clock.clone(),
            index,
        ),
        progress_generators: ProgressGeneratorsUseCase::new(generators, history),
        reset_generators: ResetGeneratorsUseCase,
        record_suggestion_insertion: RecordSuggestionInsertionUseCase::new(recency.clone(), dependencies.clock.clone()),
        apply_completion_settings: ApplyCompletionSettingsUseCase::new(runtime, parser),
        apply_session_context: ApplySessionContextUseCase,
        toggle_fuzzy_search: ToggleFuzzySearchUseCase,
        recency,
    }
}
