/// The system and user text sent to the LLM provider.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct LlmPrompt {
    system: String,
    user: String,
}

impl LlmPrompt {
    pub(crate) fn new(system: String, user: String) -> Self {
        Self { system, user }
    }

    pub(crate) fn system(&self) -> &str {
        &self.system
    }

    pub(crate) fn user(&self) -> &str {
        &self.user
    }
}
