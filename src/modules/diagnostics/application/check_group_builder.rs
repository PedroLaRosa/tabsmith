use crate::modules::diagnostics::application::ports::integration_probe::IntegrationProbe;
use crate::modules::diagnostics::application::ports::settings_snapshot::SettingsSnapshot;
use crate::modules::diagnostics::application::ports::system_facts_probe::SystemFactsProbe;
use crate::modules::diagnostics::domain::services::doctor_check_catalog as catalog;
use crate::modules::diagnostics::domain::value_objects::check::Check;
use crate::modules::diagnostics::domain::value_objects::check_group::CheckGroup;
use crate::modules::diagnostics::domain::value_objects::operating_system::OperatingSystem;
use crate::modules::diagnostics::domain::value_objects::system_facts::SystemFacts;
use crate::modules::diagnostics::domain::value_objects::terminal_environment::{TerminalEnvironment, TerminalEnvironmentParts};
use crate::modules::shared::domain::value_objects::shell::Shell;
use crate::modules::shell_integration::domain::value_objects::integration_paths::IntegrationPaths;
use std::rc::Rc;

/// Builds one check group at a time. Building is where a group asks the machine its construction-time questions,
/// so the use case builds a group only after the previous one passed, just before it prints the group header.
pub(crate) struct CheckGroupBuilder {
    system: Rc<dyn SystemFactsProbe>,
    integrations: Rc<dyn IntegrationProbe>,
    settings: Rc<dyn SettingsSnapshot>,
}

impl CheckGroupBuilder {
    pub(crate) fn new(
        system: Rc<dyn SystemFactsProbe>,
        integrations: Rc<dyn IntegrationProbe>,
        settings: Rc<dyn SettingsSnapshot>,
    ) -> Self {
        CheckGroupBuilder { system, integrations, settings }
    }

    /// One check per integration of the current shell and of every other shell on PATH. The listing is a snapshot:
    /// a fix does not repeat it.
    pub(crate) fn dotfiles(&self, paths: &IntegrationPaths, current: Option<Shell>) -> CheckGroup {
        let checks = Shell::ALL
            .into_iter()
            .flat_map(|shell| self.dotfile_checks_of(paths, shell, current))
            .collect();
        CheckGroup::new(catalog::DOTFILES_HEADER.to_string(), checks)
    }

    pub(crate) fn setup(&self, paths: &IntegrationPaths) -> CheckGroup {
        let settings_file = self.system.settings_file(paths.data_root());
        let windows = self.system.operating_system() == OperatingSystem::Windows;
        CheckGroup::new(catalog::SETUP_HEADER.to_string(), catalog::setup_checks(settings_file, windows))
    }

    /// The only place the settings are loaded, so a corrupt settings file is reported once per run.
    pub(crate) fn environment(&self, parent: Option<&str>) -> CheckGroup {
        let terminal = self.terminal_environment(parent);
        let session_id = self.system.env_text("TABSMITH_SESSION_ID");
        let tabsmith_shell = self.system.env_text("TABSMITH_SHELL");
        let facts = SystemFacts::new(terminal, session_id, self.settings.load());
        CheckGroup::new(
            catalog::ENVIRONMENT_HEADER.to_string(),
            catalog::environment_checks(facts, parent, tabsmith_shell.as_deref()),
        )
    }

    pub(crate) fn system(&self, current: Option<Shell>, parent: Option<&str>) -> CheckGroup {
        let fish_on_path = self.system.is_on_path(Shell::Fish.name());
        CheckGroup::new(catalog::SYSTEM_HEADER.to_string(), catalog::system_checks(current, parent, fish_on_path))
    }

    fn dotfile_checks_of(&self, paths: &IntegrationPaths, shell: Shell, current: Option<Shell>) -> Vec<Check> {
        let Some(severity) = catalog::dotfile_severity(shell, current, || self.system.is_on_path(shell.name())) else {
            return Vec::new();
        };
        match self.integrations.list(paths, shell) {
            Ok(integrations) => integrations
                .into_iter()
                .map(|integration| catalog::dotfile_check(paths.home(), integration, severity))
                .collect(),
            Err(error) => vec![catalog::listing_failure_check(shell, severity, error.to_string())],
        }
    }

    fn terminal_environment(&self, parent: Option<&str>) -> TerminalEnvironment {
        TerminalEnvironment::new(TerminalEnvironmentParts {
            term_program: self.system.env_text("TERM_PROGRAM"),
            bundle_identifier: self.system.env_text("__CFBundleIdentifier"),
            tabsmith_term: self.system.env_text("TABSMITH_TERM"),
            inside_emacs: self.system.env_is_set("INSIDE_EMACS"),
            launched_by_tabsmith: self.system.env_is_set("PROCESS_LAUNCHED_BY_TABSMITH"),
            in_ci: self.system.env_is_set("CI"),
            parent_is_cmd: catalog::parent_is_cmd(parent),
            tabsmith_version: self.system.tabsmith_version(),
        })
    }
}

#[cfg(test)]
mod tests;
