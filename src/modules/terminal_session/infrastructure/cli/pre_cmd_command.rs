use crate::modules::terminal_session::domain::value_objects::session_id::SessionId;
use crate::modules::terminal_session::infrastructure::terminal_session_module::TerminalSessionModule;

/// `_ pre-cmd` (SPEC §2.11), run in the background at every prompt by the bash, zsh and fish post scripts: saves the
/// env and alias text the shell reports as its session's context, then prunes stale run files. Exit 1 without a word
/// for an id that is not a session id.
pub(crate) fn run_pre_cmd_command(alias: Option<String>, module: &TerminalSessionModule) -> anyhow::Result<i32> {
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
    module.report_session_context.execute(id, env, alias)?;
    Ok(0)
}
