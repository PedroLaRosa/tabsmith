use crate::modules::shared::domain::value_objects::data_directory::DataDirectory;
use crate::modules::shared::infrastructure::data_directory_from_env::user_home_from_env;
use crate::modules::shared::infrastructure::error_chain::display_chain;
use crate::modules::shared::infrastructure::os::executable_path_error::ExecutablePathError;
#[cfg(windows)]
use crate::modules::shared::infrastructure::os::process_info::is_console;
use crate::modules::shared::infrastructure::os::process_info::parent_exe;
use crate::modules::shared::infrastructure::os::process_runner::output_with_timeout;
use crate::modules::shell_integration::application::ports::shell_environment::ShellEnvironment;
use crate::modules::shell_integration::domain::shell_integration_error::ShellIntegrationError;
use crate::modules::shell_integration::domain::value_objects::integration_paths::IntegrationPaths;
use crate::modules::shell_integration::domain::value_objects::launch_facts::LaunchFacts;
use std::collections::HashMap;
use std::error::Error;
use std::ffi::OsStr;
#[cfg(not(windows))]
use std::io::IsTerminal;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

/// Prints `$PROFILE` as raw UTF-8 bytes: pwsh's Console.Out would encode it with the (shared, OEM on Windows) console
/// code page, and changing that code page would outlive the child.
pub(crate) const PWSH_PROFILE_CMD: &str =
    "$b = [Text.Encoding]::UTF8.GetBytes($PROFILE); $s = [Console]::OpenStandardOutput(); $s.Write($b, 0, $b.Length); $s.Flush()";

/// The facts of this process, its parent and `pwsh`, each read from the OS when asked.
pub(crate) struct OsShellEnvironment;

impl ShellEnvironment for OsShellEnvironment {
    fn current_executable(&self) -> Result<String, ShellIntegrationError> {
        stable_executable().map_err(environment_error)
    }

    fn parent_shell(&self) -> Option<String> {
        parent_exe()
    }

    /// The environment (non-UTF-8 names dropped, values lossy) and whether stdin is a tty, or a console on Windows.
    fn launch_facts(&self) -> LaunchFacts {
        let env: HashMap<String, String> = std::env::vars_os()
            .filter_map(|(k, v)| Some((k.into_string().ok()?, v.to_string_lossy().into_owned())))
            .collect();
        #[cfg(windows)]
        let stdin_ok = {
            use std::os::windows::io::AsRawHandle;
            is_console(std::io::stdin().as_raw_handle())
        };
        #[cfg(not(windows))]
        let stdin_ok = std::io::stdin().is_terminal();
        LaunchFacts::new(env, stdin_ok)
    }

    /// `$PROFILE` (CurrentUserCurrentHost) from `pwsh -NoLogo -NoProfile -Command …` (5 s timeout).
    fn pwsh_profile(&self) -> Result<PathBuf, ShellIntegrationError> {
        let out = output_with_timeout(
            Command::new("pwsh").args(["-NoLogo", "-NoProfile", "-Command", PWSH_PROFILE_CMD]),
            Duration::from_secs(5),
        );
        profile_from(out)
    }
}

/// The integration paths of this user and binary: home, then the data root of `data_directory`, then the executable,
/// then `ZDOTDIR`, then `XDG_CONFIG_HOME`; pwsh's `$PROFILE` is left to ask `pwsh`.
pub(crate) fn integration_paths_from(
    data_directory: &DataDirectory,
) -> Result<IntegrationPaths, ShellIntegrationError> {
    let home = user_home_from_env().map_err(environment_error)?;
    let var = |k: &str| {
        std::env::var_os(k)
            .filter(|v| !v.is_empty())
            .map(PathBuf::from)
    };
    let data_root = data_directory.root().map_err(environment_error)?;
    let binary = stable_executable().map_err(environment_error)?;
    let zdotdir = var("ZDOTDIR");
    let fish_config_dir = var("XDG_CONFIG_HOME")
        .unwrap_or_else(|| home.join(".config"))
        .join("fish");
    Ok(IntegrationPaths::new(
        home,
        data_root,
        binary,
        zdotdir,
        fish_config_dir,
        None,
    ))
}

/// The `$PROFILE` pwsh printed (trimmed), or why it is unknown.
fn profile_from(output: std::io::Result<String>) -> Result<PathBuf, ShellIntegrationError> {
    let out = output.map_err(|e| {
        ShellIntegrationError::Environment(format!(
            "could not ask pwsh for $PROFILE: {}",
            display_chain(&e)
        ))
    })?;
    match out.trim() {
        "" => Err(ShellIntegrationError::Environment(
            "pwsh printed an empty $PROFILE".to_string(),
        )),
        p => Ok(PathBuf::from(p)),
    }
}

/// The running binary by the path the shell finds it under, written into the init scripts and dotfiles (SPEC §5.5).
/// Not `exe_path()`: self-update has to replace the real file.
fn stable_executable() -> Result<String, ExecutablePathError> {
    let exe = std::env::current_exe()?;
    Ok(stable_exe_path(exe, std::env::var_os("PATH").as_deref())
        .to_string_lossy()
        .into_owned())
}

/// The first file named like `exe` in an absolute directory of `path`, when it is `exe` itself (a symlink to it, say),
/// else `exe`. Looks up `exe`'s own file name, not `which()`'s PATHEXT spellings (`tabsmith.EXE`).
// ponytail: first PATH hit only; Homebrew's bin/ link outlives the Cellar path that Linux reports and brew upgrade
// deletes. A copy reached only through a later PATH entry keeps its own path.
fn stable_exe_path(exe: PathBuf, path: Option<&OsStr>) -> PathBuf {
    let name = exe.file_name().unwrap_or_default();
    path.and_then(|path| {
        std::env::split_paths(path)
            .filter(|dir| dir.is_absolute())
            .map(|dir| dir.join(name))
            .find(|candidate| candidate.is_file())
    })
    .filter(|first| is_same_file(first, &exe))
    .unwrap_or(exe)
}

fn is_same_file(a: &Path, b: &Path) -> bool {
    match (std::fs::canonicalize(a), std::fs::canonicalize(b)) {
        (Ok(a), Ok(b)) => a == b,
        _ => false,
    }
}

/// A fact of the environment that could not be found out, with its causes rendered once (P3).
fn environment_error(error: impl Error + 'static) -> ShellIntegrationError {
    ShellIntegrationError::Environment(display_chain(&error))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::shared::domain::shared_error::SharedError;
    use crate::modules::shared::infrastructure::testing::temp_dir::TempDir;
    use std::io;

    /// Where Homebrew on Linux keeps tabsmith 1.0.0, the path `current_exe()` reports there.
    const CELLAR_BINARY: &str = "Cellar/tabsmith/1.0.0/bin/tabsmith";

    /// A file standing in for a tabsmith binary at `path`, its directory created.
    fn binary_at(path: PathBuf, contents: &str) -> PathBuf {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, contents).unwrap();
        path
    }

    #[test]
    fn should_keep_the_executable_when_no_path_directory_holds_a_tabsmith() {
        let temp_dir = TempDir::new("stable-exe-none-on-path");
        let exe = binary_at(temp_dir.path().join(CELLAR_BINARY), "tabsmith");
        let path = std::env::join_paths([temp_dir.path().join("bin")]).unwrap();

        let stable = stable_exe_path(exe.clone(), Some(&path));

        assert_eq!(stable, exe);
    }

    /// `.` on PATH would name a file that is only there from the directory setup ran in. Cargo runs the tests from
    /// the crate root, so `./Cargo.toml` is the same file as the one given.
    #[test]
    fn should_skip_a_relative_path_directory() {
        let exe = Path::new(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml");

        let stable = stable_exe_path(exe.clone(), Some(OsStr::new(".")));

        assert_eq!(stable, exe);
    }

    /// Homebrew's `bin/` link outlives the versioned Cellar path that Linux reports and `brew upgrade` deletes.
    #[cfg(unix)]
    #[test]
    fn should_name_the_executable_by_the_symlink_a_path_directory_holds_to_it() {
        let temp_dir = TempDir::new("stable-exe-symlink-on-path");
        let exe = binary_at(temp_dir.path().join(CELLAR_BINARY), "tabsmith");
        let link = temp_dir.path().join("bin").join("tabsmith");
        std::fs::create_dir_all(link.parent().unwrap()).unwrap();
        std::os::unix::fs::symlink(&exe, &link).unwrap();
        let path = std::env::join_paths([temp_dir.path().join("bin")]).unwrap();

        let stable = stable_exe_path(exe, Some(&path));

        assert_eq!(stable, link);
    }

    /// A dev build run by its path, or a Homebrew copy shadowed by the one-liner's.
    #[test]
    fn should_keep_the_executable_when_the_first_tabsmith_on_path_is_another_file() {
        let temp_dir = TempDir::new("stable-exe-other-on-path");
        let exe = binary_at(temp_dir.path().join(CELLAR_BINARY), "tabsmith 1.0.0");
        let other = binary_at(
            temp_dir.path().join(".local/bin/tabsmith"),
            "tabsmith 0.9.0",
        );
        let path = std::env::join_paths([other.parent().unwrap()]).unwrap();

        let stable = stable_exe_path(exe.clone(), Some(&path));

        assert_eq!(stable, exe);
    }

    #[cfg(unix)]
    #[test]
    fn should_report_a_pwsh_that_cannot_start_as_the_io_error_under_the_pwsh_context() {
        let missing_program = io::Error::from_raw_os_error(2);

        let error = profile_from(Err(missing_program)).unwrap_err();

        assert_eq!(
            error.to_string(),
            "could not ask pwsh for $PROFILE: No such file or directory (os error 2)"
        );
    }

    #[test]
    fn should_report_a_profile_of_only_whitespace_as_empty() {
        let printed = Ok(" \r\n".to_string());

        let error = profile_from(printed).unwrap_err();

        assert_eq!(error.to_string(), "pwsh printed an empty $PROFILE");
    }

    #[test]
    fn should_report_an_executable_it_could_not_locate_then_the_io_cause() {
        let unlocatable = ExecutablePathError::from(io::Error::from(io::ErrorKind::NotFound));

        let error = environment_error(unlocatable);

        assert_eq!(
            error.to_string(),
            "could not locate the tabsmith executable: entity not found"
        );
    }

    /// `output_with_timeout`'s own error (`process_runner.rs` pins the same text).
    #[test]
    fn should_report_a_pwsh_still_running_at_the_timeout_under_the_pwsh_context() {
        let timed_out = io::Error::new(io::ErrorKind::TimedOut, "timed out after 0s");

        let error = profile_from(Err(timed_out)).unwrap_err();

        assert_eq!(
            error.to_string(),
            "could not ask pwsh for $PROFILE: timed out after 0s"
        );
    }

    #[test]
    fn should_render_every_cause_under_the_pwsh_context_as_anyhow_does() {
        let nested = || {
            io::Error::other(ExecutablePathError::from(io::Error::from(
                io::ErrorKind::NotFound,
            )))
        };

        let error = profile_from(Err(nested())).unwrap_err();

        let anyhow_text = format!(
            "{:#}",
            anyhow::Error::from(nested()).context("could not ask pwsh for $PROFILE")
        );
        assert_eq!(error.to_string(), anyhow_text);
    }

    #[test]
    fn should_give_the_profile_pwsh_printed_without_its_line_ending() {
        let printed = Ok("/home/u/.config/powershell/Microsoft.PowerShell_profile.ps1\r\n".to_string());

        let profile = profile_from(printed).unwrap();

        assert_eq!(
            profile,
            PathBuf::from("/home/u/.config/powershell/Microsoft.PowerShell_profile.ps1")
        );
    }

    #[test]
    fn should_report_an_unknown_home_by_the_shared_error_text_alone() {
        let unknown = [SharedError::UserHomeNotFound, SharedError::HomeDirectoryNotFound];

        let messages = unknown.map(|e| environment_error(e).to_string());

        assert_eq!(
            messages,
            [
                "could not determine the home directory",
                "could not determine the home directory (set HOME or TABSMITH_HOME)",
            ]
        );
    }
}
