use crate::shell_integration::domain::shell_integration_error::ShellIntegrationError;
use std::path::PathBuf;

/// A file tabsmith writes for an integration, `~/.tabsmith/shell/<rc>.<phase>.<ext>` or a whole fish `conf.d` file:
/// its path and exact contents.
#[derive(Debug)]
pub(crate) struct IntegrationScript {
    path: PathBuf,
    contents: String,
}

impl IntegrationScript {
    pub(crate) fn new(path: PathBuf, contents: String) -> Self {
        IntegrationScript { path, contents }
    }

    /// `actual` is what the file holds.
    pub(crate) fn check(&self, _actual: &str) -> Result<(), ShellIntegrationError> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bashrc_pre_script() -> IntegrationScript {
        IntegrationScript::new(
            PathBuf::from("/home/u/.tabsmith/shell/bashrc.pre.bash"),
            "[ -n \"$BASH_VERSION\" ] && [ -x /usr/local/bin/tabsmith ] && eval \"$(/usr/local/bin/tabsmith init bash pre --rcfile bashrc)\"\n".to_string(),
        )
    }

    #[test]
    fn should_accept_a_file_that_holds_exactly_its_contents() {
        let script = bashrc_pre_script();

        let checked = script.check(
            "[ -n \"$BASH_VERSION\" ] && [ -x /usr/local/bin/tabsmith ] && eval \"$(/usr/local/bin/tabsmith init bash pre --rcfile bashrc)\"\n",
        );

        assert!(checked.is_ok(), "{checked:?}");
    }
}
