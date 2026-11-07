/// What a finished process wrote and how it exited (the `executeCommand` result).
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct ProcessOutput {
    stdout: String,
    stderr: String,
    status: i32,
}

impl ProcessOutput {
    pub(crate) fn new(stdout: String, stderr: String, status: i32) -> Self {
        ProcessOutput {
            stdout,
            stderr,
            status,
        }
    }

    pub(crate) fn stdout(&self) -> &str {
        &self.stdout
    }

    pub(crate) fn stderr(&self) -> &str {
        &self.stderr
    }

    pub(crate) fn status(&self) -> i32 {
        self.status
    }
}
