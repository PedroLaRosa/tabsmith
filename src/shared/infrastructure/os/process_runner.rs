use crate::shared::infrastructure::os::process_runner_error::ProcessRunnerError;
use std::io::Read;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

/// Run `cmd` (stdin null, stderr ignored) and return its stdout; killed after `timeout`.
pub(crate) fn output_with_timeout(cmd: &mut Command, timeout: Duration) -> Result<String, ProcessRunnerError> {
    let mut child = cmd
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()?;
    let start = Instant::now();
    while child.try_wait()?.is_none() {
        if start.elapsed() > timeout {
            let _ = child.kill();
            let _ = child.wait();
            return Err(ProcessRunnerError::TimedOut { secs: timeout.as_secs() });
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    let mut out = String::new();
    if let Some(mut s) = child.stdout.take() {
        s.read_to_string(&mut out)?;
    }
    Ok(out)
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use anyhow::Context;

    #[test]
    fn should_report_a_program_still_running_at_the_timeout_as_timed_out() {
        let mut sleep = Command::new("sleep");
        sleep.arg("5");

        let error = output_with_timeout(&mut sleep, Duration::ZERO)
            .context("could not ask pwsh for $PROFILE")
            .unwrap_err();

        assert_eq!(format!("{error:#}"), "could not ask pwsh for $PROFILE: timed out after 0s");
    }

    #[test]
    fn should_report_a_program_that_cannot_start_as_the_bare_io_error() {
        let mut missing = Command::new("tabsmith-test-no-such-program");

        let error = output_with_timeout(&mut missing, Duration::from_secs(5))
            .context("could not ask pwsh for $PROFILE")
            .unwrap_err();

        assert_eq!(
            format!("{error:#}"),
            "could not ask pwsh for $PROFILE: No such file or directory (os error 2)"
        );
    }

    #[test]
    fn should_return_what_the_program_printed() {
        let mut echo = Command::new("echo");
        echo.arg("hi");

        let out = output_with_timeout(&mut echo, Duration::from_secs(5)).unwrap();

        assert_eq!(out, "hi\n");
    }
}
