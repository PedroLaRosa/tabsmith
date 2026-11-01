/// What the environment says about the shell to launch. The driver reads it; an empty value counts as unset.
pub(crate) struct LaunchEnvironment {
    pub(crate) tabsmith_shell: Option<String>,
    pub(crate) system_shell: Option<String>,
    pub(crate) login_shell: bool,
}
