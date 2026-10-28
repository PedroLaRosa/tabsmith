//! Fixtures shared by the shell integration tests: the paths every dotfile test installs into, and the syntax
//! check of a script in each installed checker of its shell.
use crate::shared::domain::services::shell_quoting::quote;
use crate::shared::domain::value_objects::shell::Shell;
use crate::shared::infrastructure::os::executable_lookup::which;
use crate::shell_integration::domain::value_objects::integration_paths::IntegrationPaths;
use std::path::Path;
use std::process::Command;

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
