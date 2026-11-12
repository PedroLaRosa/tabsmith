use crate::modules::shell_integration::domain::shell_integration_error::ShellIntegrationError;
use std::path::{Path, PathBuf};

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

    pub(crate) fn path(&self) -> &Path {
        &self.path
    }

    pub(crate) fn contents(&self) -> &str {
        &self.contents
    }

    /// The file must hold exactly the contents; `actual` is what it holds.
    pub(crate) fn check(&self, actual: &str) -> Result<(), ShellIntegrationError> {
        if actual == self.contents {
            Ok(())
        } else {
            Err(ShellIntegrationError::Improper(format!(
                "{} should contain:\n{}",
                self.path.display(),
                self.contents
            )))
        }
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

    /// A moved binary: the file still runs the old one.
    #[test]
    fn should_be_improper_when_the_file_holds_anything_else_and_say_what_it_should_contain() {
        let script = bashrc_pre_script();

        let checked = script.check(
            "[ -n \"$BASH_VERSION\" ] && [ -x /elsewhere/tabsmith ] && eval \"$(/elsewhere/tabsmith init bash pre --rcfile bashrc)\"\n",
        );

        assert_eq!(
            checked.unwrap_err().to_string(),
            "Improper integration installation: /home/u/.tabsmith/shell/bashrc.pre.bash should contain:\n[ -n \"$BASH_VERSION\" ] && [ -x /usr/local/bin/tabsmith ] && eval \"$(/usr/local/bin/tabsmith init bash pre --rcfile bashrc)\"\n"
        );
    }
}
