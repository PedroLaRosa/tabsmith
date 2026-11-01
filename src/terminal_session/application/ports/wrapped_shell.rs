#[cfg(test)]
use crate::terminal_session::application::test_fixtures::{Io, IoJournal};
#[cfg(test)]
use std::cell::RefCell;
#[cfg(test)]
use std::rc::Rc;

/// The shell the session wraps. A gone shell is ignored, never reported.
pub(crate) trait WrappedShell {
    fn write_input(&self, bytes: Vec<u8>);
    /// Columns come first, as in a terminal size report.
    fn resize(&self, columns: u16, rows: u16);
}

/// Test double: keeps the input and the resizes, and notes each input in the journal it shares with other doubles.
#[cfg(test)]
pub(crate) struct CapturingWrappedShell {
    writes: RefCell<Vec<Vec<u8>>>,
    resizes: RefCell<Vec<(u16, u16)>>,
    journal: Option<Rc<IoJournal>>,
}

#[cfg(test)]
impl CapturingWrappedShell {
    pub(crate) fn new() -> Self {
        CapturingWrappedShell { writes: RefCell::new(Vec::new()), resizes: RefCell::new(Vec::new()), journal: None }
    }

    pub(crate) fn journaled(journal: Rc<IoJournal>) -> Self {
        CapturingWrappedShell { journal: Some(journal), ..CapturingWrappedShell::new() }
    }

    /// Everything written since the last call, as one byte string.
    pub(crate) fn take_input(&self) -> Vec<u8> {
        self.take_writes().concat()
    }

    /// Everything written since the last call, one entry per `write_input`.
    pub(crate) fn take_writes(&self) -> Vec<Vec<u8>> {
        self.writes.take()
    }

    /// Every resize as `(columns, rows)`.
    pub(crate) fn resizes(&self) -> Vec<(u16, u16)> {
        self.resizes.borrow().clone()
    }
}

#[cfg(test)]
impl WrappedShell for CapturingWrappedShell {
    fn write_input(&self, bytes: Vec<u8>) {
        if let Some(journal) = &self.journal {
            journal.note(Io::ShellInput(bytes.clone()));
        }
        self.writes.borrow_mut().push(bytes);
    }

    fn resize(&self, columns: u16, rows: u16) {
        self.resizes.borrow_mut().push((columns, rows));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::terminal_session::application::ports::terminal_writer::{CapturingTerminalWriter, TerminalWriter};

    #[test]
    fn should_hand_the_input_over_in_the_order_it_was_written_and_only_once() {
        let shell = CapturingWrappedShell::new();
        shell.write_input(b"ab".to_vec());
        shell.write_input(b"c".to_vec());

        let handed_over = (shell.take_input(), shell.take_input());

        assert_eq!(handed_over, (b"abc".to_vec(), Vec::new()));
    }

    #[test]
    fn should_record_a_resize_as_columns_then_rows() {
        let shell = CapturingWrappedShell::new();

        shell.resize(120, 40);

        assert_eq!(shell.resizes(), vec![(120, 40)]);
    }

    #[test]
    fn should_interleave_stdout_and_shell_input_in_one_journal() {
        let journal = Rc::new(IoJournal::new());
        let stdout = CapturingTerminalWriter::journaled(journal.clone());
        let shell = CapturingWrappedShell::journaled(journal.clone());

        stdout.write(b"echo");
        shell.write_input(b"key".to_vec());
        stdout.write(b"frame");

        assert_eq!(
            journal.events(),
            vec![Io::Stdout(b"echo".to_vec()), Io::ShellInput(b"key".to_vec()), Io::Stdout(b"frame".to_vec())]
        );
    }
}
