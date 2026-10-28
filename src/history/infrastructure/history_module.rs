use crate::history::application::record_command_use_case::RecordCommandUseCase;
use crate::history::domain::repositories::history_repository::{
    HistoryRepository, InMemoryHistoryRepository,
};
use crate::history::infrastructure::adapters::jsonl_history_repository::JsonlHistoryRepository;
use crate::shared::domain::value_objects::data_directory::DataDirectory;
use crate::shared::infrastructure::logger;
use crate::shared::infrastructure::shared_dependencies::SharedDependencies;
use std::rc::Rc;

/// The history use cases, and the history they record into: the one other modules read (ghost text, history
/// rows, LLM context). One history per process, so what is recorded is what is read.
pub(crate) struct HistoryModule {
    pub(crate) history: Rc<dyn HistoryRepository>,
    pub(crate) record_command: RecordCommandUseCase,
}

impl HistoryModule {
    pub(crate) fn new(history: Rc<dyn HistoryRepository>) -> Self {
        HistoryModule {
            record_command: RecordCommandUseCase::new(Rc::clone(&history)),
            history,
        }
    }
}

/// Loads the history file only when the module is built (the engine thread), so an unknown home never fails earlier.
pub(crate) fn build_history_module(dependencies: &SharedDependencies) -> HistoryModule {
    HistoryModule::new(history_repository(&dependencies.data_directory))
}

/// The history file under the data directory, or a memory-only history when there is no data directory.
fn history_repository(data_directory: &DataDirectory) -> Rc<dyn HistoryRepository> {
    match data_directory.history_file() {
        Ok(path) => Rc::new(JsonlHistoryRepository::load(path)),
        Err(e) => {
            logger::warn(&format!("history: {e:#}"));
            Rc::new(InMemoryHistoryRepository::new(Vec::new()))
        }
    }
}
