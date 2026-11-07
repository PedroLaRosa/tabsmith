use crate::modules::shared::domain::value_objects::shell::Shell;
use std::path::PathBuf;

/// What an LLM ghost text request asks about: the buffer the user typed, where and in which shell.
pub(crate) struct LlmGhostTextRequest {
    pub(crate) buffer: String,
    pub(crate) cwd: PathBuf,
    pub(crate) shell: Shell,
}
