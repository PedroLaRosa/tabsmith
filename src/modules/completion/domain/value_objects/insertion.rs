/// Bytes-level description of an insertion (SPEC §6.8, §7.3). Counts are in chars.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct Insertion {
    /// Chars to delete before the cursor (sent as 0x7f each).
    delete: usize,
    /// Text to type (no trailing "\n"; `{cursor}` already removed).
    text: String,
    /// Chars to move the cursor left after typing (`ESC [ D` each).
    cursor_back: usize,
    /// Send "\r" afterwards.
    execute: bool,
}

/// What an `Insertion` is built from: named fields, so two counts cannot be swapped.
pub(crate) struct InsertionParts {
    pub(crate) delete: usize,
    pub(crate) text: String,
    pub(crate) cursor_back: usize,
    pub(crate) execute: bool,
}

impl Insertion {
    pub(crate) fn new(parts: InsertionParts) -> Self {
        let InsertionParts {
            delete,
            text,
            cursor_back,
            execute,
        } = parts;
        Insertion {
            delete,
            text,
            cursor_back,
            execute,
        }
    }

    pub(crate) fn delete(&self) -> usize {
        self.delete
    }

    pub(crate) fn text(&self) -> &str {
        &self.text
    }

    pub(crate) fn cursor_back(&self) -> usize {
        self.cursor_back
    }

    pub(crate) fn executes(&self) -> bool {
        self.execute
    }

    /// The same insertion, followed by "\r".
    pub(crate) fn with_execute(mut self) -> Self {
        self.execute = true;
        self
    }

    #[cfg(test)]
    pub(crate) fn with_cursor_back(mut self, cursor_back: usize) -> Self {
        self.cursor_back = cursor_back;
        self
    }
}
