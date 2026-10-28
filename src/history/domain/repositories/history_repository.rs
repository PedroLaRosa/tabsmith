use crate::history::domain::entities::history_entry::HistoryEntry;
use std::cell::RefCell;
use std::rc::Rc;

/// The commands this terminal has seen run (SPEC §3.5).
pub(crate) trait HistoryRepository {
    fn append(&self, entry: HistoryEntry);
    /// Oldest to newest.
    fn recent(&self) -> Rc<Vec<HistoryEntry>>;
}

/// A history kept in memory only.
pub(crate) struct InMemoryHistoryRepository {
    entries: RefCell<Rc<Vec<HistoryEntry>>>,
}

impl InMemoryHistoryRepository {
    pub(crate) fn new(entries: Vec<HistoryEntry>) -> Self {
        InMemoryHistoryRepository {
            entries: RefCell::new(Rc::new(entries)),
        }
    }
}

impl HistoryRepository for InMemoryHistoryRepository {
    fn append(&self, entry: HistoryEntry) {
        if entry.command().trim().is_empty() {
            return;
        }
        Rc::make_mut(&mut self.entries.borrow_mut()).push(entry);
    }

    fn recent(&self) -> Rc<Vec<HistoryEntry>> {
        Rc::clone(&self.entries.borrow())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::history::domain::entities::history_entry::HistoryEntryPrimitives;

    fn entry(command: &str) -> HistoryEntry {
        HistoryEntry::new(HistoryEntryPrimitives {
            command: command.into(),
            ..Default::default()
        })
    }

    #[test]
    fn should_give_the_entries_it_was_built_with_then_the_appended_ones_oldest_first() {
        let repository = InMemoryHistoryRepository::new(vec![entry("ls")]);

        repository.append(entry("git status"));

        assert_eq!(*repository.recent(), [entry("ls"), entry("git status")]);
    }

    #[test]
    fn should_not_record_a_blank_command() {
        let repository = InMemoryHistoryRepository::new(Vec::new());

        repository.append(entry("   "));

        assert!(repository.recent().is_empty());
    }
}
