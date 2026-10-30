use crate::ghost_text::application::clear_ghost_text_cache_use_case::ClearGhostTextCacheUseCase;
use crate::ghost_text::application::ports::directory_probe::DirectoryProbe;
use crate::ghost_text::application::ports::llm_client::LlmClient;
use crate::ghost_text::application::request_llm_ghost_text_use_case::RequestLlmGhostTextUseCase;
use crate::ghost_text::application::suggest_local_ghost_text_use_case::SuggestLocalGhostTextUseCase;
use crate::ghost_text::domain::entities::ghost_text_cache::GhostTextCache;
use crate::ghost_text::infrastructure::adapters::fs_directory_probe::FsDirectoryProbe;
use crate::ghost_text::infrastructure::adapters::http_llm_client::HttpLlmClient;
use crate::history::domain::repositories::history_repository::HistoryRepository;
use std::rc::Rc;
use std::sync::{Arc, Mutex};

/// The ghost text use cases of the engine thread, over the history the history module records into. The LLM
/// request and the cache clear share one cache.
pub(crate) struct GhostTextModule {
    pub(crate) suggest_local_ghost_text: SuggestLocalGhostTextUseCase,
    pub(crate) request_llm_ghost_text: RequestLlmGhostTextUseCase,
    pub(crate) clear_ghost_text_cache: ClearGhostTextCacheUseCase,
}

impl GhostTextModule {
    pub(crate) fn new(
        history: Rc<dyn HistoryRepository>,
        client: Arc<dyn LlmClient + Send + Sync>,
        directories: Arc<dyn DirectoryProbe + Send + Sync>,
    ) -> Self {
        let cache = Arc::new(Mutex::new(GhostTextCache::default()));
        Self {
            suggest_local_ghost_text: SuggestLocalGhostTextUseCase::new(Rc::clone(&history)),
            request_llm_ghost_text: RequestLlmGhostTextUseCase::new(
                history,
                Arc::clone(&cache),
                client,
                directories,
            ),
            clear_ghost_text_cache: ClearGhostTextCacheUseCase::new(cache),
        }
    }
}

pub(crate) fn build_ghost_text_module(history: Rc<dyn HistoryRepository>) -> GhostTextModule {
    GhostTextModule::new(history, Arc::new(HttpLlmClient), Arc::new(FsDirectoryProbe))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ghost_text::application::llm_ghost_text_answer::LlmGhostTextAnswer;
    use crate::ghost_text::application::llm_ghost_text_request::LlmGhostTextRequest;
    use crate::ghost_text::application::ports::directory_probe::InMemoryDirectoryProbe;
    use crate::ghost_text::application::ports::llm_client::ScriptedLlmClient;
    use crate::history::domain::repositories::history_repository::InMemoryHistoryRepository;
    use crate::settings::infrastructure::adapters::setting_value_json::settings_from_json;
    use crate::shared::domain::value_objects::shell::Shell;
    use serde_json::json;
    use std::collections::HashMap;
    use std::path::PathBuf;

    #[test]
    fn should_clear_the_completions_the_llm_request_cached() {
        let module = GhostTextModule::new(
            Rc::new(InMemoryHistoryRepository::new(vec![])),
            Arc::new(ScriptedLlmClient::answering(Some("git status"))),
            Arc::new(InMemoryDirectoryProbe::new(&[], None)),
        );
        let settings = settings_from_json(
            json!({
                "inline.source": "p",
                "inline.profiles.p": { "provider": "openai", "model": "m" },
            })
            .as_object()
            .unwrap()
            .clone(),
        );
        let request = || LlmGhostTextRequest {
            buffer: "git sta".into(),
            cwd: PathBuf::from("/work"),
            shell: Shell::Zsh,
        };
        let env = HashMap::new();
        if let LlmGhostTextAnswer::Deferred(job) = module.request_llm_ghost_text.execute(&settings, &env, request()) {
            job.run();
        }
        module.clear_ghost_text_cache.execute();

        let answer = module.request_llm_ghost_text.execute(&settings, &env, request());

        assert!(matches!(answer, LlmGhostTextAnswer::Deferred(_)));
    }
}
