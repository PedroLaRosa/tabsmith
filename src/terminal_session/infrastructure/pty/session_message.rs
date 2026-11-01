use crate::completion::domain::value_objects::completion_result::CompletionResult;

/// Messages to the wrapper's main loop.
pub(crate) enum SessionMessage {
    Stdin(Vec<u8>),
    Pty(Vec<u8>),
    PtyEof,
    Resize(u16, u16),
    ChildExit(i32),
    Term,
    Completion(Box<CompletionResult>),
    Ghost { req: u64, text: Option<String> },
}
