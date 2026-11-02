use crate::diagnostics::domain::value_objects::check::Check;
use crate::diagnostics::domain::value_objects::check_fix::CheckFix;
use crate::diagnostics::domain::value_objects::check_outcome::CheckOutcome;
use crate::diagnostics::domain::value_objects::check_severity::CheckSeverity;
use crate::diagnostics::domain::value_objects::check_subject::CheckSubject;
use crate::diagnostics::domain::value_objects::environment_check::EnvironmentCheck;
use crate::diagnostics::domain::value_objects::info_line::InfoLine;
use crate::diagnostics::domain::value_objects::operating_system::OperatingSystem;
use crate::diagnostics::domain::value_objects::system_check::SystemCheck;
use crate::diagnostics::domain::value_objects::system_facts::SystemFacts;
use crate::diagnostics::domain::value_objects::terminal_environment::TerminalEnvironment;
use crate::diagnostics::domain::value_objects::tool_version::ToolVersion;
use crate::ghost_text::domain::services::ghost_text_source_resolver;
use crate::ghost_text::domain::value_objects::ghost_text_source::GhostTextSource;
use crate::settings::domain::services::settings_catalog::{
    AUTOCOMPLETE_DEVELOPER_MODE, AUTOCOMPLETE_DISABLE, INLINE_ENABLED, INLINE_SOURCE,
};
use crate::settings::domain::value_objects::settings::Settings;
use crate::shared::domain::value_objects::shell::Shell;
use crate::shell_integration::domain::entities::integration::Integration;
use crate::shell_integration::domain::shell_integration_error::ShellIntegrationError;
use std::path::{Path, PathBuf};
use std::rc::Rc;

pub(crate) const DOTFILES_HEADER: &str = "Let's check your dotfiles...";
pub(crate) const SETUP_HEADER: &str = "Let's make sure tabsmith is set up correctly...";
pub(crate) const ENVIRONMENT_HEADER: &str = "Let's check if your environment is supported...";
pub(crate) const SYSTEM_HEADER: &str = "Let's check if your system is compatible...";

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

/// The parent process is `cmd.exe` (or `cmd`), whatever the directory separator.
pub(crate) fn parent_is_cmd(parent: Option<&str>) -> bool {
    parent.is_some_and(|path| {
        let base = path.rsplit(['/', '\\']).next().unwrap_or(path);
        base.eq_ignore_ascii_case("cmd.exe") || base.eq_ignore_ascii_case("cmd")
    })
}

/// The Session check runs inside a wrapped session, except pwsh, which never runs `_ pre-cmd` (SPEC 5.4.5).
/// The session shell is the parent process, else `TABSMITH_SHELL` (exported by the pre scripts).
pub(crate) fn session_kind(
    session_id: Option<&str>,
    parent: Option<&str>,
    tabsmith_shell: Option<&str>,
) -> CheckSeverity {
    let shell = parent
        .and_then(Shell::from_name)
        .or_else(|| tabsmith_shell.and_then(Shell::from_name));
    if session_id.is_some() && shell != Some(Shell::Pwsh) {
        CheckSeverity::Normal
    } else {
        CheckSeverity::Skip
    }
}

pub(crate) fn environment_checks(
    facts: SystemFacts,
    parent: Option<&str>,
    tabsmith_shell: Option<&str>,
) -> Vec<Check> {
    let session = session_kind(facts.session_id(), parent, tabsmith_shell);
    let inline = if facts.settings().bool(INLINE_ENABLED) {
        CheckSeverity::Normal
    } else {
        CheckSeverity::Soft
    };
    let facts = Rc::new(facts);
    let environment_check = |name: &str, severity: CheckSeverity, which: EnvironmentCheck| {
        Check::new(name.to_string(), severity, CheckSubject::Environment(which, Rc::clone(&facts)))
    };
    vec![
        environment_check("tabsmith terminal integrations", CheckSeverity::Normal, EnvironmentCheck::TerminalIntegrations),
        environment_check("Session check", session, EnvironmentCheck::Session),
        environment_check("Autocomplete is enabled", CheckSeverity::Normal, EnvironmentCheck::AutocompleteEnabled),
        environment_check("Autocomplete dev mode", CheckSeverity::Normal, EnvironmentCheck::DeveloperMode),
        environment_check("Inline", inline, EnvironmentCheck::Inline),
    ]
}

/// The first unsupported terminal wins; a supported one must carry this version's integration.
pub(crate) fn terminal_outcome(terminal: &TerminalEnvironment) -> CheckOutcome {
    if terminal.term_program() == Some("WarpTerminal") {
        return CheckOutcome::err("WarpTerminal is not supported");
    }
    if terminal.parent_is_cmd() {
        return CheckOutcome::err("CMD isn't supported, use PowerShell 7 (pwsh) instead");
    }
    if terminal.inside_emacs() {
        return CheckOutcome::err("Emacs is not supported");
    }
    if terminal.bundle_identifier() == Some("com.vandyke.SecureCRT") {
        return CheckOutcome::err("SecureCRT is not supported");
    }
    if terminal.launched_by_tabsmith() {
        return CheckOutcome::err("tabsmith can not run in a process it launched");
    }
    if terminal.in_ci() {
        return CheckOutcome::err("Doctor can not run in CI");
    }
    match terminal.tabsmith_term() {
        Some(term) if term == terminal.tabsmith_version() => CheckOutcome::Ok,
        None => CheckOutcome::err("tabsmith is not running in this terminal, please try restarting your terminal"),
        Some(term) => CheckOutcome::Err {
            reason: "This terminal is not running with the latest integration, please restart your terminal".into(),
            info: vec![InfoLine::Text(format!("TABSMITH_TERM={term}"))],
            fix: None,
        },
    }
}

pub(crate) fn session_outcome(run_file_exists: bool) -> CheckOutcome {
    if run_file_exists {
        return CheckOutcome::Ok;
    }
    CheckOutcome::Warn("Shell has not reported its environment yet (tabsmith _ pre-cmd did not run)".into())
}

pub(crate) fn autocomplete_enabled_outcome(settings: &Settings) -> CheckOutcome {
    if !settings.bool(AUTOCOMPLETE_DISABLE) {
        return CheckOutcome::Ok;
    }
    CheckOutcome::Err {
        reason: "Autocomplete disabled.".into(),
        info: vec![InfoLine::Text("To fix run: tabsmith settings autocomplete.disable false".into())],
        fix: None,
    }
}

pub(crate) fn developer_mode_outcome(settings: &Settings) -> CheckOutcome {
    if settings.bool(AUTOCOMPLETE_DEVELOPER_MODE) {
        CheckOutcome::Warn("autocomplete.developerMode is enabled".into())
    } else {
        CheckOutcome::Ok
    }
}

/// `env_is_set(name)` = the variable is set and non-empty.
pub(crate) fn inline_outcome(settings: &Settings, env_is_set: impl Fn(&str) -> bool) -> CheckOutcome {
    if !settings.bool(INLINE_ENABLED) {
        return CheckOutcome::Warn("Inline is disabled, to re-enable run: tabsmith inline enable".into());
    }
    match ghost_text_source_resolver::source(settings) {
        Err(_) => CheckOutcome::err(format!(
            "Inline source '{}' is not a valid profile",
            settings.string(INLINE_SOURCE).unwrap_or_default()
        )),
        Ok(GhostTextSource::Profile(profile))
            if !is_local(profile.base_url()) && !env_is_set(profile.api_key_env()) =>
        {
            CheckOutcome::err(format!("{} is not set", profile.api_key_env()))
        }
        Ok(GhostTextSource::Profile(_)) => CheckOutcome::Ok,
        Ok(GhostTextSource::History | GhostTextSource::Specs | GhostTextSource::HistoryAndSpecs) => CheckOutcome::Ok,
    }
}

fn is_local(url: &str) -> bool {
    ["://localhost", "://127.0.0.1", "://[::1]"]
        .iter()
        .any(|host| url.contains(host))
}

pub(crate) fn system_checks(current: Option<Shell>, parent: Option<&str>, fish_on_path: bool) -> Vec<Check> {
    let running_bash = current == Some(Shell::Bash);
    let program = if running_bash { parent.unwrap_or("bash") } else { "bash" };
    let bash_severity = if running_bash { CheckSeverity::Soft } else { CheckSeverity::Skip };
    let fish_severity = if fish_on_path { CheckSeverity::Normal } else { CheckSeverity::Skip };
    vec![
        Check::new("OS is supported".to_string(), CheckSeverity::Normal, CheckSubject::System(SystemCheck::Os)),
        Check::new(
            "Bash is new enough".to_string(),
            bash_severity,
            CheckSubject::System(SystemCheck::Bash { program: program.to_string() }),
        ),
        Check::new("Fish is up to date".to_string(), fish_severity, CheckSubject::System(SystemCheck::Fish)),
    ]
}

/// macOS >= 11, Linux, Windows >= 1809; the version is only asked on the systems that need it.
pub(crate) fn os_outcome(
    os: &OperatingSystem,
    macos_version: impl FnOnce() -> Option<ToolVersion>,
    windows_build: impl FnOnce() -> Option<u32>,
) -> CheckOutcome {
    match os {
        OperatingSystem::Linux => CheckOutcome::Ok,
        OperatingSystem::Macos => match macos_version() {
            Some(version) if version.major() < 11 => CheckOutcome::Warn(format!(
                "macOS {version} is not supported (needs macOS 11 or newer)"
            )),
            _ => CheckOutcome::Ok,
        },
        OperatingSystem::Windows => match windows_build() {
            Some(build) if build < CONPTY_BUILD => {
                CheckOutcome::Warn("Windows 10 1809 or newer is required (ConPTY)".into())
            }
            _ => CheckOutcome::Ok,
        },
        OperatingSystem::Other(name) => CheckOutcome::Warn(format!("{name} is not supported")),
    }
}

pub(crate) fn bash_outcome(version: Option<ToolVersion>) -> CheckOutcome {
    match version {
        Some(version) if version.is_below(&[4, 4]) => CheckOutcome::err(format!(
            "Bash {}.{} is not supported (needs >= 4.4 for PS0); install a newer bash or switch to zsh",
            version.major(),
            version.minor_or_zero()
        )),
        _ => CheckOutcome::Ok,
    }
}

pub(crate) fn fish_outcome(version: Option<ToolVersion>) -> CheckOutcome {
    match version {
        Some(version) if version.is_below(&[3, 3, 0]) => CheckOutcome::err(format!(
            "your fish version is outdated (need at least 3.3.0, found {version})"
        )),
        _ => CheckOutcome::Ok,
    }
}

#[cfg(test)]
mod tests;
