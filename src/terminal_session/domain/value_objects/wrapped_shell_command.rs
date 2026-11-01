use crate::shared::domain::value_objects::shell::Shell;

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

    pub(crate) fn program(&self) -> Option<&str> {
        self.argv.first().map(String::as_str)
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
}
