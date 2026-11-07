use crate::modules::ghost_text::application::llm_ghost_text_request::LlmGhostTextRequest;
use crate::modules::ghost_text::application::ports::directory_probe::DirectoryProbe;
use crate::modules::ghost_text::application::ports::llm_client::LlmClient;
use crate::modules::ghost_text::domain::entities::ghost_text_cache::GhostTextCache;
use crate::modules::ghost_text::domain::services::cd_target_validator::cd_target;
use crate::modules::ghost_text::domain::services::llm_completion_cleaner::clean;
use crate::modules::ghost_text::domain::services::llm_prompt_builder::llm_prompt;
use crate::modules::ghost_text::domain::value_objects::llm_profile::LlmProfile;
use crate::modules::history::domain::entities::history_entry::HistoryEntry;
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
        if let (Some(completion), Ok(mut cache)) = (&text, self.cache.lock()) {
            cache.insert(&[format!("{}{completion}", self.request.buffer)]);
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::ghost_text::infrastructure::adapters::fs_directory_probe::FsDirectoryProbe;
    use crate::modules::shared::infrastructure::testing::temp_dir::TempDir;

    #[test]
    fn cd_validation() {
        let temp_dir = TempDir::new("cd");
        let d = temp_dir.path().to_path_buf();
        std::fs::create_dir_all(d.join("a/b")).unwrap();
        std::fs::create_dir_all(d.join("space in name")).unwrap();
        std::fs::write(d.join("file"), "x").unwrap();
        std::fs::write(d.join("a/file"), "x").unwrap();
        let home = d.to_string_lossy().into_owned();
        let env: HashMap<String, String> = [("HOME", home.as_str()), ("A", "a"), ("B", "b")]
            .into_iter()
            .map(|(k, v)| (k.into(), v.into()))
            .collect();
        let valid = |c: &str| validate(c, &d, &env, &FsDirectoryProbe);
        for ok in [
            "echo \"hello\"",
            "cd ~",
            "cd ~/",
            "cd ~/a/b",
            "cd ~/$A/$B",
            "cd ${A}/b",
            "cd a/",
            "cd $A",
            "cd space\\ in\\ name",
            "cd \"space in name\"",
            "cd 'space in name'",
            "cd space in name",
            "cd $UNKNOWN/x",
            "cd \"open",
            &format!("cd {home}/a"),
        ] {
            assert!(valid(ok), "{ok}");
        }
        for bad in [
            "cd /folder/doesnt/exist",
            "cd ~/file",
            "cd ~/$A/file",
            "cd b",
            "cd file",
            "cd $B",
            "cd a/b/file",
        ] {
            assert!(!valid(bad), "{bad}");
        }
        assert!(!valid(&format!("cd {home}/file")));
        std::fs::remove_dir_all(&d).unwrap();
    }
}
