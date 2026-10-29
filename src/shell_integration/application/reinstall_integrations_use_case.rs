use crate::shared::domain::value_objects::shell::Shell;
use crate::shell_integration::application::integration_failure::IntegrationFailure;
use crate::shell_integration::application::integration_installer::IntegrationInstaller;
use crate::shell_integration::domain::repositories::restart_reminder_repository::RestartReminderRepository;
use crate::shell_integration::domain::value_objects::integration_paths::IntegrationPaths;
use std::rc::Rc;

/// `tabsmith integrations reinstall`: takes the integrations of the shells out, then installs them again.
pub(crate) struct ReinstallIntegrationsUseCase {
    installer: Rc<IntegrationInstaller>,
    restart_reminder: Rc<dyn RestartReminderRepository>,
}

impl ReinstallIntegrationsUseCase {
    pub(crate) fn new(
        installer: Rc<IntegrationInstaller>,
        restart_reminder: Rc<dyn RestartReminderRepository>,
    ) -> Self {
        ReinstallIntegrationsUseCase {
            installer,
            restart_reminder,
        }
    }

    /// Ok(true) if something was installed again. A failure to take the integrations out stops the run before the
    /// install.
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
        )?;
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

    /// The installer over `dotfiles`, in-memory backups, a fixed clock and `environment`.
    fn installer(
        dotfiles: Rc<InMemoryDotfileRepository>,
        environment: Rc<RecordingShellEnvironment>,
    ) -> Rc<IntegrationInstaller> {
        Rc::new(IntegrationInstaller::new(
            dotfiles,
            Rc::new(InMemoryBackupRepository::default()),
            Arc::new(FixedClock::new(1_700_000_123_999)),
            environment,
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

    /// Already installed, so only the uninstall half makes the install half install anything.
    #[test]
    fn should_install_again_what_was_installed_and_save_the_restart_reminder() {
        let installer = installer(
            Rc::new(InMemoryDotfileRepository::default()),
            Rc::new(RecordingShellEnvironment::default()),
        );
        installer.install_missing(&paths(), &[Shell::Zsh]).unwrap();
        let restart_reminder = Rc::new(InMemoryRestartReminderRepository::default());
        let use_case = ReinstallIntegrationsUseCase::new(installer.clone(), restart_reminder.clone());

        let reinstalled = use_case.execute(&paths(), &[Shell::Zsh]);

        let installed = installer
            .integrations(&paths(), Shell::Zsh)
            .unwrap()
            .iter()
            .all(|integration| installer.check(&paths(), integration).is_ok());
        assert_eq!(
            (reinstalled.unwrap(), installed, restart_reminder.is_pending()),
            (true, true, true)
        );
    }

    /// pwsh cannot be listed, so the uninstall half fails: nothing is installed and no reminder is saved (T30).
    #[test]
    fn should_stop_before_installing_when_the_uninstall_half_fails() {
        let dotfiles = Rc::new(InMemoryDotfileRepository::default());
        let environment = Rc::new(
            RecordingShellEnvironment::default().with_pwsh_profile(Err("pwsh printed an empty $PROFILE".into())),
        );
        let restart_reminder = Rc::new(InMemoryRestartReminderRepository::default());
        let use_case = ReinstallIntegrationsUseCase::new(
            installer(dotfiles.clone(), environment.clone()),
            restart_reminder.clone(),
        );

        let reinstalled = use_case.execute(&paths(), &[Shell::Zsh, Shell::Pwsh]);

        assert!(
            reinstalled.is_err()
                && dotfiles.writes().is_empty()
                && environment.asked() == ["pwsh_profile"]
                && !restart_reminder.is_pending(),
            "{reinstalled:?} {:?} {:?}",
            dotfiles.writes(),
            environment.asked()
        );
    }
}
