use crate::shared::infrastructure::logger;
use crate::terminal_session::application::launch_environment::LaunchEnvironment;
use crate::terminal_session::application::launch_request::LaunchRequest;
use crate::terminal_session::infrastructure::pty::fallback_shell::run_fallback_shell;
use crate::terminal_session::infrastructure::pty::session_launcher::launch_session;
use crate::terminal_session::infrastructure::pty::session_runner::SessionRunner;
use crate::terminal_session::infrastructure::terminal_session_module::TerminalSessionModule;
use anyhow::anyhow;
use std::panic::AssertUnwindSafe;

/// Run the wrapper; returns the exit code to exit with. Never leaves the user without a shell (fallback, SPEC §4.4).
pub(crate) fn run_pty_session(shell: Option<String>, cmd: Vec<String>, module: &TerminalSessionModule) -> i32 {
    // Panics are logged, never printed over the user's screen; the session then falls back to a plain shell.
    std::panic::set_hook(Box::new(|info| logger::error(&format!("panic: {info}"))));
    // The environment facts are read before the session starts: the argv also feeds the fallback.
    let command = module.prepare_session_launch.execute(LaunchRequest {
        shell,
        command: cmd,
        environment: LaunchEnvironment {
            tabsmith_shell: env_nonempty("TABSMITH_SHELL"),
            system_shell: env_nonempty("SHELL"),
            login_shell: std::env::var("TABSMITH_IS_LOGIN_SHELL").as_deref() == Ok("1"),
        },
    });
    let mut modes = None;
    let err = match attempt(|| launch_session(&command, &mut modes, module).map(SessionRunner::run)) {
        Ok(code) => return code,
        Err(e) => e,
    };
    if let Some(m) = modes {
        m.restore();
    }
    run_fallback_shell(&err, command.argv(), &*module.generator_processes())
}

fn env_nonempty(k: &str) -> Option<String> {
    std::env::var(k).ok().filter(|v| !v.is_empty())
}

/// The outer panic tier: a panic outside a handler of the running session becomes an error for the fallback shell.
fn attempt(run: impl FnOnce() -> anyhow::Result<i32>) -> anyhow::Result<i32> {
    match std::panic::catch_unwind(AssertUnwindSafe(run)) {
        Ok(outcome) => outcome,
        Err(_) => Err(anyhow!("internal error (see pty.log)")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn should_return_the_exit_code_when_the_session_ends_normally() {
        let outcome = attempt(|| Ok(7));

        assert_eq!(outcome.unwrap(), 7);
    }

    #[test]
    fn should_pass_a_failed_start_through_as_the_error() {
        let outcome = attempt(|| Err(anyhow::anyhow!("stdin/stdout is not a terminal")));

        assert_eq!(outcome.unwrap_err().to_string(), "stdin/stdout is not a terminal");
    }

    #[test]
    fn should_report_an_internal_error_when_the_session_panics_outside_a_handler() {
        let outcome = attempt(|| -> anyhow::Result<i32> { panic!("the launch panicked") });

        assert_eq!(outcome.unwrap_err().to_string(), "internal error (see pty.log)");
    }
}
