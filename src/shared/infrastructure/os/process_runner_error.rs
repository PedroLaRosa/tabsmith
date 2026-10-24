use std::fmt;
use std::io;

/// Why [`output_with_timeout`](super::process_runner::output_with_timeout) returned no output. Its Display and
/// `source()` chain print what its `anyhow` error printed before (`{:#}`): the bare I/O error, or
/// "timed out after <secs>s".
#[derive(Debug)]
pub(crate) enum ProcessRunnerError {
    /// Spawning, waiting for or reading the program failed.
    Io(io::Error),
    /// The program was still running at the timeout and was killed.
    TimedOut { secs: u64 },
}

impl From<io::Error> for ProcessRunnerError {
    fn from(error: io::Error) -> Self {
        ProcessRunnerError::Io(error)
    }
}

impl fmt::Display for ProcessRunnerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ProcessRunnerError::Io(error) => write!(f, "{error}"),
            ProcessRunnerError::TimedOut { secs } => write!(f, "timed out after {secs}s"),
        }
    }
}

impl std::error::Error for ProcessRunnerError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            // Display already is the I/O error: continue with its own cause so nothing prints twice.
            ProcessRunnerError::Io(error) => error.source(),
            ProcessRunnerError::TimedOut { .. } => None,
        }
    }
}
