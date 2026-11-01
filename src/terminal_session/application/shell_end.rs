/// How the wrapped shell's side of the session ended, as the runner learns it.
pub(crate) enum ShellEnd {
    /// The child exited with this code.
    ChildExited(i32),
    /// The pty reached EOF: the shell's output is over.
    OutputClosed,
    /// The wrapper was told to stop (SIGTERM and SIGHUP alike).
    Terminated,
}
