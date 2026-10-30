use crate::ghost_text::application::llm_ghost_text_answer::LlmGhostTextAnswer;
use crate::ghost_text::application::llm_ghost_text_job::LlmGhostTextJob;
use crate::ghost_text::application::llm_ghost_text_request::LlmGhostTextRequest;
use crate::ghost_text::application::ports::directory_probe::DirectoryProbe;
use crate::ghost_text::application::ports::llm_client::LlmClient;
use crate::ghost_text::domain::entities::ghost_text_cache::GhostTextCache;
use crate::ghost_text::domain::services::ghost_text_source_resolver::source;
use crate::ghost_text::domain::services::llm_prompt_builder;
use crate::ghost_text::domain::value_objects::ghost_text_source::GhostTextSource;
use crate::history::domain::entities::history_entry::HistoryEntry;
use crate::history::domain::repositories::history_repository::HistoryRepository;
use crate::settings::domain::services::settings_catalog::INLINE_HISTORY_COUNT;
use crate::settings::domain::value_objects::settings::Settings;
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::{Arc, Mutex};

/// LLM ghost text (SPEC section 8.4): answers at once from the cache or without a profile, otherwise hands back
/// the job that asks the provider.
pub(crate) struct RequestLlmGhostTextUseCase {
    history: Rc<dyn HistoryRepository>,
    cache: Arc<Mutex<GhostTextCache>>,
    client: Arc<dyn LlmClient + Send + Sync>,
    directories: Arc<dyn DirectoryProbe + Send + Sync>,
}

impl RequestLlmGhostTextUseCase {
    pub(crate) fn new(
        history: Rc<dyn HistoryRepository>,
        cache: Arc<Mutex<GhostTextCache>>,
        client: Arc<dyn LlmClient + Send + Sync>,
        directories: Arc<dyn DirectoryProbe + Send + Sync>,
    ) -> Self {
        Self {
            history,
            cache,
            client,
            directories,
        }
    }

    pub(crate) fn execute(
        &self,
        settings: &Settings,
        env: &HashMap<String, String>,
        request: LlmGhostTextRequest,
    ) -> LlmGhostTextAnswer {
        let Ok(GhostTextSource::Profile(profile)) = source(settings) else {
            return LlmGhostTextAnswer::Immediate(None);
        };
        if let Some(text) = self.cache.lock().ok().and_then(|c| c.lookup(&request.buffer)) {
            return LlmGhostTextAnswer::Immediate(Some(text));
        }
        let history = llm_history(settings, &self.history.recent());
        let env = env.clone();
        LlmGhostTextAnswer::Deferred(Box::new(LlmGhostTextJob::new(
            profile,
            request,
            history,
            env,
            Arc::clone(&self.client),
            Arc::clone(&self.directories),
            Arc::clone(&self.cache),
        )))
    }
}

/// The history an LLM request may send: the last `inline.historyCount` (49) usable entries (SPEC §8.5).
fn llm_history(settings: &Settings, entries: &[HistoryEntry]) -> Vec<HistoryEntry> {
    llm_prompt_builder::llm_history(entries, settings.i64(INLINE_HISTORY_COUNT).max(0) as usize)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ghost_text::application::ports::directory_probe::InMemoryDirectoryProbe;
    use crate::ghost_text::application::ports::llm_client::ScriptedLlmClient;
    use crate::ghost_text::domain::services::llm_prompt_builder::SYSTEM;
    use crate::ghost_text::domain::value_objects::llm_prompt::LlmPrompt;
    use crate::history::domain::entities::history_entry::HistoryEntryPrimitives;
    use crate::history::domain::repositories::history_repository::InMemoryHistoryRepository;
    use crate::settings::infrastructure::adapters::setting_value_json::settings_from_json;
    use crate::shared::domain::value_objects::shell::Shell;
    use serde_json::{json, Value};
    use std::path::{Path, PathBuf};

    fn entry(command: &str, exit_code: i32) -> HistoryEntry {
        HistoryEntry::new(HistoryEntryPrimitives {
            command: command.into(),
            shell: "zsh".into(),
            exit_code: Some(exit_code),
            ..Default::default()
        })
    }

    fn settings(value: serde_json::Value) -> Settings {
        settings_from_json(value.as_object().unwrap().clone())
    }

    fn profile_source() -> Settings {
        settings(json!({
            "inline.source": "p",
            "inline.profiles.p": { "provider": "openai", "model": "m" },
        }))
    }

    fn request(buffer: &str) -> LlmGhostTextRequest {
        LlmGhostTextRequest {
            buffer: buffer.into(),
            cwd: PathBuf::from("/work"),
            shell: Shell::Zsh,
        }
    }

    fn use_case(
        history: Vec<HistoryEntry>,
        client: &Arc<ScriptedLlmClient>,
        directories: InMemoryDirectoryProbe,
    ) -> RequestLlmGhostTextUseCase {
        RequestLlmGhostTextUseCase::new(
            Rc::new(InMemoryHistoryRepository::new(history)),
            Arc::new(Mutex::new(GhostTextCache::default())),
            client.clone(),
            Arc::new(directories),
        )
    }

    fn ask(use_case: &RequestLlmGhostTextUseCase, buffer: &str) -> LlmGhostTextAnswer {
        use_case.execute(&profile_source(), &HashMap::new(), request(buffer))
    }

    /// What the engine's `llm` thread does with an answer.
    fn run(answer: LlmGhostTextAnswer) -> Option<String> {
        match answer {
            LlmGhostTextAnswer::Deferred(job) => job.run(),
            LlmGhostTextAnswer::Immediate(text) => text,
        }
    }

    #[test]
    fn should_suggest_the_cleaned_completion_the_profile_returns() {
        let client = Arc::new(ScriptedLlmClient::answering(Some("git status")));
        let use_case = use_case(vec![], &client, InMemoryDirectoryProbe::new(&[], None));

        let text = run(ask(&use_case, "git sta"));

        assert_eq!(text, Some("tus".to_string()));
    }

    #[test]
    fn should_answer_from_the_cache_without_asking_again() {
        let client = Arc::new(ScriptedLlmClient::answering(Some("git status")));
        let use_case = use_case(vec![], &client, InMemoryDirectoryProbe::new(&[], None));
        run(ask(&use_case, "git sta"));

        let answer = ask(&use_case, "git stat");

        assert!(matches!(&answer, LlmGhostTextAnswer::Immediate(Some(text)) if text == "us"));
        assert_eq!(client.prompts().len(), 1);
    }

    #[test]
    fn should_answer_nothing_at_once_for_a_source_that_is_not_a_profile() {
        let client = Arc::new(ScriptedLlmClient::answering(Some("git status")));
        let use_case = use_case(vec![], &client, InMemoryDirectoryProbe::new(&[], None));

        let answer = use_case.execute(&Settings::default(), &HashMap::new(), request("git sta"));

        assert!(matches!(answer, LlmGhostTextAnswer::Immediate(None)));
        assert!(client.prompts().is_empty());
    }

    #[test]
    fn should_not_ask_about_a_private_buffer() {
        let client = Arc::new(ScriptedLlmClient::answering(Some("secret")));
        let use_case = use_case(vec![], &client, InMemoryDirectoryProbe::new(&[], None));

        let text = run(ask(&use_case, "export API_KEY="));

        assert_eq!(text, None);
        assert!(client.prompts().is_empty());
    }

    #[test]
    fn should_drop_a_cd_into_a_directory_that_does_not_exist() {
        let client = Arc::new(ScriptedLlmClient::answering(Some("nowhere")));
        let use_case = use_case(vec![], &client, InMemoryDirectoryProbe::new(&["/work/src"], None));

        let text = run(ask(&use_case, "cd "));

        assert_eq!(text, None);
        assert_eq!(client.prompts().len(), 1);
    }

    #[test]
    fn should_send_the_os_shell_cwd_and_numbered_history() {
        let client = Arc::new(ScriptedLlmClient::answering(Some("git status")));
        let use_case = use_case(vec![entry("ls", 0)], &client, InMemoryDirectoryProbe::new(&[], None));

        run(ask(&use_case, "git sta"));

        let expected = LlmPrompt::new(
            SYSTEM.to_string(),
            format!(
                "OS: {}\nShell: zsh\nCWD: {}\n\n    1  ls\n    2  git sta",
                std::env::consts::OS,
                Path::new("/work").display()
            ),
        );
        assert_eq!(client.prompts(), vec![expected]);
    }

    #[test]
    fn should_not_cache_a_reply_that_suggests_nothing() {
        let client = Arc::new(ScriptedLlmClient::answering(None));
        let use_case = use_case(vec![], &client, InMemoryDirectoryProbe::new(&[], None));
        run(ask(&use_case, "git sta"));

        let answer = ask(&use_case, "git sta");

        assert!(matches!(answer, LlmGhostTextAnswer::Deferred(_)));
    }

    #[test]
    fn llm_history_honours_history_count() {
        // llm-history-count-ignored: only the last `inline.historyCount` usable entries leave the machine
        let e = |c: &str| HistoryEntry::new(HistoryEntryPrimitives {
            command: c.into(),
            exit_code: Some(0),
            ..Default::default()
        });
        let entries: Vec<HistoryEntry> = (0..100).map(|i| e(&format!("echo {i}"))).collect();
        let with = |v: Value| {
            settings_from_json(
                json!({ "inline.historyCount": v })
                    .as_object()
                    .unwrap()
                    .clone(),
            )
        };
        let got: Vec<String> = llm_history(&with(json!(2)), &entries)
            .into_iter()
            .map(|h| h.command().to_string())
            .collect();
        assert_eq!(got, ["echo 98", "echo 99"]);
        assert!(llm_history(&with(json!(0)), &entries).is_empty());
        assert_eq!(llm_history(&Settings::default(), &entries).len(), 49);
    }
}
