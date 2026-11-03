use crate::ghost_text::application::llm_ghost_text_job::LlmGhostTextJob;

/// Either the answer is known at once, or the provider has to be asked by a job the caller runs off its thread.
pub(crate) enum LlmGhostTextAnswer {
    Immediate(Option<String>),
    Deferred(Box<LlmGhostTextJob>),
}
