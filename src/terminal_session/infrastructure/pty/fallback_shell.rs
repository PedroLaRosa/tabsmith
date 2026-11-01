use crate::shared::infrastructure::logger;
use crate::terminal_session::application::ports::generator_process_reaper::GeneratorProcessReaper;

/// Run the plain shell with `TABSMITH_TERM_DISABLED=1` so its pre block does not loop (SPEC §4.4 fallback).
pub(crate) fn run_fallback_shell(err: &anyhow::Error, argv: &[String], reaper: &dyn GeneratorProcessReaper) -> i32 {
    logger::error(&format!("pty: {err:#}"));
    eprintln!("tabsmith had an error: {err:#}");
    reaper.kill_live_children(); // exec skips atexit

    let argv = if argv.is_empty() {
        vec![if cfg!(windows) { "pwsh" } else { "/bin/sh" }.to_string()]
    } else {
        argv.to_vec()
    };
    let mut c = std::process::Command::new(&argv[0]);
    c.args(&argv[1..])
        .env("TABSMITH_TERM_DISABLED", "1")
        .env_remove("TABSMITH_SHELL")
        .env_remove("TABSMITH_IS_LOGIN_SHELL");
    #[cfg(unix)]
    let err = std::os::unix::process::CommandExt::exec(&mut c); // returns only on error
    #[cfg(windows)]
    let err = match c.status() {
        Ok(s) => return s.code().unwrap_or(1),
        Err(e) => e,
    };
    eprintln!("tabsmith: could not start {}: {err}", argv[0]);
    1
}
