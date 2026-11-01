use crate::pty;
use crate::terminal_session::infrastructure::terminal_session_module::TerminalSessionModule;

/// `_ pty`: the autocomplete session around the shell. It still runs on the legacy driver until 9.6d.
pub(crate) fn run_pty_command(
    shell: Option<String>,
    cmd: Vec<String>,
    module: &TerminalSessionModule,
) -> anyhow::Result<i32> {
    Ok(pty::run(shell, cmd, module))
}
