//! The diagnostics module's ports over the shell integrations and the specs, wired here because only bootstrap may
//! join modules.
use crate::diagnostics::application::ports::bundled_specs_probe::BundledSpecsProbe;
use crate::diagnostics::application::ports::check_fixer::CheckFixer;
use crate::diagnostics::application::ports::integration_probe::IntegrationProbe;
use crate::diagnostics::domain::value_objects::check_fix::CheckFix;
use crate::diagnostics::infrastructure::adapters::file_repairs::FileRepairs;
use crate::shared::domain::value_objects::data_directory::DataDirectory;
use crate::shared::domain::value_objects::shell::Shell;
use crate::shared::infrastructure::os::executable_lookup::which;
use crate::shell_integration::application::integration_installer::IntegrationInstaller;
use crate::shell_integration::domain::entities::integration::Integration;
use crate::shell_integration::domain::repositories::restart_reminder_repository::RestartReminderRepository;
use crate::shell_integration::domain::shell_integration_error::ShellIntegrationError;
use crate::shell_integration::domain::value_objects::integration_paths::IntegrationPaths;
use crate::shell_integration::domain::value_objects::integration_phase::IntegrationPhase;
use crate::shell_integration::infrastructure::adapters::os_shell_environment::integration_paths_from;
use crate::shell_integration::infrastructure::cli::integrations_command::integration_failures_error;
use crate::specs::application::install_spec_bundle_use_case::InstallSpecBundleUseCase;
use crate::specs::infrastructure::adapters::embedded_spec_bundle::version;
use std::path::{Path, PathBuf};
use std::rc::Rc;

/// doctor's questions about the shell integrations, answered by the shell integrations' installer.
pub(crate) struct IntegrationProbeBridge {
    installer: Rc<IntegrationInstaller>,
    data_directory: DataDirectory,
}

impl IntegrationProbeBridge {
    pub(crate) fn new(installer: Rc<IntegrationInstaller>, data_directory: DataDirectory) -> Self {
        IntegrationProbeBridge {
            installer,
            data_directory,
        }
    }

    /// The script files and fish `conf.d` files of the current integrations of every shell on PATH. Not every file
    /// in `~/.tabsmith/shell`: one of a dotfile deleted since (or of a shell gone from PATH) is sourced by nothing,
    /// and install, the fix, would never rewrite it.
    fn script_files(&self, paths: &IntegrationPaths) -> Vec<(Shell, PathBuf)> {
        // pwsh's script files do not depend on $PROFILE: no second `pwsh` spawn to ask for it.
        let paths = &paths.clone().with_pwsh_profile(
            paths
                .pwsh_profile()
                .map(Path::to_path_buf)
                .unwrap_or_default(),
        );
        let mut files = Vec::new();
        for shell in Shell::ALL.into_iter().filter(|s| which(s.name()).is_some()) {
            for i in self
                .installer
                .integrations(paths, shell)
                .unwrap_or_default()
            {
                match i.whole_file_phase() {
                    Some(_) => files.push((shell, i.path().to_path_buf())),
                    None => files.extend(
                        [IntegrationPhase::Pre, IntegrationPhase::Post]
                            .map(|w| (shell, i.script(paths, w).path().to_path_buf())),
                    ),
                }
            }
        }
        files
    }
}

impl IntegrationProbe for IntegrationProbeBridge {
    fn locate_paths(&self) -> Result<IntegrationPaths, ShellIntegrationError> {
        integration_paths_from(&self.data_directory)
    }

    fn list(
        &self,
        paths: &IntegrationPaths,
        shell: Shell,
    ) -> Result<Vec<Integration>, ShellIntegrationError> {
        self.installer.integrations(paths, shell)
    }

    fn check(
        &self,
        paths: &IntegrationPaths,
        integration: &Integration,
    ) -> Result<(), ShellIntegrationError> {
        self.installer.check(paths, integration)
    }

    fn script_binaries(&self, paths: &IntegrationPaths) -> Vec<(Shell, String)> {
        self.script_files(paths)
            .into_iter()
            .filter_map(|(shell, p)| {
                // ponytail: raw text between the first two quotes; paths containing `'` are not unescaped.
                let bin = std::fs::read_to_string(p)
                    .ok()?
                    .split('\'')
                    .nth(1)?
                    .to_string();
                Some((shell, bin))
            })
            .collect()
    }
}

/// doctor's question whether the completion specs of this version are on disk, answered from the file system.
pub(crate) struct BundledSpecsBridge;

impl BundledSpecsProbe for BundledSpecsBridge {
    fn is_extracted(&self, data_root: &Path) -> bool {
        data_root
            .join("specs")
            .join(version())
            .join("index.json")
            .is_file()
    }
}

/// doctor's repairs: the shell integrations' installer, the specs' bundle and the file repairs, and the restart
/// reminder after the fixes whose domain rule says so.
pub(crate) struct CheckFixerBridge {
    installer: Rc<IntegrationInstaller>,
    restart_reminder: Rc<dyn RestartReminderRepository>,
    spec_bundle: InstallSpecBundleUseCase,
    repairs: FileRepairs,
}

impl CheckFixerBridge {
    pub(crate) fn new(
        installer: Rc<IntegrationInstaller>,
        restart_reminder: Rc<dyn RestartReminderRepository>,
        spec_bundle: InstallSpecBundleUseCase,
        repairs: FileRepairs,
    ) -> Self {
        CheckFixerBridge {
            installer,
            restart_reminder,
            spec_bundle,
            repairs,
        }
    }

    fn apply(&self, fix: &CheckFix, paths: &IntegrationPaths) -> Result<(), anyhow::Error> {
        match fix {
            CheckFix::CreateDataDirectory(path) => self
                .repairs
                .create_data_directory(path)
                .map_err(anyhow::Error::from),
            CheckFix::InstallIntegration(integration) => self
                .installer
                .install(paths, integration)
                .map_err(anyhow::Error::from),
            CheckFix::InstallMissingIntegrations(shells) => self
                .installer
                .install_missing(paths, shells)
                .map(|_installed_something| ())
                .map_err(integration_failures_error),
            CheckFix::ResetSettings(path) => self
                .repairs
                .reset_settings(path)
                .map_err(anyhow::Error::from),
            CheckFix::InstallSpecBundle => self.spec_bundle.execute().map_err(anyhow::Error::from),
        }
    }
}

impl CheckFixer for CheckFixerBridge {
    fn fix(&self, fix: &CheckFix, paths: &IntegrationPaths) -> Result<(), String> {
        self.apply(fix, paths).map_err(rendered)?;
        if fix.marks_restart_reminder() {
            self.restart_reminder.save_pending();
        }
        Ok(())
    }
}

/// The error with its whole chain, as anyhow's `{:#}` prints it: an io error as its bare Display, a
/// `ShellIntegrationError` without its source.
fn rendered(error: anyhow::Error) -> String {
    format!("{error:#}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shared::application::ports::clock::FixedClock;
    use crate::shared::infrastructure::testing::temp_dir::TempDir;
    use crate::shell_integration::domain::repositories::restart_reminder_repository::InMemoryRestartReminderRepository;
    use crate::shell_integration::infrastructure::adapters::file_backup_repository::FileBackupRepository;
    use crate::shell_integration::infrastructure::adapters::file_dotfile_repository::FileDotfileRepository;
    use crate::shell_integration::infrastructure::adapters::os_shell_environment::OsShellEnvironment;
    use crate::specs::application::ports::spec_bundle::RecordingSpecBundle;
    use crate::shell_integration::infrastructure::testing::integration_fixtures::install_dotfiles;
    use crate::specs::domain::spec_error::SpecError;
    use std::sync::Arc;

    fn installer() -> Rc<IntegrationInstaller> {
        Rc::new(IntegrationInstaller::new(
            Rc::new(FileDotfileRepository),
            Rc::new(FileBackupRepository),
            Arc::new(FixedClock::new(1_700_000_000_000)),
            Rc::new(OsShellEnvironment),
        ))
    }

    /// A user whose home holds an empty `.bash_profile`, with the tabsmith binary at `binary`.
    fn paths_with_bash_profile(dir: &Path, binary: &str) -> IntegrationPaths {
        let home = dir.join("home");
        std::fs::create_dir_all(&home).unwrap();
        std::fs::write(home.join(".bash_profile"), "").unwrap();
        IntegrationPaths::new(
            home.clone(),
            home.join(".tabsmith"),
            binary.into(),
            None,
            home.join(".config/fish"),
            Some(home.join("profile.ps1")),
        )
    }

    fn probe() -> IntegrationProbeBridge {
        IntegrationProbeBridge::new(installer(), DataDirectory::new(None, None))
    }

    fn bash_profile(paths: &IntegrationPaths) -> Integration {
        Integration::dotfile(Shell::Bash, paths.home().join(".bash_profile"))
    }

    /// The bridge over a recording spec bundle; the reminder and the bundle stay with the test.
    fn fixer(
        reminder: &Rc<InMemoryRestartReminderRepository>,
        bundle: &Rc<RecordingSpecBundle>,
    ) -> CheckFixerBridge {
        CheckFixerBridge::new(
            installer(),
            reminder.clone(),
            InstallSpecBundleUseCase::new(bundle.clone()),
            FileRepairs::new(Arc::new(FixedClock::new(1_700_000_000_000))),
        )
    }

    fn fixer_with_reminder(reminder: &Rc<InMemoryRestartReminderRepository>) -> CheckFixerBridge {
        fixer(reminder, &Rc::new(RecordingSpecBundle::answering(Ok(()))))
    }

    /// A path that cannot be a directory or a file: below a regular file.
    fn path_below_a_file(dir: &Path) -> PathBuf {
        let file = dir.join("occupied");
        std::fs::write(&file, "x").unwrap();
        file.join("below")
    }

    /// A user whose data root is a regular file: nothing can be written below it.
    fn paths_with_occupied_data_root(dir: &Path) -> IntegrationPaths {
        let paths = paths_with_bash_profile(dir, "/new/tabsmith");
        std::fs::write(paths.data_root(), "x").unwrap();
        paths
    }

    fn install_bash_profile(paths: &IntegrationPaths) {
        installer().install(paths, &bash_profile(paths)).unwrap();
    }

    fn script_of(paths: &IntegrationPaths, phase: IntegrationPhase) -> PathBuf {
        bash_profile(paths)
            .script(paths, phase)
            .path()
            .to_path_buf()
    }

    /// After the binary moved, the fix (reinstall) makes the check pass even when a dotfile was deleted since
    /// setup: its orphaned script files are no longer checked.
    #[test]
    fn doctor_script_bins_skip_orphans() {
        let temp_dir = TempDir::new("doctor-orphans");
        let dir = temp_dir.path().to_path_buf();
        let home = dir.join("home");
        std::fs::create_dir_all(&home).unwrap();
        std::fs::write(home.join(".bash_profile"), "").unwrap();
        let c = IntegrationPaths::new(
            home.clone(),
            home.join(".tabsmith"),
            "/old/tabsmith".into(),
            None,
            home.join(".config/fish"),
            Some(home.join("profile.ps1")),
        );
        install_dotfiles(&c, &[Shell::Bash]).unwrap();
        std::fs::remove_file(home.join(".bash_profile")).unwrap();
        let moved = c.with_binary("/new/tabsmith".into());
        install_dotfiles(&moved, &[Shell::Bash]).unwrap(); // the check's fix
        assert!(moved.data_root().join("shell/bash_profile.pre.bash").exists());
        if which("bash").is_some() {
            let bins = probe().script_binaries(&moved);
            assert!(
                !bins.is_empty() && bins.iter().all(|(_, b)| b == "/new/tabsmith"),
                "{bins:?}"
            );
        }
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn should_give_the_text_between_the_first_two_quotes_of_each_installed_script() {
        let temp_dir = TempDir::new("bridge-bins");
        let paths = paths_with_bash_profile(temp_dir.path(), "/new/tabsmith");
        install_bash_profile(&paths);

        let binaries = probe().script_binaries(&paths);

        if which("bash").is_some() {
            assert_eq!(
                binaries,
                vec![
                    (Shell::Bash, "/new/tabsmith".to_string()),
                    (Shell::Bash, "/new/tabsmith".to_string())
                ]
            );
        }
    }

    #[test]
    fn should_skip_a_script_file_that_cannot_be_read() {
        let temp_dir = TempDir::new("bridge-bins-unreadable");
        let paths = paths_with_bash_profile(temp_dir.path(), "/new/tabsmith");
        install_bash_profile(&paths);
        std::fs::remove_file(script_of(&paths, IntegrationPhase::Pre)).unwrap();

        let binaries = probe().script_binaries(&paths);

        if which("bash").is_some() {
            assert_eq!(binaries, vec![(Shell::Bash, "/new/tabsmith".to_string())]);
        }
    }

    #[test]
    fn should_skip_a_script_file_without_quotes() {
        let temp_dir = TempDir::new("bridge-bins-quoteless");
        let paths = paths_with_bash_profile(temp_dir.path(), "/new/tabsmith");
        install_bash_profile(&paths);
        std::fs::write(script_of(&paths, IntegrationPhase::Post), "no quotes here").unwrap();

        let binaries = probe().script_binaries(&paths);

        if which("bash").is_some() {
            assert_eq!(binaries, vec![(Shell::Bash, "/new/tabsmith".to_string())]);
        }
    }

    #[test]
    fn should_read_a_binary_path_containing_a_quote_only_up_to_that_quote() {
        let temp_dir = TempDir::new("bridge-bins-inner-quote");
        let paths = paths_with_bash_profile(temp_dir.path(), "/new/tabsmith");
        install_bash_profile(&paths);
        std::fs::write(
            script_of(&paths, IntegrationPhase::Pre),
            "exec '/opt/it's/tabsmith' \"$@\"",
        )
        .unwrap();

        let binaries = probe().script_binaries(&paths);

        if which("bash").is_some() {
            assert_eq!(
                binaries,
                vec![
                    (Shell::Bash, "/opt/it".to_string()),
                    (Shell::Bash, "/new/tabsmith".to_string())
                ]
            );
        }
    }

    #[test]
    fn should_read_only_the_scripts_of_shells_found_on_path() {
        let temp_dir = TempDir::new("bridge-bins-on-path");
        let paths = paths_with_bash_profile(temp_dir.path(), "/new/tabsmith");
        let everywhere = [Shell::Bash, Shell::Zsh, Shell::Fish];
        installer().install_missing(&paths, &everywhere).unwrap();

        let binaries = probe().script_binaries(&paths);

        let found_on_path = |shell: &Shell| which(shell.name()).is_some();
        assert_eq!(
            binaries
                .iter()
                .filter(|(shell, _)| !found_on_path(shell))
                .count(),
            0
        );
    }

    #[test]
    fn should_list_the_integrations_of_a_shell_like_the_installer() {
        let temp_dir = TempDir::new("bridge-list");
        let paths = paths_with_bash_profile(temp_dir.path(), "/new/tabsmith");

        let listed = probe().list(&paths, Shell::Bash).unwrap();

        assert_eq!(
            listed,
            installer().integrations(&paths, Shell::Bash).unwrap()
        );
    }

    #[test]
    fn should_check_an_integration_like_the_installer() {
        let temp_dir = TempDir::new("bridge-check");
        let paths = paths_with_bash_profile(temp_dir.path(), "/new/tabsmith");

        let checked = probe().check(&paths, &bash_profile(&paths));

        assert_eq!(
            checked.unwrap_err().to_string(),
            installer()
                .check(&paths, &bash_profile(&paths))
                .unwrap_err()
                .to_string()
        );
    }

    #[test]
    fn should_mark_the_restart_reminder_after_installing_an_integration() {
        let temp_dir = TempDir::new("bridge-fix-install");
        let paths = paths_with_bash_profile(temp_dir.path(), "/new/tabsmith");
        let reminder = Rc::new(InMemoryRestartReminderRepository::default());

        let fixed = fixer_with_reminder(&reminder)
            .fix(&CheckFix::InstallIntegration(bash_profile(&paths)), &paths);

        assert_eq!((fixed, reminder.is_pending()), (Ok(()), true));
    }

    #[test]
    fn should_install_the_integration_it_is_asked_to_fix() {
        let temp_dir = TempDir::new("bridge-fix-installs");
        let paths = paths_with_bash_profile(temp_dir.path(), "/new/tabsmith");
        let reminder = Rc::new(InMemoryRestartReminderRepository::default());

        let _ = fixer_with_reminder(&reminder)
            .fix(&CheckFix::InstallIntegration(bash_profile(&paths)), &paths);

        assert!(installer().check(&paths, &bash_profile(&paths)).is_ok());
    }

    #[test]
    fn should_not_mark_the_restart_reminder_when_installing_an_integration_fails() {
        let temp_dir = TempDir::new("bridge-fix-install-fails");
        let paths = paths_with_occupied_data_root(temp_dir.path());
        let reminder = Rc::new(InMemoryRestartReminderRepository::default());

        let fixed = fixer_with_reminder(&reminder)
            .fix(&CheckFix::InstallIntegration(bash_profile(&paths)), &paths);

        assert_eq!((fixed.is_err(), reminder.is_pending()), (true, false));
    }

    #[test]
    fn should_fail_to_install_an_integration_with_the_integration_error_text() {
        let temp_dir = TempDir::new("bridge-fix-install-text");
        let paths = paths_with_occupied_data_root(temp_dir.path());
        let reminder = Rc::new(InMemoryRestartReminderRepository::default());
        let expected = installer()
            .install(&paths, &bash_profile(&paths))
            .unwrap_err();

        let fixed = fixer_with_reminder(&reminder)
            .fix(&CheckFix::InstallIntegration(bash_profile(&paths)), &paths);

        assert_eq!(fixed, Err(expected.to_string()));
    }

    #[test]
    fn should_mark_the_restart_reminder_when_the_missing_integrations_were_installed() {
        let temp_dir = TempDir::new("bridge-fix-missing");
        let paths = paths_with_bash_profile(temp_dir.path(), "/new/tabsmith");
        let reminder = Rc::new(InMemoryRestartReminderRepository::default());

        let fixed = fixer_with_reminder(&reminder).fix(
            &CheckFix::InstallMissingIntegrations(vec![Shell::Bash]),
            &paths,
        );

        assert_eq!((fixed, reminder.is_pending()), (Ok(()), true));
    }

    #[test]
    fn should_mark_the_restart_reminder_even_when_no_integration_was_missing() {
        let temp_dir = TempDir::new("bridge-fix-missing-none");
        let paths = paths_with_bash_profile(temp_dir.path(), "/new/tabsmith");
        installer().install_missing(&paths, &[Shell::Bash]).unwrap();
        let reminder = Rc::new(InMemoryRestartReminderRepository::default());

        let fixed = fixer_with_reminder(&reminder).fix(
            &CheckFix::InstallMissingIntegrations(vec![Shell::Bash]),
            &paths,
        );

        assert_eq!((fixed, reminder.is_pending()), (Ok(()), true));
    }

    #[test]
    fn should_fail_to_install_the_missing_integrations_with_a_blank_paragraph_first() {
        let temp_dir = TempDir::new("bridge-fix-missing-fails");
        let paths = paths_with_occupied_data_root(temp_dir.path());
        let reminder = Rc::new(InMemoryRestartReminderRepository::default());

        let fixed = fixer_with_reminder(&reminder).fix(
            &CheckFix::InstallMissingIntegrations(vec![Shell::Bash]),
            &paths,
        );

        assert_eq!(
            (
                fixed.unwrap_err().starts_with("\n\n"),
                reminder.is_pending()
            ),
            (true, false)
        );
    }

    #[test]
    fn should_create_the_data_directory() {
        let temp_dir = TempDir::new("bridge-fix-create");
        let paths = paths_with_bash_profile(temp_dir.path(), "/new/tabsmith");
        let data_root = temp_dir.path().join("created/below");
        let reminder = Rc::new(InMemoryRestartReminderRepository::default());

        let fixed = fixer_with_reminder(&reminder)
            .fix(&CheckFix::CreateDataDirectory(data_root.clone()), &paths);

        assert_eq!(
            (fixed, data_root.is_dir(), reminder.is_pending()),
            (Ok(()), true, false)
        );
    }

    #[test]
    fn should_fail_to_create_the_data_directory_with_the_bare_io_text() {
        let temp_dir = TempDir::new("bridge-fix-create-fails");
        let paths = paths_with_bash_profile(temp_dir.path(), "/new/tabsmith");
        let occupied = path_below_a_file(temp_dir.path());
        let bare = std::fs::create_dir_all(&occupied).unwrap_err().to_string();
        let reminder = Rc::new(InMemoryRestartReminderRepository::default());

        let fixed =
            fixer_with_reminder(&reminder).fix(&CheckFix::CreateDataDirectory(occupied), &paths);

        assert_eq!((fixed, reminder.is_pending()), (Err(bare), false));
    }

    #[test]
    fn should_reset_the_settings_keeping_the_old_bytes_named_after_the_clock() {
        let temp_dir = TempDir::new("bridge-fix-reset");
        let paths = paths_with_bash_profile(temp_dir.path(), "/new/tabsmith");
        let settings = temp_dir.path().join("settings.json");
        std::fs::write(&settings, "{oops").unwrap();
        let reminder = Rc::new(InMemoryRestartReminderRepository::default());

        let fixed =
            fixer_with_reminder(&reminder).fix(&CheckFix::ResetSettings(settings.clone()), &paths);

        let backup = temp_dir.path().join("settings.json.bak-1700000000");
        assert_eq!(
            (
                fixed,
                std::fs::read_to_string(&settings).unwrap(),
                std::fs::read_to_string(backup).unwrap(),
                reminder.is_pending()
            ),
            (Ok(()), "{}\n".to_string(), "{oops".to_string(), false)
        );
    }

    #[test]
    fn should_fail_to_reset_the_settings_with_the_bare_io_text() {
        let temp_dir = TempDir::new("bridge-fix-reset-fails");
        let paths = paths_with_bash_profile(temp_dir.path(), "/new/tabsmith");
        let missing = temp_dir.path().join("missing.json");
        let bare = std::fs::copy(&missing, temp_dir.path().join("copy"))
            .unwrap_err()
            .to_string();
        let reminder = Rc::new(InMemoryRestartReminderRepository::default());

        let fixed = fixer_with_reminder(&reminder).fix(&CheckFix::ResetSettings(missing), &paths);

        assert_eq!((fixed, reminder.is_pending()), (Err(bare), false));
    }

    #[test]
    fn should_install_the_spec_bundle_without_marking_the_restart_reminder() {
        let temp_dir = TempDir::new("bridge-fix-specs");
        let paths = paths_with_bash_profile(temp_dir.path(), "/new/tabsmith");
        let reminder = Rc::new(InMemoryRestartReminderRepository::default());
        let bundle = Rc::new(RecordingSpecBundle::answering(Ok(())));

        let fixed = fixer(&reminder, &bundle).fix(&CheckFix::InstallSpecBundle, &paths);

        assert_eq!(
            (fixed, bundle.installs(), reminder.is_pending()),
            (Ok(()), 1, false)
        );
    }

    #[test]
    fn should_fail_to_install_the_spec_bundle_with_the_spec_error_text() {
        let temp_dir = TempDir::new("bridge-fix-specs-fails");
        let paths = paths_with_bash_profile(temp_dir.path(), "/new/tabsmith");
        let reminder = Rc::new(InMemoryRestartReminderRepository::default());
        let failure =
            SpecError::BundleUnavailable("creating /home/u/.tabsmith/specs: disk full".into());
        let bundle = Rc::new(RecordingSpecBundle::answering(Err(failure)));

        let fixed = fixer(&reminder, &bundle).fix(&CheckFix::InstallSpecBundle, &paths);

        assert_eq!(
            (fixed, reminder.is_pending()),
            (
                Err("creating /home/u/.tabsmith/specs: disk full".to_string()),
                false
            )
        );
    }

    #[test]
    fn should_not_find_the_bundled_specs_in_an_empty_data_root() {
        let temp_dir = TempDir::new("bridge-specs-empty");

        let extracted = BundledSpecsBridge.is_extracted(temp_dir.path());

        assert!(!extracted);
    }

    #[test]
    fn should_find_the_bundled_specs_once_the_index_of_this_version_exists() {
        let temp_dir = TempDir::new("bridge-specs-extracted");
        let version_dir = temp_dir.path().join("specs").join(version());
        std::fs::create_dir_all(&version_dir).unwrap();
        std::fs::write(version_dir.join("index.json"), "{}").unwrap();

        let extracted = BundledSpecsBridge.is_extracted(temp_dir.path());

        assert!(extracted);
    }
}
