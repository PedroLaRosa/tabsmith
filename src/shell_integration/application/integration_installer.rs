use crate::shared::application::ports::clock::Clock;
use crate::shared::domain::value_objects::shell::Shell;
use crate::shell_integration::application::ports::shell_environment::ShellEnvironment;
use crate::shell_integration::domain::entities::integration::Integration;
use crate::shell_integration::domain::repositories::backup_repository::BackupRepository;
use crate::shell_integration::domain::repositories::dotfile_repository::DotfileRepository;
use crate::shell_integration::domain::services::integration_catalog;
use crate::shell_integration::domain::shell_integration_error::ShellIntegrationError;
use crate::shell_integration::domain::value_objects::integration_paths::IntegrationPaths;
use std::rc::Rc;
use std::sync::Arc;

/// Lists, checks, installs and uninstalls the integrations of a shell through the dotfile and backup repositories
/// (SPEC §5.5), and runs one of those operations over several shells. Shared by the use cases; never marks the
/// restart reminder.
pub(crate) struct IntegrationInstaller {
    dotfiles: Rc<dyn DotfileRepository>,
    backups: Rc<dyn BackupRepository>,
    clock: Arc<dyn Clock + Send + Sync>,
    environment: Rc<dyn ShellEnvironment>,
}

impl IntegrationInstaller {
    pub(crate) fn new(
        dotfiles: Rc<dyn DotfileRepository>,
        backups: Rc<dyn BackupRepository>,
        clock: Arc<dyn Clock + Send + Sync>,
        environment: Rc<dyn ShellEnvironment>,
    ) -> Self {
        IntegrationInstaller {
            dotfiles,
            backups,
            clock,
            environment,
        }
    }

    /// Integrations of `shell` (SPEC §5.5 table), asking `pwsh` for `$PROFILE` when the paths do not know it.
    pub(crate) fn integrations(
        &self,
        paths: &IntegrationPaths,
        shell: Shell,
    ) -> Result<Vec<Integration>, ShellIntegrationError> {
        integration_catalog::integrations(
            paths,
            shell,
            |p| self.dotfiles.exists(p),
            || self.environment.pwsh_profile(),
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
    use std::path::{Path, PathBuf};

    /// The installer over in-memory dotfiles and backups, a clock fixed at 1_700_000_123_999 ms and a recording
    /// shell environment, with the doubles kept for the asserts.
    struct TestBed {
        installer: IntegrationInstaller,
        dotfiles: Rc<InMemoryDotfileRepository>,
    }

    fn test_bed() -> TestBed {
        let dotfiles = Rc::new(InMemoryDotfileRepository::default());
        let installer = IntegrationInstaller::new(
            dotfiles.clone(),
            Rc::new(InMemoryBackupRepository::default()),
            Arc::new(FixedClock::new(1_700_000_123_999)),
            Rc::new(RecordingShellEnvironment::default()),
        );
        TestBed { installer, dotfiles }
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
    fn should_list_the_bash_login_files_that_exist_after_bashrc() {
        let bed = test_bed();
        bed.dotfiles.write(Path::new("/home/u/.bash_profile"), "").unwrap();

        let listed = bed.installer.integrations(&paths(), Shell::Bash).unwrap();

        assert_eq!(
            listed.iter().map(Integration::path).collect::<Vec<_>>(),
            [Path::new("/home/u/.bashrc"), Path::new("/home/u/.bash_profile")]
        );
    }
}
