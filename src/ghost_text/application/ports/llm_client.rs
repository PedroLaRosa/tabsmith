use crate::ghost_text::domain::value_objects::llm_profile::LlmProfile;
use crate::ghost_text::domain::value_objects::llm_prompt::LlmPrompt;
use std::collections::HashMap;
#[cfg(test)]
use std::sync::Mutex;

/// Sends the prompt to the profile's provider. `None` when there is no reply worth cleaning (the adapter has
/// logged any transport failure).
pub(crate) trait LlmClient {
    fn complete(
        &self,
        profile: &LlmProfile,
        prompt: &LlmPrompt,
        env: &HashMap<String, String>,
    ) -> Option<String>;
}

/// Answers every prompt with the same reply and remembers the prompts it was sent. It holds a `Mutex`, so it is
/// `Send + Sync` like the real client.
#[cfg(test)]
pub(crate) struct ScriptedLlmClient {
    reply: Option<String>,
    prompts: Mutex<Vec<LlmPrompt>>,
}

#[cfg(test)]
impl ScriptedLlmClient {
    pub(crate) fn answering(reply: Option<&str>) -> Self {
        Self {
            reply: reply.map(String::from),
            prompts: Mutex::new(Vec::new()),
        }
    }

    pub(crate) fn prompts(&self) -> Vec<LlmPrompt> {
        self.prompts.lock().unwrap().clone()
    }
}

#[cfg(test)]
impl LlmClient for ScriptedLlmClient {
    fn complete(
        &self,
        _profile: &LlmProfile,
        prompt: &LlmPrompt,
        _env: &HashMap<String, String>,
    ) -> Option<String> {
        self.prompts.lock().unwrap().push(prompt.clone());
        self.reply.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ghost_text::domain::services::llm_prompt_builder::SYSTEM;
    use crate::ghost_text::domain::value_objects::llm_profile::LlmProfileParts;
    use crate::ghost_text::domain::value_objects::llm_provider::LlmProvider;

    fn profile() -> LlmProfile {
        LlmProfile::new(LlmProfileParts {
            name: "p".into(),
            provider: LlmProvider::OpenAi,
            model: "m".into(),
            base_url: "http://x/".into(),
            api_key_env: "K".into(),
            max_tokens: 7,
            effort: None,
        })
    }

    fn prompt(user: &str) -> LlmPrompt {
        LlmPrompt::new(SYSTEM.into(), user.into())
    }

    #[test]
    fn should_answer_every_prompt_with_its_scripted_reply() {
        let client = ScriptedLlmClient::answering(Some("tus"));
        let env = HashMap::new();

        let replies = (
            client.complete(&profile(), &prompt("one"), &env),
            client.complete(&profile(), &prompt("two"), &env),
        );

        assert_eq!(replies, (Some("tus".to_string()), Some("tus".to_string())));
    }

    #[test]
    fn should_remember_each_prompt_it_was_sent() {
        let client = ScriptedLlmClient::answering(None);
        let env = HashMap::new();
        client.complete(&profile(), &prompt("one"), &env);
        client.complete(&profile(), &prompt("two"), &env);

        let sent = client.prompts();

        assert_eq!(sent, vec![prompt("one"), prompt("two")]);
    }
}
