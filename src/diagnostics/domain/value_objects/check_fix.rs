use crate::shared::domain::value_objects::shell::Shell;
use crate::shell_integration::domain::entities::integration::Integration;
use std::path::PathBuf;

/// What doctor can do to repair a failed check.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum CheckFix {
    CreateDataDirectory(PathBuf),
    InstallIntegration(Integration),
    /// Installs the integrations that are missing; it is not a reinstall.
    InstallMissingIntegrations(Vec<Shell>),
    ResetSettings(PathBuf),
    InstallSpecBundle,
}

impl CheckFix {
    /// Both integration fixes mark the terminal restart reminder, even when they installed nothing.
    pub(crate) fn marks_restart_reminder(&self) -> bool {
        match self {
            CheckFix::CreateDataDirectory(_) => false,
            CheckFix::InstallIntegration(_) => true,
            CheckFix::InstallMissingIntegrations(_) => true,
            CheckFix::ResetSettings(_) => false,
            CheckFix::InstallSpecBundle => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shared::domain::value_objects::shell::Shell;
    use crate::shell_integration::domain::entities::integration::Integration;
    use std::path::PathBuf;

    #[test]
    fn should_not_mark_the_restart_reminder_when_creating_the_data_directory() {
        let fix = CheckFix::CreateDataDirectory(PathBuf::from("/data"));

        assert!(!fix.marks_restart_reminder());
    }

    #[test]
    fn should_mark_the_restart_reminder_when_installing_an_integration() {
        let fix = CheckFix::InstallIntegration(Integration::dotfile(Shell::Bash, PathBuf::from("/home/u/.bashrc")));

        assert!(fix.marks_restart_reminder());
    }

    #[test]
    fn should_mark_the_restart_reminder_when_installing_the_missing_integrations() {
        let fix = CheckFix::InstallMissingIntegrations(Vec::new());

        assert!(fix.marks_restart_reminder());
    }

    #[test]
    fn should_not_mark_the_restart_reminder_when_resetting_the_settings() {
        let fix = CheckFix::ResetSettings(PathBuf::from("/data/settings.json"));

        assert!(!fix.marks_restart_reminder());
    }

    #[test]
    fn should_not_mark_the_restart_reminder_when_installing_the_spec_bundle() {
        let fix = CheckFix::InstallSpecBundle;

        assert!(!fix.marks_restart_reminder());
    }
}
