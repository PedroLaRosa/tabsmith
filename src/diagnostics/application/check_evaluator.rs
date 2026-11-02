use crate::diagnostics::application::ports::bundled_specs_probe::BundledSpecsProbe;
use crate::diagnostics::application::ports::integration_probe::IntegrationProbe;
use crate::diagnostics::application::ports::system_facts_probe::SystemFactsProbe;
use crate::diagnostics::domain::services::doctor_check_catalog as catalog;
use crate::diagnostics::domain::value_objects::check::Check;
use crate::diagnostics::domain::value_objects::check_outcome::CheckOutcome;
use crate::diagnostics::domain::value_objects::check_subject::CheckSubject;
use crate::diagnostics::domain::value_objects::environment_check::EnvironmentCheck;
use crate::diagnostics::domain::value_objects::system_check::SystemCheck;
use crate::diagnostics::domain::value_objects::system_facts::SystemFacts;
use crate::shell_integration::domain::entities::integration::Integration;
use crate::shell_integration::domain::value_objects::integration_paths::IntegrationPaths;
use std::path::Path;
use std::rc::Rc;

/// Evaluates one check at a time. It asks the machine only the facts that check needs, every time it is called, so
/// a check evaluated again after a fix sees the repaired machine.
pub(crate) struct CheckEvaluator {
    system: Rc<dyn SystemFactsProbe>,
    integrations: Rc<dyn IntegrationProbe>,
    bundled_specs: Rc<dyn BundledSpecsProbe>,
}

impl CheckEvaluator {
    pub(crate) fn new(
        system: Rc<dyn SystemFactsProbe>,
        integrations: Rc<dyn IntegrationProbe>,
        bundled_specs: Rc<dyn BundledSpecsProbe>,
    ) -> Self {
        CheckEvaluator { system, integrations, bundled_specs }
    }

    pub(crate) fn evaluate(&self, check: &Check, paths: &IntegrationPaths) -> CheckOutcome {
        match check.subject() {
            CheckSubject::Dotfile(integration) => self.dotfile(integration, paths),
            CheckSubject::ListingFailure(text) => CheckOutcome::err(text.as_str()),
            CheckSubject::DataDirectory => self.data_directory(paths.data_root()),
            CheckSubject::ScriptsPointAtBinary => self.scripts(paths),
            CheckSubject::SettingsCorruption(settings_file) => {
                catalog::settings_outcome(settings_file, self.system.settings_file_loads(settings_file))
            }
            CheckSubject::SpecsExtracted => {
                catalog::specs_outcome(self.bundled_specs.is_extracted(paths.data_root()))
            }
            CheckSubject::WindowsConsole => catalog::windows_console_outcome(self.system.console_attached()),
            CheckSubject::WindowsVersion => catalog::windows_version_setup_outcome(self.system.windows_build()),
            CheckSubject::Environment(which, facts) => self.environment(*which, facts, paths),
            CheckSubject::System(which) => self.system_check(which),
        }
    }

    fn dotfile(&self, integration: &Integration, paths: &IntegrationPaths) -> CheckOutcome {
        catalog::dotfile_outcome(integration, self.integrations.check(paths, integration))
    }

    fn data_directory(&self, data_root: &Path) -> CheckOutcome {
        catalog::data_directory_outcome(data_root, self.system.directory_exists(data_root))
    }

    fn scripts(&self, paths: &IntegrationPaths) -> CheckOutcome {
        catalog::scripts_outcome(paths.binary(), &self.integrations.script_binaries(paths))
    }

    fn environment(&self, which: EnvironmentCheck, facts: &SystemFacts, paths: &IntegrationPaths) -> CheckOutcome {
        match which {
            EnvironmentCheck::TerminalIntegrations => catalog::terminal_outcome(facts.terminal()),
            EnvironmentCheck::Session => self.session(facts, paths),
            EnvironmentCheck::AutocompleteEnabled => catalog::autocomplete_enabled_outcome(facts.settings()),
            EnvironmentCheck::DeveloperMode => catalog::developer_mode_outcome(facts.settings()),
            EnvironmentCheck::Inline => {
                catalog::inline_outcome(facts.settings(), |name| self.system.env_is_non_empty(name))
            }
        }
    }

    /// The run file of a session that has no id (or an empty one) is `run/.json`, as before.
    fn session(&self, facts: &SystemFacts, paths: &IntegrationPaths) -> CheckOutcome {
        let session_id = facts.session_id().unwrap_or_default();
        let run_file = paths.data_root().join("run").join(format!("{session_id}.json"));
        catalog::session_outcome(self.system.file_exists(&run_file))
    }

    fn system_check(&self, which: &SystemCheck) -> CheckOutcome {
        match which {
            SystemCheck::Os => catalog::os_outcome(
                &self.system.operating_system(),
                || self.system.tool_version("sw_vers", &["-productVersion"]),
                || self.system.windows_build(),
            ),
            SystemCheck::Bash { program } => {
                catalog::bash_outcome(self.system.tool_version(program, &["--version"]))
            }
            SystemCheck::Fish => catalog::fish_outcome(self.system.tool_version("fish", &["--version"])),
        }
    }
}

#[cfg(test)]
mod tests;
