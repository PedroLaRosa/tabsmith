use crate::shared::application::ports::clock::Clock;
use crate::shared::domain::value_objects::shell::Shell;
use crate::shell_integration::application::ports::shell_environment::ShellEnvironment;
use crate::shell_integration::domain::entities::integration::Integration;
use crate::shell_integration::domain::repositories::backup_repository::BackupRepository;
use crate::shell_integration::domain::repositories::dotfile_repository::DotfileRepository;
use crate::shell_integration::domain::services::integration_catalog;
use crate::shell_integration::domain::shell_integration_error::ShellIntegrationError;
use crate::shell_integration::domain::value_objects::dotfile::Dotfile;
use crate::shell_integration::domain::value_objects::integration_paths::IntegrationPaths;
use crate::shell_integration::domain::value_objects::integration_phase::IntegrationPhase;
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

    /// SPEC §5.5 `is_installed()`: the first failing reason.
    pub(crate) fn check(
        &self,
        paths: &IntegrationPaths,
        integration: &Integration,
    ) -> Result<(), ShellIntegrationError> {
        if let Some(script) = integration.whole_file_script(paths) {
            return script.check(&self.dotfiles.read(script.path())?);
        }
        let dotfile = Dotfile::new(self.dotfiles.read(integration.path())?);
        if dotfile.is_opted_out() {
            return Ok(());
        }
        let lines = dotfile.sourced_lines();
        for (when, at, actual) in [
            (IntegrationPhase::Pre, "first", lines.first()),
            (IntegrationPhase::Post, "last", lines.last()),
        ] {
            let src = integration.source_line(paths, when);
            if actual.copied() != Some(src.as_str()) {
                let msg = format!(
                    "{} does not source {} integration",
                    integration.path().display(),
                    when.name()
                );
                return Err(if lines.contains(&src.as_str()) {
                    ShellIntegrationError::Improper(format!("{msg} {at}"))
                } else {
                    ShellIntegrationError::NotInstalled(msg)
                });
            }
            let script = integration.script(paths, when);
            script.check(&self.dotfiles.read(script.path())?)?;
        }
        Ok(())
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
        environment: Rc<RecordingShellEnvironment>,
    }

    fn test_bed() -> TestBed {
        test_bed_with(RecordingShellEnvironment::default())
    }

    fn test_bed_with(environment: RecordingShellEnvironment) -> TestBed {
        let dotfiles = Rc::new(InMemoryDotfileRepository::default());
        let environment = Rc::new(environment);
        let installer = IntegrationInstaller::new(
            dotfiles.clone(),
            Rc::new(InMemoryBackupRepository::default()),
            Arc::new(FixedClock::new(1_700_000_123_999)),
            environment.clone(),
        );
        TestBed {
            installer,
            dotfiles,
            environment,
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
    fn should_list_the_bash_login_files_that_exist_after_bashrc() {
        let bed = test_bed();
        bed.dotfiles.write(Path::new("/home/u/.bash_profile"), "").unwrap();

        let listed = bed.installer.integrations(&paths(), Shell::Bash).unwrap();

        assert_eq!(
            listed.iter().map(Integration::path).collect::<Vec<_>>(),
            [Path::new("/home/u/.bashrc"), Path::new("/home/u/.bash_profile")]
        );
    }

    /// Never cached: every listing of pwsh asks again, and no other shell asks (T26).
    #[test]
    fn should_ask_pwsh_for_its_profile_on_each_listing_of_pwsh_and_for_no_other_shell() {
        let bed = test_bed();

        for shell in [Shell::Bash, Shell::Pwsh, Shell::Zsh, Shell::Fish, Shell::Pwsh] {
            bed.installer.integrations(&paths(), shell).unwrap();
        }

        assert_eq!(bed.environment.asked(), ["pwsh_profile", "pwsh_profile"]);
    }

    #[test]
    fn should_list_the_profile_pwsh_gives_as_its_one_dotfile() {
        let bed = test_bed_with(
            RecordingShellEnvironment::default().with_pwsh_profile(Ok(PathBuf::from("/home/u/pwsh/profile.ps1"))),
        );

        let listed = bed.installer.integrations(&paths(), Shell::Pwsh).unwrap();

        assert_eq!(
            listed.iter().map(Integration::path).collect::<Vec<_>>(),
            [Path::new("/home/u/pwsh/profile.ps1")]
        );
    }

    #[test]
    fn should_not_ask_pwsh_for_the_profile_the_paths_know() {
        let bed = test_bed();
        let paths = paths().with_pwsh_profile(PathBuf::from("/home/u/known.ps1"));

        let listed = bed.installer.integrations(&paths, Shell::Pwsh).unwrap();

        assert_eq!(
            (listed[0].path(), bed.environment.asked()),
            (Path::new("/home/u/known.ps1"), vec![])
        );
    }

    #[test]
    fn should_fail_to_list_pwsh_when_pwsh_cannot_give_its_profile() {
        let bed = test_bed_with(
            RecordingShellEnvironment::default().with_pwsh_profile(Err("pwsh printed an empty $PROFILE".into())),
        );

        let error = bed.installer.integrations(&paths(), Shell::Pwsh).unwrap_err();

        assert_eq!(error.to_string(), "pwsh printed an empty $PROFILE");
    }

    #[test]
    fn should_find_a_dotfile_that_does_not_source_the_pre_script_not_installed() {
        let bed = test_bed();
        let zshrc = Integration::dotfile(Shell::Zsh, PathBuf::from("/home/u/.zshrc"));
        bed.dotfiles.write(zshrc.path(), "export A=1\n").unwrap();

        let checked = bed.installer.check(&paths(), &zshrc);

        assert!(
            matches!(&checked, Err(ShellIntegrationError::NotInstalled(m)) if m == "/home/u/.zshrc does not source pre integration"),
            "{checked:?}"
        );
    }
}
