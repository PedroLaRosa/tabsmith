use crate::shared::domain::value_objects::shell::Shell;
use crate::shell_integration::application::integration_installer::IntegrationInstaller;
use crate::shell_integration::application::integration_status::IntegrationStatus;
use crate::shell_integration::application::integration_status_report::IntegrationStatusReport;
use crate::shell_integration::domain::value_objects::integration_paths::IntegrationPaths;
use std::rc::Rc;

/// `tabsmith integrations status`: whether each integration of the shells is installed.
pub(crate) struct GetIntegrationStatusUseCase {
    installer: Rc<IntegrationInstaller>,
}

impl GetIntegrationStatusUseCase {
    pub(crate) fn new(installer: Rc<IntegrationInstaller>) -> Self {
        GetIntegrationStatusUseCase { installer }
    }

    /// Lists a shell's integrations, then checks each, shell by shell.
    pub(crate) fn execute(&self, paths: &IntegrationPaths, shells: &[Shell]) -> IntegrationStatusReport {
        let mut integrations = Vec::new();
        for &shell in shells {
            if let Ok(list) = self.installer.integrations(paths, shell) {
                integrations.extend(list.iter().map(|i| IntegrationStatus {
                    installed: self.installer.check(paths, i).is_ok(),
                    description: i.describe(),
                    shell: i.shell().name().to_string(),
                    file_name: i.file_name(),
                }));
            }
        }
        IntegrationStatusReport {
            integrations,
            failures: Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shared::application::ports::clock::FixedClock;
    use crate::shell_integration::application::ports::shell_environment::RecordingShellEnvironment;
    use crate::shell_integration::domain::entities::integration::Integration;
    use crate::shell_integration::domain::repositories::backup_repository::InMemoryBackupRepository;
    use crate::shell_integration::domain::repositories::dotfile_repository::InMemoryDotfileRepository;
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

    fn status(installed: bool, description: &str, shell: &str, file_name: &str) -> IntegrationStatus {
        IntegrationStatus {
            installed,
            description: description.to_string(),
            shell: shell.to_string(),
            file_name: file_name.to_string(),
        }
    }

    #[test]
    fn should_give_a_row_per_integration_shell_by_shell_with_whether_it_is_installed() {
        let installer = installer(RecordingShellEnvironment::default());
        let zshrc = Integration::dotfile(Shell::Zsh, PathBuf::from("/home/u/.zshrc"));
        installer.install(&paths(), &zshrc).unwrap();
        let use_case = GetIntegrationStatusUseCase::new(installer);

        let report = use_case.execute(&paths(), &[Shell::Zsh, Shell::Fish]);

        assert_eq!(
            (report.integrations, report.failures.len()),
            (
                vec![
                    status(true, "zsh pre post into .zshrc", "zsh", ".zshrc"),
                    status(false, "zsh pre post into .zprofile", "zsh", ".zprofile"),
                    status(false, "fish pre", "fish", "00_tabsmith_pre.fish"),
                    status(false, "fish post", "fish", "99_tabsmith_post.fish"),
                ],
                0
            )
        );
    }
}
