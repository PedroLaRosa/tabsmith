use crate::modules::terminal_session::application::launch_environment::LaunchEnvironment;

/// What the user asked to launch under the wrapper: a shell (`--shell`), the arguments after `--`, and the
/// environment around them.
pub(crate) struct LaunchRequest {
    pub(crate) shell: Option<String>,
    pub(crate) command: Vec<String>,
    pub(crate) environment: LaunchEnvironment,
}
