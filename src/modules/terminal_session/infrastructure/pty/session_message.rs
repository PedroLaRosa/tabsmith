use crate::modules::completion::domain::value_objects::completion_result::CompletionResult;

/// Messages to the wrapper's main loop.
pub(crate) enum SessionMessage {
    Stdin(Vec<u8>),
    Pty(Vec<u8>),
    PtyEof,
    Resize(u16, u16),
    ChildExit(i32),
    #[cfg_attr(windows, allow(dead_code, reason = "only the unix signal thread sends it"))]
    Term,
    Completion(Box<CompletionResult>),
    Ghost { req: u64, text: Option<String> },
}
