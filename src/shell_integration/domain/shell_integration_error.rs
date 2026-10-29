use std::path::PathBuf;

/// `is_installed()` / `install()` failure (SPEC §5.5 display strings), or a fact of the environment an integration
/// command needs that could not be found out.
#[derive(Debug)]
pub(crate) enum ShellIntegrationError {
    NotInstalled(String),
    Improper(String),
    FileDoesNotExist(PathBuf),
    PermissionDenied(PathBuf),
    Io(PathBuf, std::io::Error),
    /// The home, the tabsmith binary or pwsh's `$PROFILE` is unknown; the message already holds its causes.
    Environment(String),
    /// setup could not extract the completion specs; the message already holds its causes.
    SpecBundle(String),
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
            ShellIntegrationError::Environment(m) => f.write_str(m),
            ShellIntegrationError::SpecBundle(m) => write!(f, "could not extract the completion specs: {m}"),
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

    /// Home, binary or pwsh `$PROFILE` unknown: the message already holds its causes, so `{}` and `{:#}` print it
    /// whole and nothing follows it as a source.
    #[test]
    fn should_describe_an_environment_failure_by_its_message_alone() {
        let failure = ShellIntegrationError::Environment("could not ask pwsh for $PROFILE: timed out after 5s".into());

        let described = (failure.to_string(), format!("{failure:#}"), std::error::Error::source(&failure).is_none());

        assert_eq!(
            described,
            (
                "could not ask pwsh for $PROFILE: timed out after 5s".to_string(),
                "could not ask pwsh for $PROFILE: timed out after 5s".to_string(),
                true
            )
        );
    }

    /// setup could not extract the completion specs: the message already holds the whole cause chain, so main's
    /// `error: {:#}` prints it after the context, as it printed the old anyhow chain.
    #[test]
    fn should_describe_a_spec_bundle_failure_after_its_context() {
        let failure = ShellIntegrationError::SpecBundle(
            "creating /home/u/.tabsmith/specs: Permission denied (os error 13)".into(),
        );

        let described = (failure.to_string(), format!("{failure:#}"), std::error::Error::source(&failure).is_none());

        assert_eq!(
            described,
            (
                "could not extract the completion specs: creating /home/u/.tabsmith/specs: Permission denied (os error 13)"
                    .to_string(),
                "could not extract the completion specs: creating /home/u/.tabsmith/specs: Permission denied (os error 13)"
                    .to_string(),
                true
            )
        );
    }
}
