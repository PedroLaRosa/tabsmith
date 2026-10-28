use std::path::PathBuf;

/// `is_installed()` / `install()` failure (SPEC §5.5 display strings).
#[derive(Debug)]
pub(crate) enum ShellIntegrationError {
    NotInstalled(String),
    Improper(String),
    FileDoesNotExist(PathBuf),
    PermissionDenied(PathBuf),
    Io(PathBuf, std::io::Error),
}

impl std::fmt::Display for ShellIntegrationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ShellIntegrationError::NotInstalled(m) => write!(f, "Integration not installed: {m}"),
            ShellIntegrationError::Improper(m) => write!(f, "Improper integration installation: {m}"),
            ShellIntegrationError::FileDoesNotExist(p) => {
                write!(f, "File does not exist: {}", p.display())
            }
            ShellIntegrationError::PermissionDenied(p) => {
                write!(f, "Permission denied: {}", p.display())
            }
            ShellIntegrationError::Io(p, e) => write!(f, "{}: {e}", p.display()),
        }
    }
}

impl std::error::Error for ShellIntegrationError {}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    /// The text of each failure, with no source: doctor prints a failed fix with `{e:#}`, which would repeat a
    /// source after the text.
    #[test]
    fn should_describe_each_integration_failure() {
        let path = Path::new("/home/u/.bashrc");
        let failures = [
            ShellIntegrationError::NotInstalled("/home/u/.bashrc does not source pre integration".into()),
            ShellIntegrationError::Improper("/home/u/.bashrc does not source pre integration first".into()),
            ShellIntegrationError::FileDoesNotExist(path.into()),
            ShellIntegrationError::PermissionDenied(path.into()),
            ShellIntegrationError::Io(path.into(), std::io::Error::other("disk full")),
        ];

        let described: Vec<(String, Option<String>)> = failures
            .iter()
            .map(|e| (e.to_string(), std::error::Error::source(e).map(ToString::to_string)))
            .collect();

        assert_eq!(
            described,
            [
                ("Integration not installed: /home/u/.bashrc does not source pre integration".to_string(), None),
                ("Improper integration installation: /home/u/.bashrc does not source pre integration first".to_string(), None),
                ("File does not exist: /home/u/.bashrc".to_string(), None),
                ("Permission denied: /home/u/.bashrc".to_string(), None),
                ("/home/u/.bashrc: disk full".to_string(), None),
            ]
        );
    }
}
