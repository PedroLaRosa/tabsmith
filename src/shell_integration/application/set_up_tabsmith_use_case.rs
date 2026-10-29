use crate::shared::domain::value_objects::shell::Shell;
use crate::shell_integration::application::integration_installer::IntegrationInstaller;
use crate::shell_integration::application::ports::spec_bundle_installer::SpecBundleInstaller;
use crate::shell_integration::application::setup_progress::SetupProgress;
use crate::shell_integration::domain::repositories::restart_reminder_repository::RestartReminderRepository;
use crate::shell_integration::domain::shell_integration_error::ShellIntegrationError;
use crate::shell_integration::domain::value_objects::integration_paths::IntegrationPaths;
use std::rc::Rc;

/// `tabsmith setup`: installs the integrations, or gives the lines to add by hand and writes the script files they
/// source, then installs the spec bundle.
pub(crate) struct SetUpTabsmithUseCase {
    installer: Rc<IntegrationInstaller>,
    restart_reminder: Rc<dyn RestartReminderRepository>,
    spec_bundle: Rc<dyn SpecBundleInstaller>,
}

impl SetUpTabsmithUseCase {
    pub(crate) fn new(
        installer: Rc<IntegrationInstaller>,
        restart_reminder: Rc<dyn RestartReminderRepository>,
        spec_bundle: Rc<dyn SpecBundleInstaller>,
    ) -> Self {
        SetUpTabsmithUseCase {
            installer,
            restart_reminder,
            spec_bundle,
        }
    }

    /// Ok(true) if the user has to finish by hand.
    pub(crate) fn execute(
        &self,
        paths: &IntegrationPaths,
        shells: &[Shell],
        _automatic: bool,
        mut report: impl FnMut(SetupProgress),
    ) -> Result<bool, ShellIntegrationError> {
        if let Ok(installed) = self.installer.install_missing(paths, shells) {
            self.restart_reminder.save_pending();
            report(SetupProgress::Installed(installed));
        }
        Ok(false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shared::application::ports::clock::FixedClock;
    use crate::shell_integration::application::ports::shell_environment::RecordingShellEnvironment;
    use crate::shell_integration::application::ports::spec_bundle_installer::RecordingSpecBundleInstaller;
    use crate::shell_integration::domain::repositories::backup_repository::InMemoryBackupRepository;
    use crate::shell_integration::domain::repositories::dotfile_repository::InMemoryDotfileRepository;
    use crate::shell_integration::domain::repositories::restart_reminder_repository::InMemoryRestartReminderRepository;
    use std::path::PathBuf;
    use std::sync::Arc;

    /// The use case over in-memory dotfiles and backups, a fixed clock and recording doubles, kept for the asserts.
    struct TestBed {
        use_case: SetUpTabsmithUseCase,
        installer: Rc<IntegrationInstaller>,
        dotfiles: Rc<InMemoryDotfileRepository>,
        restart_reminder: Rc<InMemoryRestartReminderRepository>,
        spec_bundle: Rc<RecordingSpecBundleInstaller>,
    }

    fn test_bed() -> TestBed {
        test_bed_with(
            RecordingShellEnvironment::default(),
            RecordingSpecBundleInstaller::default(),
        )
    }

    fn test_bed_with(
        environment: RecordingShellEnvironment,
        spec_bundle: RecordingSpecBundleInstaller,
    ) -> TestBed {
        let dotfiles = Rc::new(InMemoryDotfileRepository::default());
        let installer = Rc::new(IntegrationInstaller::new(
            dotfiles.clone(),
            Rc::new(InMemoryBackupRepository::default()),
            Arc::new(FixedClock::new(1_700_000_123_999)),
            Rc::new(environment),
        ));
        let restart_reminder = Rc::new(InMemoryRestartReminderRepository::default());
        let spec_bundle = Rc::new(spec_bundle);
        TestBed {
            use_case: SetUpTabsmithUseCase::new(
                installer.clone(),
                restart_reminder.clone(),
                spec_bundle.clone(),
            ),
            installer,
            dotfiles,
            restart_reminder,
            spec_bundle,
        }
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
    fn should_report_the_automatic_install_and_leave_nothing_to_do_by_hand() {
        let bed = test_bed();
        let mut events = Vec::new();

        let manual = bed
            .use_case
            .execute(&paths(), &[Shell::Zsh], true, |event| events.push(event));

        assert!(
            matches!(manual, Ok(false)) && events == [SetupProgress::Installed(true)],
            "{manual:?} {events:?}"
        );
    }

    #[test]
    fn should_save_the_restart_reminder_before_reporting_something_installed() {
        let bed = test_bed();
        let mut pending_when_reported = Vec::new();

        let _ = bed.use_case.execute(&paths(), &[Shell::Zsh], true, |event| {
            pending_when_reported.push((event, bed.restart_reminder.is_pending()))
        });

        assert_eq!(pending_when_reported, [(SetupProgress::Installed(true), true)]);
    }
}
