use crate::shared::domain::value_objects::data_directory::DataDirectory;
use crate::shared::infrastructure::shared_dependencies::SharedDependencies;
use crate::shell_integration::application::decide_launch_use_case::DecideLaunchUseCase;
use crate::shell_integration::application::get_integration_status_use_case::GetIntegrationStatusUseCase;
use crate::shell_integration::application::install_integrations_use_case::InstallIntegrationsUseCase;
use crate::shell_integration::application::integration_installer::IntegrationInstaller;
use crate::shell_integration::application::ports::spec_bundle_installer::SpecBundleInstaller;
use crate::shell_integration::application::reinstall_integrations_use_case::ReinstallIntegrationsUseCase;
use crate::shell_integration::application::render_init_script_use_case::RenderInitScriptUseCase;
use crate::shell_integration::application::set_up_tabsmith_use_case::SetUpTabsmithUseCase;
use crate::shell_integration::application::uninstall_integrations_use_case::UninstallIntegrationsUseCase;
use crate::shell_integration::application::uninstall_tabsmith_use_case::UninstallTabsmithUseCase;
use crate::shell_integration::domain::repositories::restart_reminder_repository::RestartReminderRepository;
use crate::shell_integration::domain::shell_integration_error::ShellIntegrationError;
use crate::shell_integration::domain::value_objects::integration_paths::IntegrationPaths;
use crate::shell_integration::infrastructure::adapters::embedded_shell_scripts::EmbeddedShellScripts;
use crate::shell_integration::infrastructure::adapters::file_backup_repository::FileBackupRepository;
use crate::shell_integration::infrastructure::adapters::file_data_directory_purger::FileDataDirectoryPurger;
use crate::shell_integration::infrastructure::adapters::file_dotfile_repository::FileDotfileRepository;
use crate::shell_integration::infrastructure::adapters::os_shell_environment::{
    integration_paths_from, OsShellEnvironment,
};
use crate::shell_integration::infrastructure::adapters::state_restart_reminder_repository::StateRestartReminderRepository;
use std::rc::Rc;

/// The shell integration use cases over the files on disk, the running system, the embedded shell scripts and the
/// restart reminder in `state.json`.
pub(crate) struct ShellIntegrationModule {
    data_directory: DataDirectory,
    pub(crate) install_integrations: InstallIntegrationsUseCase,
    pub(crate) uninstall_integrations: UninstallIntegrationsUseCase,
    pub(crate) reinstall_integrations: ReinstallIntegrationsUseCase,
    pub(crate) get_integration_status: GetIntegrationStatusUseCase,
    pub(crate) render_init_script: RenderInitScriptUseCase,
    pub(crate) decide_launch: DecideLaunchUseCase,
    pub(crate) set_up_tabsmith: SetUpTabsmithUseCase,
    pub(crate) uninstall_tabsmith: UninstallTabsmithUseCase,
}

impl ShellIntegrationModule {
    /// Where an integration command reads and writes: the home, then the data root, then the tabsmith binary, or the
    /// first of them that is unknown. Each command locates them at the moment it first needs them.
    pub(crate) fn locate_integration_paths(&self) -> Result<IntegrationPaths, ShellIntegrationError> {
        integration_paths_from(&self.data_directory)
    }
}

/// Only builds: it reads no env, locates no executable, looks nothing up on PATH and runs no `pwsh`, so commands that
/// need none of that keep working without them.
pub(crate) fn build_shell_integration_module(
    dependencies: &SharedDependencies,
    spec_bundle: Rc<dyn SpecBundleInstaller>,
) -> ShellIntegrationModule {
    let data_directory = dependencies.data_directory.clone();
    let environment = Rc::new(OsShellEnvironment);
    let installer = Rc::new(IntegrationInstaller::new(
        Rc::new(FileDotfileRepository),
        Rc::new(FileBackupRepository),
        dependencies.clock.clone(),
        environment.clone(),
    ));
    let restart_reminder: Rc<dyn RestartReminderRepository> =
        Rc::new(StateRestartReminderRepository::new(data_directory.clone()));
    ShellIntegrationModule {
        install_integrations: InstallIntegrationsUseCase::new(installer.clone(), restart_reminder.clone()),
        uninstall_integrations: UninstallIntegrationsUseCase::new(installer.clone()),
        reinstall_integrations: ReinstallIntegrationsUseCase::new(installer.clone(), restart_reminder.clone()),
        get_integration_status: GetIntegrationStatusUseCase::new(installer.clone()),
        render_init_script: RenderInitScriptUseCase::new(environment.clone(), Rc::new(EmbeddedShellScripts)),
        decide_launch: DecideLaunchUseCase::new(environment),
        set_up_tabsmith: SetUpTabsmithUseCase::new(installer.clone(), restart_reminder, spec_bundle),
        uninstall_tabsmith: UninstallTabsmithUseCase::new(installer, Rc::new(FileDataDirectoryPurger)),
        data_directory,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shared::application::ports::clock::FixedClock;
    use crate::shell_integration::application::ports::spec_bundle_installer::RecordingSpecBundleInstaller;
    use std::path::Path;
    use std::sync::Arc;

    /// The script files, backups and purge go under the data directory the module was built with, never one the
    /// env names (T36).
    #[test]
    fn should_locate_the_integration_paths_under_the_data_directory_it_was_built_with() {
        let module = build_shell_integration_module(
            &SharedDependencies {
                data_directory: DataDirectory::new(Some("/nowhere/tabsmith-data".into()), None),
                clock: Arc::new(FixedClock::new(0)),
            },
            Rc::new(RecordingSpecBundleInstaller::default()),
        );

        let paths = module.locate_integration_paths().unwrap();

        assert_eq!(paths.data_root(), Path::new("/nowhere/tabsmith-data"));
    }
}
