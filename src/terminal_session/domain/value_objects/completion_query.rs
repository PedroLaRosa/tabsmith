/// What a refreshed edit buffer asks of the completion engine: the use case adds the working directory and the shell.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct CompletionQuery {
    request: u64,
    buffer: String,
    cursor: usize,
    history_mode: bool,
}

impl CompletionQuery {
    pub(crate) fn new(request: u64, buffer: String, cursor: usize, history_mode: bool) -> Self {
        CompletionQuery {
            request,
            buffer,
            cursor,
            history_mode,
        }
    }

    pub(crate) fn request(&self) -> u64 {
        self.request
    }

    pub(crate) fn buffer(&self) -> &str {
        &self.buffer
    }

    /// Byte offset into `buffer()`.
    pub(crate) fn cursor(&self) -> usize {
        self.cursor
    }

    pub(crate) fn history_mode(&self) -> bool {
        self.history_mode
    }
}
