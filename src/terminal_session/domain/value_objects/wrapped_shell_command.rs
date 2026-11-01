use crate::shared::domain::value_objects::shell::Shell;
use crate::terminal_session::domain::value_objects::session_id::SessionId;

/// The program a wrapped launch runs and its arguments.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct WrappedShellCommand {
    argv: Vec<String>,
    env_to_set: Vec<(String, String)>,
    env_to_remove: Vec<&'static str>,
}

impl WrappedShellCommand {
    pub(crate) fn new(shell: Option<&str>, extra: &[String], login: bool) -> Self {
        WrappedShellCommand {
            argv: resolve_argv(shell, extra, login),
            env_to_set: Vec::new(),
            env_to_remove: Vec::new(),
        }
    }

    pub(crate) fn argv(&self) -> &[String] {
        &self.argv
    }

    /// The environment the child of a wrapped session gets: the wrapper's version, the tmux marker only inside
    /// tmux and the session id are set; the variables that chose the shell are removed.
    pub(crate) fn in_session(mut self, version: &str, in_tmux: bool, id: &SessionId) -> Self {
        self.env_to_set.push(("TABSMITH_TERM".to_string(), version.to_string()));
        if in_tmux {
            self.env_to_set
                .push(("TABSMITH_TERM_TMUX".to_string(), version.to_string()));
        }
        self.env_to_set
            .push(("TABSMITH_SESSION_ID".to_string(), id.as_str().to_string()));
        self.env_to_remove = vec!["TABSMITH_SHELL", "TABSMITH_IS_LOGIN_SHELL"];
        self
    }

    pub(crate) fn program(&self) -> Option<&str> {
        self.argv.first().map(String::as_str)
    }

    pub(crate) fn env_to_set(&self) -> &[(String, String)] {
        &self.env_to_set
    }

    pub(crate) fn env_to_remove(&self) -> &[&'static str] {
        &self.env_to_remove
    }

    /// The title escape the wrapper writes once the child runs: the program's name without its directory.
    pub(crate) fn window_title(&self) -> Vec<u8> {
        let Some(program) = self.program() else {
            return Vec::new();
        };
        let base = program.rsplit(['/', '\\']).next().unwrap_or(program);
        format!("\x1b]0;{base}\x07").into_bytes()
    }
}

/// Child argv (SPEC §4.4 step 7). After `--`: a command (replaces the shell), or extra shell arguments when they
/// start with `-` (`debug shell`: `--shell bash -- --noprofile --rcfile …`).
fn resolve_argv(shell: Option<&str>, extra: &[String], login: bool) -> Vec<String> {
    if extra.first().is_some_and(|a| !a.starts_with('-')) {
        return extra.to_vec();
    }
    let Some(shell) = shell else {
        return Vec::new();
    };
    let mut v = vec![shell.to_string()];
    if login
        && matches!(
            Shell::from_name(shell),
            Some(Shell::Bash | Shell::Zsh | Shell::Fish)
        )
    {
        v.push("--login".into());
    }
    v.extend(extra.iter().cloned());
    v
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn child_argv() {
        let argv = |a: Option<&str>, b: &[String], c: bool| WrappedShellCommand::new(a, b, c).argv().to_vec();
        let s = |v: &[&str]| v.iter().map(|x| x.to_string()).collect::<Vec<_>>();
        assert_eq!(
            argv(Some("/bin/zsh"), &[], true),
            s(&["/bin/zsh", "--login"])
        );
        assert_eq!(argv(Some("pwsh"), &[], true), s(&["pwsh"]));
        assert_eq!(
            argv(Some("bash"), &s(&["--noprofile", "--rcfile", "x"]), false),
            s(&["bash", "--noprofile", "--rcfile", "x"])
        );
        assert_eq!(
            argv(Some("bash"), &s(&["vim", "-u"]), true),
            s(&["vim", "-u"])
        );
        assert!(argv(None, &[], false).is_empty());
    }

    fn pair(name: &str, value: &str) -> (String, String) {
        (name.to_string(), value.to_string())
    }

    #[test]
    fn should_set_the_version_tmux_marker_and_session_id_and_remove_the_shell_choice_variables() {
        let id = SessionId::new("abc-123".to_string()).unwrap();

        let command = WrappedShellCommand::new(Some("/bin/zsh"), &[], false).in_session("1.2.3", true, &id);

        assert_eq!(
            command.env_to_set(),
            [
                pair("TABSMITH_TERM", "1.2.3"),
                pair("TABSMITH_TERM_TMUX", "1.2.3"),
                pair("TABSMITH_SESSION_ID", "abc-123"),
            ]
        );
        assert_eq!(command.env_to_remove(), ["TABSMITH_SHELL", "TABSMITH_IS_LOGIN_SHELL"]);
    }

    #[test]
    fn should_not_mark_tmux_outside_tmux() {
        let id = SessionId::new("abc-123".to_string()).unwrap();

        let command = WrappedShellCommand::new(Some("/bin/zsh"), &[], false).in_session("1.2.3", false, &id);

        assert_eq!(
            command.env_to_set(),
            [pair("TABSMITH_TERM", "1.2.3"), pair("TABSMITH_SESSION_ID", "abc-123")]
        );
    }

    #[test]
    fn should_start_nothing_without_a_shell_or_a_command() {
        let command = WrappedShellCommand::new(None, &[], false);

        assert_eq!(
            (command.program(), command.window_title()),
            (None, Vec::<u8>::new())
        );
    }

    #[test]
    fn should_title_the_window_with_the_program_name_after_the_last_separator() {
        let unix = WrappedShellCommand::new(Some("/usr/local/bin/fish"), &[], false);
        let windows = WrappedShellCommand::new(Some("C:\\Program Files\\PowerShell\\pwsh.exe"), &[], false);

        assert_eq!(
            (unix.window_title(), windows.window_title()),
            (b"\x1b]0;fish\x07".to_vec(), b"\x1b]0;pwsh.exe\x07".to_vec())
        );
    }

    #[test]
    fn should_title_the_window_with_the_whole_program_when_it_has_no_separator() {
        let command = WrappedShellCommand::new(Some("pwsh"), &[], false);

        assert_eq!(command.window_title(), b"\x1b]0;pwsh\x07".to_vec());
    }
}
