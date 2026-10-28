use crate::history::domain::entities::history_entry::HistoryEntry;
use std::cell::RefCell;
use std::rc::Rc;

/// How many entries a history keeps in memory: what ghost text, history rows and the LLM can see.
pub(crate) const HISTORY_IN_MEMORY: usize = 5000;

/// The commands this terminal has seen run (SPEC §3.5).
pub(crate) trait HistoryRepository {
    /// Records a finished command; a blank one is not recorded.
    fn append(&self, entry: HistoryEntry);
    /// What this history held when it was loaded plus what this process appended since, oldest to newest (an
    /// append keeps the newest `HISTORY_IN_MEMORY`). Shared rather than copied: ghost text reads it per keystroke.
    fn recent(&self) -> Rc<Vec<HistoryEntry>>;
}

/// A history kept in memory only: the history when there is no data directory, and the memory of the JSONL one.
pub(crate) struct InMemoryHistoryRepository {
    entries: RefCell<Rc<Vec<HistoryEntry>>>,
}

impl InMemoryHistoryRepository {
    /// Keeps the newest `HISTORY_IN_MEMORY` of `entries`.
    pub(crate) fn new(mut entries: Vec<HistoryEntry>) -> Self {
        keep_newest(&mut entries);
        InMemoryHistoryRepository {
            entries: RefCell::new(Rc::new(entries)),
        }
    }
}

fn keep_newest(entries: &mut Vec<HistoryEntry>) {
    entries.drain(..entries.len().saturating_sub(HISTORY_IN_MEMORY));
}

impl HistoryRepository for InMemoryHistoryRepository {
    fn append(&self, entry: HistoryEntry) {
        if entry.command().trim().is_empty() {
            return;
        }
        let mut shared = self.entries.borrow_mut();
        let entries = Rc::make_mut(&mut shared);
        entries.push(entry);
        keep_newest(entries);
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

    #[test]
    fn should_keep_only_the_newest_5000_entries() {
        let full = (0..5000).map(|i| entry(&format!("c{i}"))).collect();
        let repository = InMemoryHistoryRepository::new(full);

        repository.append(entry("newest"));

        let recent = repository.recent();
        assert_eq!(
            (recent.len(), recent[0].command(), recent[4999].command()),
            (HISTORY_IN_MEMORY, "c1", "newest")
        );
    }

    #[test]
    fn should_keep_only_the_newest_5000_entries_it_was_built_with() {
        let more_than_full = (0..5001).map(|i| entry(&format!("c{i}"))).collect();

        let repository = InMemoryHistoryRepository::new(more_than_full);

        let recent = repository.recent();
        assert_eq!(
            (recent.len(), recent[0].command(), recent[4999].command()),
            (HISTORY_IN_MEMORY, "c1", "c5000")
        );
    }
}
