//! The internal `_ pre-cmd` command (SPEC §2.11): saves the session context the shell reports and prunes stale run
//! files.
use crate::shared::infrastructure::data_directory_from_env::data_directory_from_env;
use crate::terminal_session::domain::entities::session_context::SessionContext;
use crate::terminal_session::domain::repositories::session_context_repository::SessionContextRepository;
use crate::terminal_session::domain::value_objects::session_id::SessionId;
use crate::terminal_session::infrastructure::adapters::file_session_context_repository::FileSessionContextRepository;
use crate::types::now_ms;
use std::time::Duration;

pub fn pre_cmd(alias: Option<String>) -> anyhow::Result<i32> {
    // The id becomes a file name: anything but a session id exits 1 silently, before the data directory is resolved.
    let Some(id) = std::env::var("TABSMITH_SESSION_ID")
        .ok()
        .and_then(|v| SessionId::new(v).ok())
    else {
        return Ok(1);
    };
    let env = std::env::vars_os()
        .filter_map(|(k, v)| Some((k.into_string().ok()?, v.into_string().ok()?)))
        .collect();
    let context = SessionContext::new(id, env, alias, now_ms());
    let session_contexts = FileSessionContextRepository::new(data_directory_from_env());
    session_contexts.save(&context)?;
    // Only after a successful save. A shell that exits right after a prompt leaves its run file behind.
    session_contexts.prune_older_than(Duration::from_secs(24 * 3600));
    Ok(0)
}
