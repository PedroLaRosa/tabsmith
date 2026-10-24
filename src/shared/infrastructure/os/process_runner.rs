use anyhow::{bail, Result};
use std::io::Read;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

/// Run `cmd` (stdin null, stderr ignored) and return its stdout; killed after `timeout`.
pub(crate) fn output_with_timeout(cmd: &mut Command, timeout: Duration) -> Result<String> {
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
            bail!("timed out after {}s", timeout.as_secs());
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    let mut out = String::new();
    if let Some(mut s) = child.stdout.take() {
        s.read_to_string(&mut out)?;
    }
    Ok(out)
}
