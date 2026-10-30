//! Inline ghost text: local sources, LLM profiles/clients, completion cache, validation (SPEC §8,
//! §3.5, §6.11). OWNER: inline agent. Used by the engine (sources, LLM worker), the CLI (`inline` command) and
//! doctor (profile checks).
use crate::ghost_text::application::ports::directory_probe::DirectoryProbe;
use crate::ghost_text::application::ports::llm_client::LlmClient;
use crate::ghost_text::domain::services::cd_target_validator::cd_target;
use crate::ghost_text::domain::services::llm_completion_cleaner::clean;
use crate::ghost_text::domain::services::llm_prompt_builder::llm_prompt;
use crate::ghost_text::domain::value_objects::llm_profile::LlmProfile;
use crate::ghost_text::infrastructure::adapters::fs_directory_probe::FsDirectoryProbe;
use crate::ghost_text::infrastructure::adapters::http_llm_client::HttpLlmClient;
use crate::shared::domain::value_objects::shell::Shell;
use crate::history::domain::entities::history_entry::HistoryEntry;
use std::collections::HashMap;
use std::path::Path;

/// Blocking LLM request (worker thread): prompt with privacy filter, HTTP, cleaning, validation (SPEC §8.5).
/// `None` on any error (logged) or when nothing should be suggested. `history` = `llm_history(..)` snapshot.
pub fn llm_complete(
    profile: &LlmProfile,
    buffer: &str,
    cwd: &Path,
    shell: Shell,
    history: &[HistoryEntry],
    env: &HashMap<String, String>,
) -> Option<String> {
    let prompt = llm_prompt(history, buffer, cwd, shell)?;
    let completion = clean(&HttpLlmClient.complete(profile, &prompt, env)?, buffer)?;
    let full = format!("{buffer}{completion}");
    validate(&full, cwd, env, &FsDirectoryProbe).then_some(completion)
}

/// `cd` validation (SPEC §8.5): `full` = buffer + completion.
pub fn validate(
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
    use crate::shared::infrastructure::testing::temp_dir::TempDir;

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
