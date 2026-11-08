use crate::modules::diagnostics::application::check_evaluator::CheckEvaluator;
use crate::modules::diagnostics::application::check_group_builder::CheckGroupBuilder;
use crate::modules::diagnostics::application::collect_diagnostic_report_use_case::CollectDiagnosticReportUseCase;
use crate::modules::diagnostics::application::ports::bundled_specs_probe::BundledSpecsProbe;
use crate::modules::diagnostics::application::ports::check_fixer::CheckFixer;
use crate::modules::diagnostics::application::ports::integration_probe::IntegrationProbe;
use crate::modules::diagnostics::application::ports::system_facts_probe::SystemFactsProbe;
use crate::modules::diagnostics::application::run_doctor_use_case::RunDoctorUseCase;
use crate::modules::diagnostics::application::show_recent_logs_use_case::ShowRecentLogsUseCase;
use crate::modules::diagnostics::infrastructure::adapters::file_log_reader::FileLogReader;
use crate::modules::diagnostics::infrastructure::adapters::lenient_settings::LenientSettings;
use crate::modules::diagnostics::infrastructure::adapters::os_diagnostic_host::OsDiagnosticHost;
use crate::modules::diagnostics::infrastructure::adapters::os_system_facts_probe::OsSystemFactsProbe;
use crate::modules::diagnostics::infrastructure::adapters::terminal_doctor_reporter::TerminalDoctorReporter;
use crate::modules::settings::domain::repositories::settings_repository::SettingsRepository;
use crate::modules::shared::infrastructure::shared_dependencies::SharedDependencies;
use crate::modules::shell_integration::domain::repositories::restart_reminder_repository::RestartReminderRepository;
use std::rc::Rc;

/// The diagnostics use cases over the running system, the terminal, the log files and the settings file.
pub(crate) struct DiagnosticsModule {
    pub(crate) run_doctor: RunDoctorUseCase,
    pub(crate) collect_diagnostic_report: CollectDiagnosticReportUseCase,
    pub(crate) show_recent_logs: ShowRecentLogsUseCase,
}

/// Only builds: it reads no env, looks nothing up on PATH, runs no `pwsh` and opens no file, so `debug logs`,
/// `diagnostic` and the doctor's root check keep failing at the same moment with the same text without a home.
pub(crate) fn build_diagnostics_module(
    dependencies: &SharedDependencies,
    settings: Rc<dyn SettingsRepository>,
    restart_reminder: Rc<dyn RestartReminderRepository>,
    integrations: Rc<dyn IntegrationProbe>,
    bundled_specs: Rc<dyn BundledSpecsProbe>,
    fixer: Rc<dyn CheckFixer>,
) -> DiagnosticsModule {
    let system: Rc<dyn SystemFactsProbe> = Rc::new(OsSystemFactsProbe::new(dependencies.data_directory.clone()));
    let builder = CheckGroupBuilder::new(system.clone(), integrations.clone(), Rc::new(LenientSettings::new(settings)));
    let evaluator = CheckEvaluator::new(system.clone(), integrations.clone(), bundled_specs);
    DiagnosticsModule {
        run_doctor: RunDoctorUseCase::new(
            builder,
            evaluator,
            fixer,
            Rc::new(TerminalDoctorReporter::new()),
            system,
            integrations,
            restart_reminder,
        ),
        collect_diagnostic_report: CollectDiagnosticReportUseCase::new(Rc::new(OsDiagnosticHost)),
        show_recent_logs: ShowRecentLogsUseCase::new(dependencies.data_directory.clone(), Rc::new(FileLogReader)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::diagnostics::application::ports::bundled_specs_probe::InMemoryBundledSpecsProbe;
    use crate::modules::diagnostics::application::ports::check_fixer::RecordingCheckFixer;
    use crate::modules::diagnostics::application::ports::integration_probe::InMemoryIntegrationProbe;
    use crate::modules::settings::domain::repositories::settings_repository::InMemorySettingsRepository;
    use crate::modules::shared::application::ports::clock::FixedClock;
    use crate::modules::shared::domain::value_objects::data_directory::DataDirectory;
    use crate::modules::shell_integration::domain::repositories::restart_reminder_repository::InMemoryRestartReminderRepository;
    use std::sync::Arc;

    fn module_without_a_home() -> DiagnosticsModule {
        build_diagnostics_module(
            &SharedDependencies {
                data_directory: DataDirectory::new(None, None),
                clock: Arc::new(FixedClock::new(0)),
            },
            Rc::new(InMemorySettingsRepository::default()),
            Rc::new(InMemoryRestartReminderRepository::default()),
            Rc::new(InMemoryIntegrationProbe::default()),
            Rc::new(InMemoryBundledSpecsProbe::extracted(true)),
            Rc::new(RecordingCheckFixer::succeeding()),
        )
    }

    /// Building reads nothing (D3): the missing home surfaces only from the paths a command asks for.
    #[test]
    fn should_build_without_a_home_and_fail_only_when_the_log_paths_are_asked_for() {
        let module = module_without_a_home();

        let error = module.show_recent_logs.paths().unwrap_err();

        assert_eq!(
            error.to_string(),
            "could not determine the home directory (set HOME or TABSMITH_HOME)"
        );
    }

    #[test]
    fn should_locate_the_logs_under_the_data_directory_it_was_built_with() {
        let module = build_diagnostics_module(
            &SharedDependencies {
                data_directory: DataDirectory::new(Some("/nowhere/tabsmith-data".into()), None),
                clock: Arc::new(FixedClock::new(0)),
            },
            Rc::new(InMemorySettingsRepository::default()),
            Rc::new(InMemoryRestartReminderRepository::default()),
            Rc::new(InMemoryIntegrationProbe::default()),
            Rc::new(InMemoryBundledSpecsProbe::extracted(true)),
            Rc::new(RecordingCheckFixer::succeeding()),
        );

        let paths = module.show_recent_logs.paths().unwrap();

        assert!(paths.cli_log().starts_with("/nowhere/tabsmith-data"));
    }
}
