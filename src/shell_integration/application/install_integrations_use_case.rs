use crate::shared::domain::value_objects::shell::Shell;
use crate::shell_integration::application::integration_failure::IntegrationFailure;
use crate::shell_integration::application::integration_installer::IntegrationInstaller;
use crate::shell_integration::domain::repositories::restart_reminder_repository::RestartReminderRepository;
use crate::shell_integration::domain::value_objects::integration_paths::IntegrationPaths;
use std::rc::Rc;

/// `tabsmith integrations install`: installs what is missing and reminds doctor that terminals opened before need a
/// restart.
pub(crate) struct InstallIntegrationsUseCase {
    installer: Rc<IntegrationInstaller>,
    restart_reminder: Rc<dyn RestartReminderRepository>,
}

impl InstallIntegrationsUseCase {
    pub(crate) fn new(
        installer: Rc<IntegrationInstaller>,
        restart_reminder: Rc<dyn RestartReminderRepository>,
    ) -> Self {
        InstallIntegrationsUseCase {
            installer,
            restart_reminder,
        }
    }

    /// Ok(true) if something was installed.
    pub(crate) fn execute(
        &self,
        paths: &IntegrationPaths,
        shells: &[Shell],
    ) -> Result<bool, Vec<IntegrationFailure>> {
        let installed = self.installer.install_missing(paths, shells);
        self.restart_reminder.save_pending();
        installed
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shared::application::ports::clock::FixedClock;
    use crate::shell_integration::application::ports::shell_environment::RecordingShellEnvironment;
    use crate::shell_integration::domain::repositories::backup_repository::InMemoryBackupRepository;
    use crate::shell_integration::domain::repositories::dotfile_repository::InMemoryDotfileRepository;
    use crate::shell_integration::domain::repositories::restart_reminder_repository::InMemoryRestartReminderRepository;
    use std::path::PathBuf;
    use std::sync::Arc;

    /// The installer over in-memory dotfiles and backups, a fixed clock and `environment`.
    fn installer(environment: RecordingShellEnvironment) -> Rc<IntegrationInstaller> {
        Rc::new(IntegrationInstaller::new(
            Rc::new(InMemoryDotfileRepository::default()),
            Rc::new(InMemoryBackupRepository::default()),
            Arc::new(FixedClock::new(1_700_000_123_999)),
            Rc::new(environment),
        ))
    }

    /// No pwsh `$PROFILE` known, so listing pwsh asks the environment.
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

    #[test]
    fn should_save_the_restart_reminder_when_something_was_installed() {
        let restart_reminder = Rc::new(InMemoryRestartReminderRepository::default());
        let use_case = InstallIntegrationsUseCase::new(
            installer(RecordingShellEnvironment::default()),
            restart_reminder.clone(),
        );

        let installed = use_case.execute(&paths(), &[Shell::Zsh]);

        assert!(
            matches!(installed, Ok(true)) && restart_reminder.is_pending(),
            "{installed:?}"
        );
    }
}
