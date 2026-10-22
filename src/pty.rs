//! `tabsmith _ pty`: the PTY wrapper session (SPEC §4.3–4.5, §7.2.4, §7.3, §7.6, §8).
//! OWNER: wrapper agent.

/// Run the wrapper; returns the exit code to exit with. Never leaves the user without a shell (fallback, SPEC §4.4).
pub fn run(shell: Option<String>, cmd: Vec<String>) -> i32 {
    let _ = (shell, cmd);
    todo!("wrapper agent")
}
