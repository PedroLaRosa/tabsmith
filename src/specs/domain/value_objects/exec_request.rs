use std::collections::HashMap;

/// A process a spec asks to run (`executeCommand` input `{command, args, cwd?, env?, timeout?}`).
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct ExecRequest {
    command: String,
    args: Vec<String>,
    cwd: Option<String>,
    /// A `None` value removes the variable from the child's env.
    env: HashMap<String, Option<String>>,
    /// Milliseconds; it can only raise the runtime's script timeout.
    timeout: Option<u64>,
}

impl ExecRequest {
    pub(crate) fn new(
        command: String,
        args: Vec<String>,
        cwd: Option<String>,
        env: HashMap<String, Option<String>>,
        timeout: Option<u64>,
    ) -> Self {
        ExecRequest {
            command,
            args,
            cwd,
            env,
            timeout,
        }
    }

    pub(crate) fn command(&self) -> &str {
        &self.command
    }

    pub(crate) fn args(&self) -> &Vec<String> {
        &self.args
    }

    pub(crate) fn cwd(&self) -> &Option<String> {
        &self.cwd
    }

    pub(crate) fn env(&self) -> &HashMap<String, Option<String>> {
        &self.env
    }

    pub(crate) fn timeout(&self) -> Option<u64> {
        self.timeout
    }
}
