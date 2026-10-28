use crate::history::domain::entities::history_entry::HistoryEntry;
use crate::history::domain::repositories::history_repository::HistoryRepository;
use std::rc::Rc;

/// A command finished in the terminal: it goes into the history.
pub(crate) struct RecordCommandUseCase {
    history: Rc<dyn HistoryRepository>,
}

impl RecordCommandUseCase {
    pub(crate) fn new(history: Rc<dyn HistoryRepository>) -> Self {
        RecordCommandUseCase { history }
    }

    pub(crate) fn execute(&self, entry: HistoryEntry) {
        self.history.append(entry);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::history::domain::entities::history_entry::HistoryEntryPrimitives;
    use crate::history::domain::repositories::history_repository::InMemoryHistoryRepository;

    fn entry(command: &str) -> HistoryEntry {
        HistoryEntry::new(HistoryEntryPrimitives {
            command: command.into(),
            ..Default::default()
        })
    }

    #[test]
    fn should_record_the_finished_command() {
        let history = Rc::new(InMemoryHistoryRepository::new(vec![entry("ls")]));
        let use_case = RecordCommandUseCase::new(history.clone());

        use_case.execute(entry("git status"));

        assert_eq!(*history.recent(), [entry("ls"), entry("git status")]);
    }
}
