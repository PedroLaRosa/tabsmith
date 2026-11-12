use crate::modules::diagnostics::infrastructure::diagnostics_module::DiagnosticsModule;

/// `debug logs`: the two path lines are printed before the pty log is read, which may be huge or not a regular file.
pub(crate) fn run_debug_logs_command(module: &DiagnosticsModule) -> anyhow::Result<i32> {
    let paths = module.show_recent_logs.paths()?;
    println!("{}\n{}", paths.cli_log().display(), paths.pty_log().display());
    if let Some(lines) = module.show_recent_logs.pty_tail(&paths) {
        println!();
        lines.iter().for_each(|line| println!("{line}"));
    }
    Ok(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::diagnostics::application::ports::bundled_specs_probe::InMemoryBundledSpecsProbe;
    use crate::modules::diagnostics::application::ports::check_fixer::RecordingCheckFixer;
    use crate::modules::diagnostics::application::ports::integration_probe::InMemoryIntegrationProbe;
    use crate::modules::diagnostics::infrastructure::diagnostics_module::build_diagnostics_module;
    use crate::modules::settings::domain::repositories::settings_repository::InMemorySettingsRepository;
    use crate::modules::shared::application::ports::clock::FixedClock;
    use crate::modules::shared::domain::value_objects::data_directory::DataDirectory;
    use crate::modules::shared::infrastructure::shared_dependencies::SharedDependencies;
    use crate::modules::shell_integration::domain::repositories::restart_reminder_repository::InMemoryRestartReminderRepository;
    use std::rc::Rc;
    use std::sync::Arc;

    fn module_over(data_directory: DataDirectory) -> DiagnosticsModule {
        build_diagnostics_module(
            &SharedDependencies { data_directory, clock: Arc::new(FixedClock::new(0)) },
            Rc::new(InMemorySettingsRepository::default()),
            Rc::new(InMemoryRestartReminderRepository::default()),
            Rc::new(InMemoryIntegrationProbe::default()),
            Rc::new(InMemoryBundledSpecsProbe::extracted(true)),
            Rc::new(RecordingCheckFixer::succeeding()),
        )
    }

    #[test]
    fn should_fail_with_the_home_error_when_the_data_directory_is_unknown() {
        let module = module_over(DataDirectory::new(None, None));

        let error = run_debug_logs_command(&module).unwrap_err();

        assert_eq!(
            error.to_string(),
            "could not determine the home directory (set HOME or TABSMITH_HOME)"
        );
    }

    #[test]
    fn should_succeed_when_the_pty_log_does_not_exist() {
        let module = module_over(DataDirectory::new(Some("/nowhere/tabsmith-data".into()), None));

        let exit_code = run_debug_logs_command(&module).unwrap();

        assert_eq!(exit_code, 0);
    }
}
