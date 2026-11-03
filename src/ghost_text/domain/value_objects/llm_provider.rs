#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum LlmProvider {
    OpenAi,
    Anthropic,
}

impl LlmProvider {
    pub(crate) fn name(self) -> &'static str {
        match self {
            LlmProvider::OpenAi => "openai",
            LlmProvider::Anthropic => "anthropic",
        }
    }
}
