use crate::diagnostics::domain::value_objects::check::Check;
use crate::diagnostics::domain::value_objects::check_fix::CheckFix;
use crate::diagnostics::domain::value_objects::check_outcome::CheckOutcome;
use crate::diagnostics::domain::value_objects::check_severity::CheckSeverity;
use crate::diagnostics::domain::value_objects::check_subject::CheckSubject;
use crate::diagnostics::domain::value_objects::info_line::InfoLine;
use crate::shared::domain::value_objects::shell::Shell;
use crate::shell_integration::domain::entities::integration::Integration;
use crate::shell_integration::domain::shell_integration_error::ShellIntegrationError;
use std::path::{Path, PathBuf};

pub(crate) const DOTFILES_HEADER: &str = "Let's check your dotfiles...";
pub(crate) const SETUP_HEADER: &str = "Let's make sure tabsmith is set up correctly...";

/// The first Windows build with ConPTY (Windows 10 1809).
const CONPTY_BUILD: u32 = 17763;

/// `~/rel` when under home.
pub(crate) fn short_path(home: &Path, p: &Path) -> String {
    match p.strip_prefix(home) {
        Ok(rel) => format!("~/{}", rel.display()),
        Err(_) => p.display().to_string(),
    }
}

/// The parent shell is always checked; another shell only when it is on PATH (and never asked otherwise).
pub(crate) fn dotfile_severity(
    shell: Shell,
    current: Option<Shell>,
    on_path: impl FnOnce() -> bool,
) -> Option<CheckSeverity> {
    if Some(shell) == current {
        Some(CheckSeverity::Normal)
    } else if on_path() {
        Some(CheckSeverity::Soft)
    } else {
        None
    }
}

pub(crate) fn dotfile_check(home: &Path, integration: Integration, severity: CheckSeverity) -> Check {
    let name = format!(
        "{} {} integration check",
        integration.shell().name(),
        short_path(home, integration.path())
    );
    Check::new(name, severity, CheckSubject::Dotfile(integration))
}

/// The check that stands for a shell whose integrations could not be listed.
pub(crate) fn listing_failure_check(shell: Shell, severity: CheckSeverity, reason: String) -> Check {
    Check::new(
        format!("{} integration check", shell.name()),
        severity,
        CheckSubject::ListingFailure(reason),
    )
}

pub(crate) fn dotfile_outcome(integration: &Integration, result: Result<(), ShellIntegrationError>) -> CheckOutcome {
    let error = match result {
        Ok(()) => return CheckOutcome::Ok,
        Err(error) => error,
    };
    let reason = match &error {
        ShellIntegrationError::NotInstalled(message) | ShellIntegrationError::Improper(message) => message.clone(),
        ShellIntegrationError::FileDoesNotExist(_) => error.to_string(),
        ShellIntegrationError::PermissionDenied(_) => error.to_string(),
        ShellIntegrationError::Io(..) => error.to_string(),
        ShellIntegrationError::Environment(_) => error.to_string(),
        ShellIntegrationError::SpecBundle(_) => error.to_string(),
    };
    let fix = match &error {
        ShellIntegrationError::PermissionDenied(_) => None,
        _ => Some(CheckFix::InstallIntegration(integration.clone())),
    };
    CheckOutcome::Err {
        reason,
        info: vec![reinstall_hint(integration.shell())],
        fix,
    }
}

fn reinstall_hint(shell: Shell) -> InfoLine {
    let shell = shell.name();
    InfoLine::Command {
        before: "Run ".to_string(),
        command: format!("tabsmith integrations install dotfiles {shell}"),
        after: format!(" to reinstall shell integrations for {shell}"),
    }
}

pub(crate) fn setup_checks(settings_file: PathBuf, windows: bool) -> Vec<Check> {
    let mut checks = vec![
        normal_check("tabsmith data dir exists", CheckSubject::DataDirectory),
        normal_check("Integration scripts point at this binary", CheckSubject::ScriptsPointAtBinary),
        normal_check("tabsmith settings corruption", CheckSubject::SettingsCorruption(settings_file)),
        normal_check("Completion specs installed", CheckSubject::SpecsExtracted),
    ];
    if windows {
        checks.push(normal_check("Windows Console Check", CheckSubject::WindowsConsole));
        checks.push(normal_check("Windows version", CheckSubject::WindowsVersion));
    }
    checks
}

fn normal_check(name: &str, subject: CheckSubject) -> Check {
    Check::new(name.to_string(), CheckSeverity::Normal, subject)
}

pub(crate) fn data_directory_outcome(data_root: &Path, exists: bool) -> CheckOutcome {
    if exists {
        return CheckOutcome::Ok;
    }
    CheckOutcome::Err {
        reason: format!("No directory at path {}", data_root.display()),
        info: vec![],
        fix: Some(CheckFix::CreateDataDirectory(data_root.to_path_buf())),
    }
}

/// `script_binaries` is the binary each current script file points at; every other binary than `binary` is stale.
pub(crate) fn scripts_outcome(binary: &str, script_binaries: &[(Shell, String)]) -> CheckOutcome {
    let stale: Vec<&(Shell, String)> = script_binaries.iter().filter(|(_, bin)| bin != binary).collect();
    let Some((_, first_stale)) = stale.first() else {
        return CheckOutcome::Ok;
    };
    let mut shells: Vec<Shell> = stale.iter().map(|(shell, _)| *shell).collect();
    shells.dedup();
    CheckOutcome::Err {
        reason: format!("Integration scripts reference {first_stale}"),
        info: vec![],
        fix: Some(CheckFix::InstallMissingIntegrations(shells)),
    }
}

pub(crate) fn settings_outcome(settings_file: &Path, loads: bool) -> CheckOutcome {
    if loads {
        return CheckOutcome::Ok;
    }
    CheckOutcome::Err {
        reason: "tabsmith settings file is corrupted".into(),
        info: vec![],
        fix: Some(CheckFix::ResetSettings(settings_file.to_path_buf())),
    }
}

pub(crate) fn specs_outcome(extracted: bool) -> CheckOutcome {
    if extracted {
        return CheckOutcome::Ok;
    }
    CheckOutcome::Err {
        reason: "Completion specs are not extracted".into(),
        info: vec![],
        fix: Some(CheckFix::InstallSpecBundle),
    }
}

pub(crate) fn windows_console_outcome(attached: bool) -> CheckOutcome {
    if attached {
        return CheckOutcome::Ok;
    }
    CheckOutcome::Err {
        reason: "Windows Console APIs are not supported in this terminal".into(),
        info: windows_console_info(),
        fix: None,
    }
}

fn windows_console_info() -> Vec<InfoLine> {
    [
        "The pseudoterminal layer only supports the new Windows Console API.",
        "MinTTY and other TTY implementations may not work properly.",
        "",
        "You can try the following fixes to get completions working:",
        "- If using Git for Windows, reinstall and choose \"Use default console window\" instead of MinTTY",
        "- If using Git for Windows and you really want to use MinTTY, reinstall and check \"Enable experimental support for pseudo consoles\"",
        "- Use your shell with a supported terminal emulator like Windows Terminal.",
        "- Launch your terminal emulator with winpty (e.g. winpty mintty). NOTE: this can lead to some UI bugs.",
    ]
    .map(String::from)
    .map(InfoLine::Text)
    .to_vec()
}

pub(crate) fn windows_version_setup_outcome(build: Option<u32>) -> CheckOutcome {
    match build {
        Some(b) if b < CONPTY_BUILD => CheckOutcome::err("Windows 10 1809 or newer is required (ConPTY)"),
        _ => CheckOutcome::Ok,
    }
}

#[cfg(test)]
mod tests;
