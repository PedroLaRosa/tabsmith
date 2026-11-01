use crate::terminal_session::infrastructure::pty::session_runner;
use crate::terminal_session::infrastructure::terminal_session_module::TerminalSessionModule;

/// `_ pty`: the autocomplete session around the shell.
pub(crate) fn run_pty_command(
    shell: Option<String>,
    cmd: Vec<String>,
    module: &TerminalSessionModule,
) -> anyhow::Result<i32> {
    Ok(session_runner::run(shell, cmd, module))
}
