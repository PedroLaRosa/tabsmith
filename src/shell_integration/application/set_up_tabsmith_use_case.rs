use crate::shared::domain::value_objects::shell::Shell;
use crate::shell_integration::application::integration_installer::IntegrationInstaller;
use crate::shell_integration::application::ports::spec_bundle_installer::SpecBundleInstaller;
use crate::shell_integration::application::setup_progress::SetupProgress;
use crate::shell_integration::domain::repositories::restart_reminder_repository::RestartReminderRepository;
use crate::shell_integration::domain::services::manual_install_instructions;
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
        automatic: bool,
        mut report: impl FnMut(SetupProgress),
    ) -> Result<bool, ShellIntegrationError> {
        if automatic {
            if let Ok(installed) = self.installer.install_missing(paths, shells) {
                if installed {
                    self.restart_reminder.save_pending();
                }
                report(SetupProgress::Installed(installed));
            }
        }
        let manual = !automatic;
        if manual {
            report(SetupProgress::ManualInstructions(manual_install_instructions::text(paths, shells)));
            if let Err(failures) = self.installer.apply(
                paths,
                shells,
                |i| i.whole_file_phase().is_none(),
                |i| self.installer.write_scripts(paths, i),
            ) {
                report(SetupProgress::ScriptFilesFailed(failures));
            }
        }
        Ok(manual)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shared::application::ports::clock::FixedClock;
    use crate::shell_integration::application::integration_failure::IntegrationFailure;
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

    #[test]
    fn should_save_no_restart_reminder_when_everything_already_was_installed() {
        let bed = test_bed();
        bed.installer.install_missing(&paths(), &[Shell::Zsh]).unwrap();
        let mut events = Vec::new();

        let _ = bed.use_case.execute(&paths(), &[Shell::Zsh], true, |event| events.push(event));

        assert_eq!(
            (events, bed.restart_reminder.is_pending()),
            (vec![SetupProgress::Installed(false)], false)
        );
    }

    #[test]
    fn should_give_the_manual_instructions_when_not_asked_to_install() {
        let bed = test_bed();
        let mut events = Vec::new();

        let manual = bed.use_case.execute(&paths(), &[Shell::Bash, Shell::Zsh], false, |event| events.push(event));

        assert!(
            matches!(manual, Ok(true))
                && events
                    == [SetupProgress::ManualInstructions(manual_install_instructions::text(
                        &paths(),
                        &[Shell::Bash, Shell::Zsh]
                    ))],
            "{manual:?} {events:?}"
        );
    }

    /// The manual lines source the `~/.tabsmith/shell/` script files; fish's integrations are whole `conf.d` files,
    /// which the manual lines do not cover.
    #[test]
    fn should_write_the_script_files_of_the_dotfile_integrations_after_the_manual_instructions() {
        let bed = test_bed();
        let mut writes_when_reported = Vec::new();

        let _ = bed.use_case.execute(&paths(), &[Shell::Zsh, Shell::Fish], false, |_| {
            writes_when_reported.push(bed.dotfiles.writes().len())
        });

        assert_eq!(
            (writes_when_reported, bed.dotfiles.writes()),
            (
                vec![0],
                vec![
                    PathBuf::from("/home/u/.tabsmith/shell/zshrc.pre.zsh"),
                    PathBuf::from("/home/u/.tabsmith/shell/zshrc.post.zsh"),
                    PathBuf::from("/home/u/.tabsmith/shell/zprofile.pre.zsh"),
                    PathBuf::from("/home/u/.tabsmith/shell/zprofile.post.zsh"),
                ]
            )
        );
    }

    #[test]
    fn should_report_the_script_files_that_could_not_be_written() {
        let bed = test_bed_with(
            RecordingShellEnvironment::default().with_pwsh_profile(Err("pwsh printed an empty $PROFILE".into())),
            RecordingSpecBundleInstaller::default(),
        );
        let mut events = Vec::new();

        let manual = bed.use_case.execute(&paths(), &[Shell::Pwsh], false, |event| events.push(event));

        assert!(
            matches!(manual, Ok(true))
                && events
                    == [
                        SetupProgress::ManualInstructions(manual_install_instructions::text(&paths(), &[Shell::Pwsh])),
                        SetupProgress::ScriptFilesFailed(vec![IntegrationFailure::Listing {
                            shell: "pwsh".into(),
                            reason: "pwsh printed an empty $PROFILE".into(),
                        }]),
                    ],
            "{manual:?} {events:?}"
        );
    }
}
