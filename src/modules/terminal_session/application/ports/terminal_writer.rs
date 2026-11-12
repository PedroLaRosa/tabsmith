#[cfg(test)]
use crate::modules::terminal_session::application::test_fixtures::{Io, IoJournal};
#[cfg(test)]
use std::cell::RefCell;
#[cfg(test)]
use std::rc::Rc;

/// The user's terminal. An empty slice writes nothing; a failing terminal is ignored, never reported.
pub(crate) trait TerminalWriter {
    fn write(&self, bytes: &[u8]);
}

/// Test double: keeps what was written, and notes each non-empty write in the journal it shares with other doubles.
#[cfg(test)]
pub(crate) struct CapturingTerminalWriter {
    written: RefCell<Vec<u8>>,
    journal: Option<Rc<IoJournal>>,
}

#[cfg(test)]
impl CapturingTerminalWriter {
    pub(crate) fn new() -> Self {
        CapturingTerminalWriter { written: RefCell::new(Vec::new()), journal: None }
    }

    pub(crate) fn journaled(journal: Rc<IoJournal>) -> Self {
        CapturingTerminalWriter { written: RefCell::new(Vec::new()), journal: Some(journal) }
    }

    pub(crate) fn written(&self) -> Vec<u8> {
        self.written.borrow().clone()
    }

    pub(crate) fn clear(&self) {
        self.written.borrow_mut().clear();
    }
}

#[cfg(test)]
impl TerminalWriter for CapturingTerminalWriter {
    fn write(&self, bytes: &[u8]) {
        if bytes.is_empty() {
            return;
        }
        self.written.borrow_mut().extend_from_slice(bytes);
        if let Some(journal) = &self.journal {
            journal.note(Io::Stdout(bytes.to_vec()));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn should_capture_the_bytes_written_in_the_order_they_were_written() {
        let writer = CapturingTerminalWriter::new();

        writer.write(b"one ");
        writer.write(b"two");

        assert_eq!(writer.written(), b"one two");
    }

    #[test]
    fn should_forget_what_was_captured_once_cleared() {
        let writer = CapturingTerminalWriter::new();
        writer.write(b"old");

        writer.clear();
        writer.write(b"new");

        assert_eq!(writer.written(), b"new");
    }

    #[test]
    fn should_note_each_write_in_the_shared_journal() {
        let journal = Rc::new(IoJournal::new());
        let writer = CapturingTerminalWriter::journaled(journal.clone());

        writer.write(b"a");
        writer.write(b"");
        writer.write(b"b");

        assert_eq!(journal.events(), vec![Io::Stdout(b"a".to_vec()), Io::Stdout(b"b".to_vec())]);
    }
}
