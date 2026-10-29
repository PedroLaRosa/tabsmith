use crate::shared::domain::value_objects::shell::Shell;
use crate::shell_integration::application::integration_installer::IntegrationInstaller;
use crate::shell_integration::application::ports::data_directory_purger::DataDirectoryPurger;
use crate::shell_integration::application::uninstall_progress::UninstallProgress;
use crate::shell_integration::domain::value_objects::integration_paths::IntegrationPaths;
use std::rc::Rc;

/// `tabsmith uninstall`: takes out every integration, carrying on past failures, and on purge deletes the data
/// directory (SPEC §2.9).
pub(crate) struct UninstallTabsmithUseCase {
    installer: Rc<IntegrationInstaller>,
    purger: Rc<dyn DataDirectoryPurger>,
}

impl UninstallTabsmithUseCase {
    pub(crate) fn new(
        installer: Rc<IntegrationInstaller>,
        purger: Rc<dyn DataDirectoryPurger>,
    ) -> Self {
        UninstallTabsmithUseCase { installer, purger }
    }

    /// true if anything failed.
    pub(crate) fn execute(
        &self,
        paths: &IntegrationPaths,
        shells: &[Shell],
        purge: bool,
        mut report: impl FnMut(UninstallProgress),
    ) -> bool {
        let integrations_failed = self.uninstall_integrations(paths, shells, &mut report);
        let purge_failed = purge && self.purge_data_root(paths, &mut report);
        integrations_failed || purge_failed
    }

    /// true if not every integration could be taken out.
    fn uninstall_integrations(
        &self,
        paths: &IntegrationPaths,
        shells: &[Shell],
        report: &mut impl FnMut(UninstallProgress),
    ) -> bool {
        let uninstalled = self.installer.apply(
            paths,
            shells,
            |_| true,
            |i| self.installer.uninstall(paths, i),
        );
        match uninstalled {
            Ok(_) => false,
            Err(failures) => {
                report(UninstallProgress::IntegrationsFailed(failures));
                true
            }
        }
    }

    /// true if the data root could not be deleted.
    fn purge_data_root(
        &self,
        paths: &IntegrationPaths,
        report: &mut impl FnMut(UninstallProgress),
    ) -> bool {
        match self.purger.purge(paths.data_root()) {
            Ok(()) => false,
            Err(error) => {
                report(UninstallProgress::PurgeFailed(error.to_string()));
                true
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shared::application::ports::clock::FixedClock;
    use crate::shell_integration::application::integration_failure::IntegrationFailure;
    use crate::shell_integration::application::ports::data_directory_purger::RecordingDataDirectoryPurger;
    use crate::shell_integration::application::ports::shell_environment::RecordingShellEnvironment;
    use crate::shell_integration::domain::repositories::backup_repository::InMemoryBackupRepository;
    use crate::shell_integration::domain::repositories::dotfile_repository::DotfileRepository;
    use crate::shell_integration::domain::repositories::dotfile_repository::InMemoryDotfileRepository;
    use std::path::{Path, PathBuf};
    use std::sync::Arc;

    /// The use case over in-memory dotfiles and backups, a fixed clock and recording doubles, kept for the asserts.
    struct TestBed {
        use_case: UninstallTabsmithUseCase,
        installer: Rc<IntegrationInstaller>,
        dotfiles: Rc<InMemoryDotfileRepository>,
        purger: Rc<RecordingDataDirectoryPurger>,
    }

    fn test_bed() -> TestBed {
        test_bed_with(
            RecordingShellEnvironment::default(),
            RecordingDataDirectoryPurger::default(),
        )
    }

    fn test_bed_with(
        environment: RecordingShellEnvironment,
        purger: RecordingDataDirectoryPurger,
    ) -> TestBed {
        let dotfiles = Rc::new(InMemoryDotfileRepository::default());
        let installer = Rc::new(IntegrationInstaller::new(
            dotfiles.clone(),
            Rc::new(InMemoryBackupRepository::default()),
            Arc::new(FixedClock::new(1_700_000_123_999)),
            Rc::new(environment),
        ));
        let purger = Rc::new(purger);
        TestBed {
            use_case: UninstallTabsmithUseCase::new(installer.clone(), purger.clone()),
            installer,
            dotfiles,
            purger,
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

    fn has_traces(installer: &IntegrationInstaller, shell: Shell) -> bool {
        installer
            .integrations(&paths(), shell)
            .unwrap()
            .iter()
            .any(|i| installer.has_traces(&paths(), i))
    }

    #[test]
    fn should_take_out_every_integration_and_purge_nothing_unless_asked() {
        let bed = test_bed();
        bed.installer
            .install_missing(&paths(), &[Shell::Zsh, Shell::Fish])
            .unwrap();
        let mut events = Vec::new();

        let failed = bed
            .use_case
            .execute(&paths(), &[Shell::Zsh, Shell::Fish], false, |event| {
                events.push(event)
            });

        assert_eq!(
            (
                failed,
                events,
                [Shell::Zsh, Shell::Fish].map(|shell| has_traces(&bed.installer, shell)),
                bed.purger.purged()
            ),
            (false, vec![], [false, false], vec![])
        );
    }

    #[test]
    fn should_report_the_integrations_that_could_not_be_taken_out_and_fail() {
        let bed = test_bed_with(
            RecordingShellEnvironment::default()
                .with_pwsh_profile(Err("pwsh printed an empty $PROFILE".into())),
            RecordingDataDirectoryPurger::default(),
        );
        let mut events = Vec::new();

        let failed = bed
            .use_case
            .execute(&paths(), &[Shell::Pwsh], false, |event| events.push(event));

        assert_eq!(
            (failed, events),
            (
                true,
                vec![UninstallProgress::IntegrationsFailed(vec![
                    IntegrationFailure::Listing {
                        shell: "pwsh".into(),
                        reason: "pwsh printed an empty $PROFILE".into(),
                    }
                ])]
            )
        );
    }

    /// The data root comes from the paths, never from the environment (T36).
    #[test]
    fn should_purge_the_data_root_of_the_paths_when_asked() {
        let bed = test_bed();
        let mut events = Vec::new();

        let failed = bed
            .use_case
            .execute(&paths(), &[Shell::Zsh], true, |event| events.push(event));

        assert_eq!(
            (failed, events, bed.purger.purged()),
            (false, vec![], vec![PathBuf::from("/home/u/.tabsmith")])
        );
    }

    /// The CLI prints `Could not remove <data>: <reason>`, with the io error's own text as the old `{e}` (T34).
    #[test]
    fn should_report_a_failed_purge_by_the_io_error_text_and_fail() {
        let bed = test_bed_with(
            RecordingShellEnvironment::default(),
            RecordingDataDirectoryPurger::failing("Permission denied (os error 13)"),
        );
        let mut events = Vec::new();

        let failed = bed
            .use_case
            .execute(&paths(), &[Shell::Zsh], true, |event| events.push(event));

        assert_eq!(
            (failed, events),
            (
                true,
                vec![UninstallProgress::PurgeFailed(
                    "Permission denied (os error 13)".into()
                )]
            )
        );
    }

    #[test]
    fn should_report_the_integration_failures_before_purging() {
        let bed = test_bed_with(
            RecordingShellEnvironment::default()
                .with_pwsh_profile(Err("pwsh printed an empty $PROFILE".into())),
            RecordingDataDirectoryPurger::default(),
        );
        let mut purges_when_reported = Vec::new();

        let failed = bed.use_case.execute(&paths(), &[Shell::Pwsh], true, |_| {
            purges_when_reported.push(bed.purger.purged().len())
        });

        assert_eq!(
            (failed, purges_when_reported, bed.purger.purged()),
            (true, vec![0], vec![PathBuf::from("/home/u/.tabsmith")])
        );
    }

    /// Unlike `integrations uninstall`, which takes out only what has traces, every integration is uninstalled: a
    /// dotfile tabsmith never touched is still rewritten by the whitespace normalisation (T34).
    #[test]
    fn should_take_out_every_integration_even_one_without_traces() {
        let bed = test_bed();
        let zshrc = Path::new("/home/u/.zshrc");
        bed.dotfiles.write(zshrc, "").unwrap();

        let _ = bed.use_case.execute(&paths(), &[Shell::Zsh], false, |_| {});

        assert_eq!(bed.dotfiles.read(zshrc).unwrap(), "\n");
    }
}
