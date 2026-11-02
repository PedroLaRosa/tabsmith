use std::path::{Path, PathBuf};

/// Where the two logs tabsmith keeps live: the one written by the command line and the one written by the pty.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct LogPaths {
    cli_log: PathBuf,
    pty_log: PathBuf,
}

impl LogPaths {
    pub(crate) fn new(cli_log: PathBuf, pty_log: PathBuf) -> Self {
        LogPaths { cli_log, pty_log }
    }

    pub(crate) fn cli_log(&self) -> &Path {
        &self.cli_log
    }

    pub(crate) fn pty_log(&self) -> &Path {
        &self.pty_log
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn should_keep_the_cli_log_and_the_pty_log_it_was_built_with() {
        let paths = LogPaths::new(
            PathBuf::from("/logs/cli.log"),
            PathBuf::from("/logs/pty.log"),
        );

        assert_eq!(
            (paths.cli_log(), paths.pty_log()),
            (Path::new("/logs/cli.log"), Path::new("/logs/pty.log"))
        );
    }

    #[test]
    fn should_equal_a_copy_of_itself_and_differ_when_a_path_differs() {
        let paths = LogPaths::new(PathBuf::from("/a/cli.log"), PathBuf::from("/a/pty.log"));

        let other = LogPaths::new(PathBuf::from("/a/cli.log"), PathBuf::from("/b/pty.log"));

        assert_eq!((paths.clone() == paths, paths == other), (true, false));
    }
}
