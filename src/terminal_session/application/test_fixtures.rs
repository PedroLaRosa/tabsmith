use std::cell::RefCell;

/// One thing the session sent out, in the order the doubles saw it.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Io {
    Stdout(Vec<u8>),
    ShellInput(Vec<u8>),
}

/// Test support: the order in which the user's terminal and the shell were written, across both doubles.
pub(crate) struct IoJournal {
    events: RefCell<Vec<Io>>,
}

impl IoJournal {
    pub(crate) fn new() -> Self {
        IoJournal { events: RefCell::new(Vec::new()) }
    }

    pub(crate) fn note(&self, event: Io) {
        self.events.borrow_mut().push(event);
    }

    pub(crate) fn events(&self) -> Vec<Io> {
        self.events.borrow().clone()
    }
}
