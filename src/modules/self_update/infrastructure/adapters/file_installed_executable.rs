use crate::modules::self_update::application::ports::installed_executable::InstalledExecutable;
use crate::modules::self_update::domain::self_update_error::SelfUpdateError;
use crate::modules::self_update::domain::value_objects::install_location::InstallLocation;
use crate::modules::shared::infrastructure::os::process_runner::output_with_timeout;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

/// How long the new binary may take to print its version; the first run of a new file can be slow while the OS
/// scans it.
const VERSION_CHECK_TIMEOUT: Duration = Duration::from_secs(30);

/// The tabsmith binary on disk that this process runs from. It is replaced by renaming a checked copy over it, never
/// by writing into it (decision 3): sessions still running keep the old file, and on macOS a signed binary
/// overwritten in place gets SIGKILLed.
// ponytail: not AtomicFile, whose methods rename right after the write; the version check has to run in between.
pub(crate) struct FileInstalledExecutable {
    executable: PathBuf,
    user_home: PathBuf,
}

impl FileInstalledExecutable {
    pub(crate) fn new(executable: PathBuf, user_home: PathBuf) -> Self {
        FileInstalledExecutable { executable, user_home }
    }

}

const STAGED_PREFIX: &str = ".tabsmith-update-";

/// How old a staged binary has to be to count as abandoned: far past a download and a version check, so a tab
/// updating at the same moment keeps its own.
const ABANDONED_AFTER: Duration = Duration::from_secs(10 * 60);

/// Next to the executable, so the final rename stays on one filesystem; named per process, so two tabs updating at
/// once never write the same file.
fn staged_path(executable: &Path) -> PathBuf {
    let name = format!("{STAGED_PREFIX}{}{}", std::process::id(), std::env::consts::EXE_SUFFIX);
    executable.with_file_name(name)
}

/// Staged binaries that an update stopped between staging and the rename (a closed tab, Ctrl-C) left next to the
/// executable, removed best effort: nothing else would ever delete them.
fn remove_abandoned_staged_files(executable: &Path) {
    let Some(entries) = executable.parent().and_then(|dir| fs::read_dir(dir).ok()) else {
        return;
    };
    for entry in entries.flatten() {
        let is_staged = entry.file_name().to_string_lossy().starts_with(STAGED_PREFIX);
        let age = entry.metadata().and_then(|metadata| metadata.modified()).ok().and_then(|modified| modified.elapsed().ok());
        if is_staged && age.is_some_and(|age| age >= ABANDONED_AFTER) {
            let _ = fs::remove_file(entry.path());
        }
    }
}

impl InstalledExecutable for FileInstalledExecutable {
    /// The binary and the native directory with every symlink resolved where they exist: Linux reports the running
    /// binary that way, and the home, `~/.local` or `~/.local/bin` may each be a link (a dotfiles tree, another disk).
    fn location(&self) -> InstallLocation {
        let native_dir = InstallLocation::native_dir(&self.user_home);
        InstallLocation::new(resolved(&self.executable), resolved(&native_dir))
    }

    /// The resolved file is replaced, the one `location` judged native: renaming over a symlink to it (macOS reports
    /// the binary as invoked) would leave the native install old and turn the link into a copy.
    fn replace(&self, bytes: &[u8], expected_version: &str) -> Result<(), SelfUpdateError> {
        let executable = resolved(&self.executable);
        remove_abandoned_staged_files(&executable);
        let staged = staged_path(&executable);
        let replaced = stage(&staged, bytes)
            .and_then(|()| check_version(&staged, expected_version))
            .and_then(|()| swap_into_place(&staged, &executable));
        if replaced.is_err() {
            let _ = fs::remove_file(&staged);
        }
        replaced
    }
}

fn resolved(path: &Path) -> PathBuf {
    fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
}

/// `bytes` at `path`, executable.
fn stage(path: &Path, bytes: &[u8]) -> Result<(), SelfUpdateError> {
    fs::write(path, bytes).map_err(io_failure(path))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o755)).map_err(io_failure(path))?;
    }
    Ok(())
}

/// The staged binary runs and prints `tabsmith <expected_version>`, so a truncated or wrong download never becomes
/// the binary every new terminal execs.
fn check_version(staged: &Path, expected_version: &str) -> Result<(), SelfUpdateError> {
    let expected = format!("tabsmith {expected_version}");
    let printed = output_with_timeout(Command::new(staged).arg("--version"), VERSION_CHECK_TIMEOUT)
        .map_err(io_failure(staged))?;
    let printed = printed.trim();
    if printed != expected {
        return Err(SelfUpdateError::InstallFailed(format!(
            "the new binary reports {printed:?}, not {expected}"
        )));
    }
    Ok(())
}

/// Unix: one rename; whoever runs the old file keeps its inode.
#[cfg(not(windows))]
fn swap_into_place(staged: &Path, executable: &Path) -> Result<(), SelfUpdateError> {
    fs::rename(staged, executable).map_err(io_failure(executable))
}

/// Windows lets a running image be renamed but not replaced or deleted: the running file steps aside as
/// `tabsmith.exe.<pid>.<nanos>.old`, the new one takes its name, and the old one comes back if that fails. Copies left
/// from earlier updates are deleted first, best effort (one still running stays).
#[cfg(windows)]
fn swap_into_place(staged: &Path, executable: &Path) -> Result<(), SelfUpdateError> {
    remove_old_copies(executable);
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|elapsed| elapsed.as_nanos())
        .unwrap_or_default();
    let old = old_copy_path(executable, std::process::id(), nanos);
    fs::rename(executable, &old).map_err(io_failure(executable))?;
    if let Err(error) = fs::rename(staged, executable) {
        let _ = fs::rename(&old, executable);
        return Err(io_failure(executable)(error));
    }
    Ok(())
}

/// `<executable>.<pid>.<nanos>.old`: unique even when Windows reuses a process id while an earlier copy still runs.
#[cfg(any(windows, test))]
fn old_copy_path(executable: &Path, pid: u32, nanos: u128) -> PathBuf {
    let file_name = executable.file_name().unwrap_or_default().to_string_lossy();
    executable.with_file_name(format!("{file_name}.{pid}.{nanos}.old"))
}

#[cfg(windows)]
fn remove_old_copies(executable: &Path) {
    let (Some(dir), Some(file_name)) = (executable.parent(), executable.file_name()) else {
        return;
    };
    let prefix = format!("{}.", file_name.to_string_lossy());
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        if name.starts_with(&prefix) && name.ends_with(".old") {
            let _ = fs::remove_file(entry.path());
        }
    }
}

fn io_failure(path: &Path) -> impl FnOnce(std::io::Error) -> SelfUpdateError + '_ {
    move |error| SelfUpdateError::InstallFailed(format!("{}: {error}", path.display()))
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use crate::modules::shared::infrastructure::testing::temp_dir::TempDir;
    use std::fs;

    const RELEASE_9_9_9: &[u8] = b"#!/bin/sh\necho 'tabsmith 9.9.9'\n";

    /// An old binary at `<temp>/home/.local/bin/tabsmith`, and its path.
    fn native_install(temp_dir: &TempDir) -> (FileInstalledExecutable, PathBuf) {
        native_install_under(&temp_dir.path().join("home"))
    }

    /// An old binary at `<home>/.local/bin/tabsmith`, and its path.
    fn native_install_under(home: &Path) -> (FileInstalledExecutable, PathBuf) {
        let executable = home.join(".local").join("bin").join("tabsmith");
        fs::create_dir_all(executable.parent().unwrap()).unwrap();
        fs::write(&executable, b"old binary").unwrap();
        (FileInstalledExecutable::new(executable.clone(), home.to_path_buf()), executable)
    }

    #[test]
    fn should_replace_the_executable_with_a_binary_that_reports_the_expected_version() {
        let temp_dir = TempDir::new("self-update-replace");
        let (installed, executable) = native_install(&temp_dir);

        installed.replace(RELEASE_9_9_9, "9.9.9").unwrap();

        assert_eq!(fs::read(&executable).unwrap(), RELEASE_9_9_9);
    }

    #[test]
    fn should_leave_the_executable_and_no_temp_file_when_the_binary_reports_another_version() {
        let temp_dir = TempDir::new("self-update-mismatch");
        let (installed, executable) = native_install(&temp_dir);

        let result = installed.replace(RELEASE_9_9_9, "1.0.0");

        let bin_dir: Vec<_> = fs::read_dir(executable.parent().unwrap())
            .unwrap()
            .map(|entry| entry.unwrap().file_name())
            .collect();
        assert_eq!(
            (result, fs::read(&executable).unwrap(), bin_dir),
            (
                Err(SelfUpdateError::InstallFailed(
                    "the new binary reports \"tabsmith 9.9.9\", not tabsmith 1.0.0".to_string()
                )),
                b"old binary".to_vec(),
                vec![std::ffi::OsString::from("tabsmith")]
            )
        );
    }

    /// Decision 3: a new inode. A hard link stands in for a session still running the old binary.
    #[test]
    fn should_leave_the_old_file_whole_for_sessions_still_running_it() {
        let temp_dir = TempDir::new("self-update-inode");
        let (installed, executable) = native_install(&temp_dir);
        let running_session = temp_dir.path().join("running-session");
        fs::hard_link(&executable, &running_session).unwrap();

        installed.replace(RELEASE_9_9_9, "9.9.9").unwrap();

        assert_eq!(fs::read(&running_session).unwrap(), b"old binary");
    }

    /// On Linux the running binary's path comes with symlinks resolved, so the home has to be resolved too.
    #[test]
    fn should_take_the_local_bin_of_a_symlinked_home_for_the_native_install() {
        let temp_dir = TempDir::new("self-update-symlinked-home");
        let real_home = temp_dir.path().join("real-home");
        let (_, executable) = native_install_under(&real_home);
        let home = temp_dir.path().join("home");
        std::os::unix::fs::symlink(&real_home, &home).unwrap();

        let location = FileInstalledExecutable::new(executable, home).location();

        assert!(location.is_native());
    }

    /// GNU stow folds `~/.local/bin` into a symlink to a dotfiles tree; the installer's `mv` follows it.
    #[test]
    fn should_take_a_symlinked_local_bin_for_the_native_directory() {
        let temp_dir = TempDir::new("self-update-symlinked-bin");
        let dotfiles_bin = temp_dir.path().join("dotfiles").join("bin");
        fs::create_dir_all(&dotfiles_bin).unwrap();
        let executable = dotfiles_bin.join("tabsmith");
        fs::write(&executable, b"old binary").unwrap();
        let home = temp_dir.path().join("home");
        fs::create_dir_all(home.join(".local")).unwrap();
        std::os::unix::fs::symlink(&dotfiles_bin, home.join(".local").join("bin")).unwrap();

        let location = FileInstalledExecutable::new(executable, home).location();

        assert!(location.is_native());
    }

    /// macOS reports the running binary as it was invoked, here through a link on PATH to the native install: the
    /// file judged native is the one replaced.
    #[test]
    fn should_replace_the_native_binary_a_symlink_points_at_and_keep_the_symlink() {
        let temp_dir = TempDir::new("self-update-through-link");
        let home = temp_dir.path().join("home");
        let (_, native) = native_install_under(&home);
        let path_dir = temp_dir.path().join("opt-homebrew-bin");
        fs::create_dir_all(&path_dir).unwrap();
        let link = path_dir.join("tabsmith");
        std::os::unix::fs::symlink(&native, &link).unwrap();

        FileInstalledExecutable::new(link.clone(), home).replace(RELEASE_9_9_9, "9.9.9").unwrap();

        let link_kept = fs::symlink_metadata(&link).unwrap().file_type().is_symlink();
        assert_eq!((fs::read(&native).unwrap(), link_kept), (RELEASE_9_9_9.to_vec(), true));
    }

    /// A tab closed (or `tabsmith update` interrupted) between staging and the rename leaves its staged binary behind.
    #[test]
    fn should_remove_a_staged_binary_left_behind_by_an_interrupted_update() {
        let temp_dir = TempDir::new("self-update-leftover");
        let (installed, executable) = native_install(&temp_dir);
        let leftover = executable.with_file_name(".tabsmith-update-4242");
        fs::write(&leftover, b"half a binary").unwrap();
        let two_hours_ago = std::time::SystemTime::now() - std::time::Duration::from_secs(2 * 60 * 60);
        fs::File::options().write(true).open(&leftover).unwrap().set_modified(two_hours_ago).unwrap();

        installed.replace(RELEASE_9_9_9, "9.9.9").unwrap();

        assert!(!leftover.exists());
    }

    /// Two tabs may update at the same moment: the other one's staged binary is not abandoned yet.
    #[test]
    fn should_keep_the_staged_binary_of_an_update_still_running_in_another_tab() {
        let temp_dir = TempDir::new("self-update-other-tab");
        let (installed, executable) = native_install(&temp_dir);
        let other_tab = executable.with_file_name(".tabsmith-update-4242");
        fs::write(&other_tab, RELEASE_9_9_9).unwrap();

        installed.replace(RELEASE_9_9_9, "9.9.9").unwrap();

        assert!(other_tab.exists());
    }

    /// Windows reuses process ids, and an `.old` copy from an earlier update may still be running in another tab.
    #[test]
    fn should_name_the_old_copy_after_the_process_and_the_moment() {
        let executable = Path::new("/home/ana/.local/bin/tabsmith.exe");

        let old_copy = old_copy_path(executable, 4242, 1_700_000_000_123_456_789);

        assert_eq!(old_copy, Path::new("/home/ana/.local/bin/tabsmith.exe.4242.1700000000123456789.old"));
    }
}
