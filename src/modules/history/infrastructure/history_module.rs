use crate::modules::history::application::record_command_use_case::RecordCommandUseCase;
use crate::modules::history::domain::repositories::history_repository::{
    HistoryRepository, InMemoryHistoryRepository,
};
use crate::modules::history::infrastructure::adapters::jsonl_history_repository::JsonlHistoryRepository;
use crate::modules::shared::domain::value_objects::data_directory::DataDirectory;
use crate::modules::shared::infrastructure::logger;
use crate::modules::shared::infrastructure::shared_dependencies::SharedDependencies;
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
    HistoryModule::new(load_history_repository(&dependencies.data_directory))
}

/// The history file under the data directory, or a memory-only history when there is no data directory.
fn load_history_repository(data_directory: &DataDirectory) -> Rc<dyn HistoryRepository> {
    match data_directory.history_file() {
        Ok(path) => Rc::new(JsonlHistoryRepository::load(path)),
        Err(e) => {
            logger::warn(&format!("history: {e:#}"));
            Rc::new(InMemoryHistoryRepository::new(Vec::new()))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::history::domain::entities::history_entry::{HistoryEntry, HistoryEntryPrimitives};

    fn entry(command: &str) -> HistoryEntry {
        HistoryEntry::new(HistoryEntryPrimitives {
            command: command.into(),
            ..Default::default()
        })
    }

    #[test]
    fn should_read_back_the_command_it_records() {
        let module = HistoryModule::new(Rc::new(InMemoryHistoryRepository::new(Vec::new())));

        module.record_command.execute(entry("git status"));

        assert_eq!(*module.history.recent(), [entry("git status")]);
    }
}
