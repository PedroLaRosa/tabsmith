use crate::shared::domain::value_objects::shell::Shell;
use crate::shell_integration::application::integration_failure::IntegrationFailure;
use crate::shell_integration::application::integration_installer::IntegrationInstaller;
use crate::shell_integration::domain::value_objects::integration_paths::IntegrationPaths;
use std::rc::Rc;

/// `tabsmith integrations uninstall`: takes the integrations of the shells out of the dotfiles.
pub(crate) struct UninstallIntegrationsUseCase {
    installer: Rc<IntegrationInstaller>,
}

impl UninstallIntegrationsUseCase {
    pub(crate) fn new(installer: Rc<IntegrationInstaller>) -> Self {
        UninstallIntegrationsUseCase { installer }
    }

    /// Uninstalls every integration with something on disk, not only the installed ones: a moved binary or lines
    /// appended after the post block fail the check exactly when an uninstall is needed. Ok(true) if any.
    pub(crate) fn execute(
        &self,
        paths: &IntegrationPaths,
        shells: &[Shell],
    ) -> Result<bool, Vec<IntegrationFailure>> {
        self.installer.apply(
            paths,
            shells,
            |i| self.installer.has_traces(paths, i),
            |i| self.installer.uninstall(paths, i),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shared::application::ports::clock::FixedClock;
    use crate::shell_integration::application::ports::shell_environment::RecordingShellEnvironment;
    use crate::shell_integration::domain::repositories::backup_repository::InMemoryBackupRepository;
    use crate::shell_integration::domain::repositories::dotfile_repository::InMemoryDotfileRepository;
    use std::path::PathBuf;
    use std::sync::Arc;

    /// The installer over `dotfiles`, in-memory backups, a fixed clock and a recording shell environment.
    fn installer(dotfiles: Rc<InMemoryDotfileRepository>) -> Rc<IntegrationInstaller> {
        Rc::new(IntegrationInstaller::new(
            dotfiles,
            Rc::new(InMemoryBackupRepository::default()),
            Arc::new(FixedClock::new(1_700_000_123_999)),
            Rc::new(RecordingShellEnvironment::default()),
        ))
    }

    fn paths() -> IntegrationPaths {
        IntegrationPaths::new(
            PathBuf::from("/home/u"),
            PathBuf::from("/home/u/.tabsmith"),
            "/usr/local/bin/tabsmith".into(),
            None,
            PathBuf::from("/home/u/.config/fish"),
            None,
        )
    }

    /// Whether anything of an integration of `shell` is still on disk.
    fn has_traces(installer: &IntegrationInstaller, shell: Shell) -> bool {
        installer
            .integrations(&paths(), shell)
            .unwrap()
            .iter()
            .any(|integration| installer.has_traces(&paths(), integration))
    }

    #[test]
    fn should_take_out_every_installed_integration_of_the_shells() {
        let installer = installer(Rc::new(InMemoryDotfileRepository::default()));
        installer.install_missing(&paths(), &[Shell::Zsh, Shell::Fish]).unwrap();
        let use_case = UninstallIntegrationsUseCase::new(installer.clone());

        let uninstalled = use_case.execute(&paths(), &[Shell::Zsh, Shell::Fish]);

        assert_eq!(
            (
                uninstalled.unwrap(),
                has_traces(&installer, Shell::Zsh),
                has_traces(&installer, Shell::Fish)
            ),
            (true, false, false)
        );
    }

    /// What `integrations uninstall` reports as `Not installed` (T30).
    #[test]
    fn should_report_nothing_uninstalled_when_no_integration_left_traces() {
        let use_case =
            UninstallIntegrationsUseCase::new(installer(Rc::new(InMemoryDotfileRepository::default())));

        let uninstalled = use_case.execute(&paths(), &[Shell::Bash, Shell::Zsh, Shell::Fish]);

        assert!(matches!(uninstalled, Ok(false)), "{uninstalled:?}");
    }
}
