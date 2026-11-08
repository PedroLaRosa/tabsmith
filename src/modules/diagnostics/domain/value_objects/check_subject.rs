use crate::modules::diagnostics::domain::value_objects::environment_check::EnvironmentCheck;
use crate::modules::diagnostics::domain::value_objects::system_check::SystemCheck;
use crate::modules::diagnostics::domain::value_objects::system_facts::SystemFacts;
use crate::modules::shell_integration::domain::entities::integration::Integration;
use std::path::PathBuf;
use std::rc::Rc;

/// What a check evaluates.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum CheckSubject {
    Dotfile(Integration),
    ListingFailure(String),
    DataDirectory,
    ScriptsPointAtBinary,
    SettingsCorruption(PathBuf),
    SpecsExtracted,
    WindowsConsole,
    WindowsVersion,
    /// The facts are shared by every check of the environment group.
    Environment(EnvironmentCheck, Rc<SystemFacts>),
    System(SystemCheck),
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::diagnostics::domain::value_objects::terminal_environment::{TerminalEnvironment, TerminalEnvironmentParts};
    use crate::modules::settings::domain::value_objects::settings::Settings;
    use crate::modules::shared::domain::value_objects::shell::Shell;

    fn facts() -> Rc<SystemFacts> {
        let terminal = TerminalEnvironment::new(TerminalEnvironmentParts::default());
        Rc::new(SystemFacts::new(terminal, None, Settings::default()))
    }

    #[test]
    fn should_tell_dotfile_subjects_of_different_integrations_apart() {
        let bashrc = CheckSubject::Dotfile(Integration::dotfile(Shell::Bash, PathBuf::from("/home/u/.bashrc")));
        let zshrc = CheckSubject::Dotfile(Integration::dotfile(Shell::Zsh, PathBuf::from("/home/u/.zshrc")));

        assert_ne!(bashrc, zshrc);
    }

    #[test]
    fn should_treat_environment_subjects_of_the_same_check_as_equal() {
        let subject = CheckSubject::Environment(EnvironmentCheck::Session, facts());

        assert_eq!(subject, CheckSubject::Environment(EnvironmentCheck::Session, facts()));
    }

    #[test]
    fn should_tell_environment_subjects_of_different_checks_apart() {
        let session = CheckSubject::Environment(EnvironmentCheck::Session, facts());
        let inline = CheckSubject::Environment(EnvironmentCheck::Inline, facts());

        assert_ne!(session, inline);
    }

    #[test]
    fn should_compare_system_subjects_by_system_check() {
        let subject = CheckSubject::System(SystemCheck::Fish);

        assert_eq!((subject == CheckSubject::System(SystemCheck::Fish), subject == CheckSubject::System(SystemCheck::Os)), (true, false));
    }

    #[test]
    fn should_compare_settings_corruption_subjects_by_path() {
        let subject = CheckSubject::SettingsCorruption(PathBuf::from("/data/settings.json"));

        assert_ne!(subject, CheckSubject::SettingsCorruption(PathBuf::from("/other/settings.json")));
    }
}
