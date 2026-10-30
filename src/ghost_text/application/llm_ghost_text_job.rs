use crate::ghost_text::application::llm_ghost_text_request::LlmGhostTextRequest;
use crate::ghost_text::application::ports::directory_probe::DirectoryProbe;
use crate::ghost_text::application::ports::llm_client::LlmClient;
use crate::ghost_text::domain::entities::ghost_text_cache::GhostTextCache;
use crate::ghost_text::domain::services::cd_target_validator::cd_target;
use crate::ghost_text::domain::services::llm_completion_cleaner::clean;
use crate::ghost_text::domain::services::llm_prompt_builder::llm_prompt;
use crate::ghost_text::domain::value_objects::llm_profile::LlmProfile;
use crate::history::domain::entities::history_entry::HistoryEntry;
use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, Mutex};

/// The blocking half of an LLM request: the prompt, the provider call, the cleaning and the `cd` check. It owns
/// everything it reads, so the caller can run it on another thread.
pub(crate) struct LlmGhostTextJob {
    profile: LlmProfile,
    request: LlmGhostTextRequest,
    history: Vec<HistoryEntry>,
    env: HashMap<String, String>,
    client: Arc<dyn LlmClient + Send + Sync>,
    directories: Arc<dyn DirectoryProbe + Send + Sync>,
    cache: Arc<Mutex<GhostTextCache>>,
}

impl LlmGhostTextJob {
    pub(crate) fn new(
        profile: LlmProfile,
        request: LlmGhostTextRequest,
        history: Vec<HistoryEntry>,
        env: HashMap<String, String>,
        client: Arc<dyn LlmClient + Send + Sync>,
        directories: Arc<dyn DirectoryProbe + Send + Sync>,
        cache: Arc<Mutex<GhostTextCache>>,
    ) -> Self {
        Self {
            profile,
            request,
            history,
            env,
            client,
            directories,
            cache,
        }
    }

    /// The completion to show, remembered in the cache before it is returned.
    pub(crate) fn run(self: Box<Self>) -> Option<String> {
        let text = self.complete();
        if let (Some(t), Ok(mut c)) = (&text, self.cache.lock()) {
            c.insert(&[format!("{}{t}", self.request.buffer)]);
        }
        text
    }

    /// Prompt with privacy filter, provider call, cleaning, validation (SPEC section 8.5). `None` when nothing
    /// should be suggested.
    fn complete(&self) -> Option<String> {
        let prompt = llm_prompt(
            &self.history,
            &self.request.buffer,
            &self.request.cwd,
            self.request.shell,
        )?;
        let completion = clean(
            &self.client.complete(&self.profile, &prompt, &self.env)?,
            &self.request.buffer,
        )?;
        let full = format!("{}{completion}", self.request.buffer);
        validate(&full, &self.request.cwd, &self.env, self.directories.as_ref()).then_some(completion)
    }
}

/// `cd` validation (SPEC §8.5): `full` = buffer + completion.
fn validate(
    full: &str,
    cwd: &Path,
    env: &HashMap<String, String>,
    directories: &dyn DirectoryProbe,
) -> bool {
    let Some(path) = cd_target(full, env, || directories.home_directory()) else {
        return true;
    };
    let path = Path::new(&path);
    if path.is_absolute() && !directories.is_dir(path) {
        return false;
    }
    directories.resolves_to_directory(&cwd.join(path))
}
