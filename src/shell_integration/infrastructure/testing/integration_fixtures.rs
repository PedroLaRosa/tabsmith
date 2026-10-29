//! Fixtures shared by the shell integration tests: the paths every dotfile test installs into, the syntax check of
//! a script in each installed checker of its shell, and a test driver over the production pieces. The driver only
//! builds the installer over the files on disk, the system clock and the running system's shell facts (and the
//! uninstall use case over it and the data directory on disk), and delegates to them (and to the CLI's failures
//! presenter and uninstall report): it holds no branching, formatting or text of its own, so every string a test
//! asserts still comes from production code.
use crate::shared::domain::services::shell_quoting::quote;
use crate::shared::domain::value_objects::shell::Shell;
use crate::shared::infrastructure::os::executable_lookup::which;
use crate::shared::infrastructure::system_clock::SystemClock;
use crate::shell_integration::application::integration_installer::IntegrationInstaller;
use crate::shell_integration::application::uninstall_tabsmith_use_case::UninstallTabsmithUseCase;
use crate::shell_integration::domain::entities::integration::Integration;
use crate::shell_integration::domain::services::manual_install_instructions;
use crate::shell_integration::domain::shell_integration_error::ShellIntegrationError;
use crate::shell_integration::domain::value_objects::integration_paths::IntegrationPaths;
use crate::shell_integration::domain::value_objects::integration_phase::IntegrationPhase;
use crate::shell_integration::infrastructure::adapters::file_backup_repository::FileBackupRepository;
use crate::shell_integration::infrastructure::adapters::file_data_directory_purger::FileDataDirectoryPurger;
use crate::shell_integration::infrastructure::adapters::file_dotfile_repository::FileDotfileRepository;
use crate::shell_integration::infrastructure::adapters::os_shell_environment::OsShellEnvironment;
use crate::shell_integration::infrastructure::cli::integrations_command::integration_failures_error;
use crate::shell_integration::infrastructure::cli::uninstall_command::uninstall_and_report;
use std::path::Path;
use std::process::Command;
use std::rc::Rc;
use std::sync::Arc;

pub(crate) fn ctx(dir: &Path) -> IntegrationPaths {
    let home = dir.join("home");
    IntegrationPaths::new(
        home.clone(),
        home.join(".tabsmith"),
        "/opt/tab smith/it's/tabsmith".into(), // space and quote exercise the quoting
        None,
        home.join(".config/fish"),
        Some(home.join(".config/powershell/Microsoft.PowerShell_profile.ps1")),
    )
}

/// Syntax-check `file` with every checker of `shell` that is installed (others are skipped).
pub(crate) fn syntax_ok(shell: Shell, file: &Path) {
    let f = file.display().to_string();
    let checks: Vec<(&str, Vec<String>)> = match shell {
        Shell::Bash => vec![
            ("bash", vec!["-n".into(), f]),
            (
                "shellcheck",
                vec![
                    "-s".into(),
                    "bash".into(),
                    "-e".into(),
                    "SC1090,SC1091,SC2148".into(),
                    file.display().to_string(),
                ],
            ),
        ],
        Shell::Zsh => vec![("zsh", vec!["-n".into(), f])],
        Shell::Fish => vec![("fish", vec!["--no-execute".into(), f])],
        Shell::Pwsh => vec![(
            "pwsh",
            vec![
                "-NoProfile".into(),
                "-Command".into(),
                format!(
                    "$null = [ScriptBlock]::Create((Get-Content -Raw -LiteralPath {}))",
                    quote(Shell::Pwsh, &f)
                ),
            ],
        )],
    };
    for (prog, args) in checks {
        if which(prog).is_none() {
            continue;
        }
        let out = Command::new(prog).args(&args).output().unwrap();
        assert!(
            out.status.success(),
            "{prog} rejects {}:\n{}{}",
            file.display(),
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        );
    }
}

/// The installer production wires: the files on disk, the system clock, the running system's shell facts.
fn installer() -> IntegrationInstaller {
    IntegrationInstaller::new(
        Rc::new(FileDotfileRepository),
        Rc::new(FileBackupRepository),
        Arc::new(SystemClock),
        Rc::new(OsShellEnvironment),
    )
}

/// The operations of one integration on disk, through the installer.
pub(crate) trait IntegrationOnDisk {
    fn is_installed(&self, paths: &IntegrationPaths) -> Result<(), ShellIntegrationError>;
    fn install(&self, paths: &IntegrationPaths) -> Result<(), ShellIntegrationError>;
    fn uninstall(&self, paths: &IntegrationPaths) -> Result<(), ShellIntegrationError>;
    fn write_scripts(&self, paths: &IntegrationPaths) -> Result<(), ShellIntegrationError>;
    fn has_traces(&self, paths: &IntegrationPaths) -> bool;
    fn script_contents(&self, paths: &IntegrationPaths, phase: IntegrationPhase) -> String;
}

impl IntegrationOnDisk for Integration {
    fn is_installed(&self, paths: &IntegrationPaths) -> Result<(), ShellIntegrationError> {
        installer().check(paths, self)
    }

    fn install(&self, paths: &IntegrationPaths) -> Result<(), ShellIntegrationError> {
        installer().install(paths, self)
    }

    fn uninstall(&self, paths: &IntegrationPaths) -> Result<(), ShellIntegrationError> {
        installer().uninstall(paths, self)
    }

    fn write_scripts(&self, paths: &IntegrationPaths) -> Result<(), ShellIntegrationError> {
        installer().write_scripts(paths, self)
    }

    fn has_traces(&self, paths: &IntegrationPaths) -> bool {
        installer().has_traces(paths, self)
    }

    fn script_contents(&self, paths: &IntegrationPaths, phase: IntegrationPhase) -> String {
        self.script(paths, phase).contents().to_string()
    }
}

/// The integrations of `shell`, as the installer lists them.
pub(crate) fn integrations(paths: &IntegrationPaths, shell: Shell) -> Result<Vec<Integration>, ShellIntegrationError> {
    installer().integrations(paths, shell)
}

/// Install every integration of `shells` that is not installed, failing with the text main prints.
pub(crate) fn install_dotfiles(paths: &IntegrationPaths, shells: &[Shell]) -> anyhow::Result<bool> {
    installer()
        .install_missing(paths, shells)
        .map_err(integration_failures_error)
}

/// Uninstall every integration of `shells` with something on disk, failing with the text main prints.
pub(crate) fn uninstall_dotfiles(paths: &IntegrationPaths, shells: &[Shell]) -> anyhow::Result<bool> {
    installer()
        .uninstall_traced(paths, shells)
        .map_err(integration_failures_error)
}

/// Uninstall every integration of `shells` (and purge the data root of `paths`), printing what `tabsmith uninstall`
/// prints: its exit code.
pub(crate) fn uninstall_all(paths: &IntegrationPaths, shells: &[Shell], purge: bool) -> i32 {
    uninstall_and_report(
        &UninstallTabsmithUseCase::new(Rc::new(installer()), Rc::new(FileDataDirectoryPurger)),
        paths,
        shells,
        purge,
    )
}

/// The lines `setup` prints for a manual install.
pub(crate) fn manual_text(paths: &IntegrationPaths, shells: &[Shell]) -> String {
    manual_install_instructions::text(paths, shells)
}
