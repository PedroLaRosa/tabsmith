use crate::shared::domain::value_objects::shell::Shell;
use std::path::PathBuf;

/// Wrapper → engine completion request.
#[derive(Clone, Debug)]
pub(crate) struct CompletionRequest {
    pub(crate) id: u64,
    pub(crate) buffer: String,
    /// Byte offset into `buffer`.
    pub(crate) cursor: usize,
    pub(crate) cwd: PathBuf,
    pub(crate) shell: Shell,
    pub(crate) history_mode: bool,
}
