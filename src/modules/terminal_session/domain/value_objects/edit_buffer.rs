/// Edit buffer and cursor read from the model (SPEC §5.6).
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct EditBuffer {
    buffer: String,
    cursor: usize,
    shell_ghost: bool,
    at_prompt: bool,
}

impl EditBuffer {
    pub(crate) fn new(buffer: String, cursor: usize, shell_ghost: bool, at_prompt: bool) -> Self {
        EditBuffer {
            buffer,
            cursor,
            shell_ghost,
            at_prompt,
        }
    }

    pub(crate) fn buffer(&self) -> &str {
        &self.buffer
    }

    /// Byte offset into `buffer()`.
    pub(crate) fn cursor(&self) -> usize {
        self.cursor
    }

    /// The shell already draws its own suggestion after the cursor (SPEC §8.1).
    pub(crate) fn shell_ghost(&self) -> bool {
        self.shell_ghost
    }

    /// The cursor is on the command line: false on the rows an inline TUI draws below it.
    pub(crate) fn at_prompt(&self) -> bool {
        self.at_prompt
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn should_expose_what_it_was_built_with() {
        let edit_buffer = EditBuffer::new("ls -l".to_string(), 2, true, false);

        assert_eq!(
            (
                edit_buffer.buffer(),
                edit_buffer.cursor(),
                edit_buffer.shell_ghost(),
                edit_buffer.at_prompt()
            ),
            ("ls -l", 2, true, false)
        );
    }
}
