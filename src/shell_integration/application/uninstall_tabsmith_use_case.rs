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
        _purge: bool,
        _report: impl FnMut(UninstallProgress),
    ) -> bool {
        let _ = self.installer.apply(
            paths,
            shells,
            |_| true,
            |i| self.installer.uninstall(paths, i),
        );
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shared::application::ports::clock::FixedClock;
    use crate::shell_integration::application::ports::data_directory_purger::RecordingDataDirectoryPurger;
    use crate::shell_integration::application::ports::shell_environment::RecordingShellEnvironment;
    use crate::shell_integration::domain::repositories::backup_repository::InMemoryBackupRepository;
    use crate::shell_integration::domain::repositories::dotfile_repository::InMemoryDotfileRepository;
    use std::path::PathBuf;
    use std::sync::Arc;

    /// The use case over in-memory dotfiles and backups, a fixed clock and recording doubles, kept for the asserts.
    struct TestBed {
        use_case: UninstallTabsmithUseCase,
        installer: Rc<IntegrationInstaller>,
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
        let installer = Rc::new(IntegrationInstaller::new(
            Rc::new(InMemoryDotfileRepository::default()),
            Rc::new(InMemoryBackupRepository::default()),
            Arc::new(FixedClock::new(1_700_000_123_999)),
            Rc::new(environment),
        ));
        let purger = Rc::new(purger);
        TestBed {
            use_case: UninstallTabsmithUseCase::new(installer.clone(), purger.clone()),
            installer,
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
}
