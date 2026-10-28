use crate::shared::application::ports::clock::Clock;
use crate::shared::domain::value_objects::shell::Shell;
use crate::shell_integration::application::ports::shell_environment::ShellEnvironment;
use crate::shell_integration::domain::entities::integration::Integration;
use crate::shell_integration::domain::repositories::backup_repository::BackupRepository;
use crate::shell_integration::domain::repositories::dotfile_repository::DotfileRepository;
use crate::shell_integration::domain::services::integration_catalog;
use crate::shell_integration::domain::shell_integration_error::ShellIntegrationError;
use crate::shell_integration::domain::value_objects::dotfile::Dotfile;
use crate::shell_integration::domain::value_objects::integration_paths::IntegrationPaths;
use crate::shell_integration::domain::value_objects::integration_phase::IntegrationPhase;
use std::rc::Rc;
use std::sync::Arc;

/// Lists, checks, installs and uninstalls the integrations of a shell through the dotfile and backup repositories
/// (SPEC §5.5), and runs one of those operations over several shells. Shared by the use cases; never marks the
/// restart reminder.
pub(crate) struct IntegrationInstaller {
    dotfiles: Rc<dyn DotfileRepository>,
    backups: Rc<dyn BackupRepository>,
    clock: Arc<dyn Clock + Send + Sync>,
    environment: Rc<dyn ShellEnvironment>,
}

impl IntegrationInstaller {
    pub(crate) fn new(
        dotfiles: Rc<dyn DotfileRepository>,
        backups: Rc<dyn BackupRepository>,
        clock: Arc<dyn Clock + Send + Sync>,
        environment: Rc<dyn ShellEnvironment>,
    ) -> Self {
        IntegrationInstaller {
            dotfiles,
            backups,
            clock,
            environment,
        }
    }

    /// Integrations of `shell` (SPEC §5.5 table), asking `pwsh` for `$PROFILE` when the paths do not know it.
    pub(crate) fn integrations(
        &self,
        paths: &IntegrationPaths,
        shell: Shell,
    ) -> Result<Vec<Integration>, ShellIntegrationError> {
        integration_catalog::integrations(
            paths,
            shell,
            |p| self.dotfiles.exists(p),
            || self.environment.pwsh_profile(),
        )
    }

    /// SPEC §5.5 `is_installed()`: the first failing reason.
    pub(crate) fn check(
        &self,
        paths: &IntegrationPaths,
        integration: &Integration,
    ) -> Result<(), ShellIntegrationError> {
        if let Some(script) = integration.whole_file_script(paths) {
            return script.check(&self.dotfiles.read(script.path())?);
        }
        let dotfile = Dotfile::new(self.dotfiles.read(integration.path())?);
        if dotfile.is_opted_out() {
            return Ok(());
        }
        let lines = dotfile.sourced_lines();
        for (when, at, actual) in [
            (IntegrationPhase::Pre, "first", lines.first()),
            (IntegrationPhase::Post, "last", lines.last()),
        ] {
            let src = integration.source_line(paths, when);
            if actual.copied() != Some(src.as_str()) {
                let msg = format!(
                    "{} does not source {} integration",
                    integration.path().display(),
                    when.name()
                );
                return Err(if lines.contains(&src.as_str()) {
                    ShellIntegrationError::Improper(format!("{msg} {at}"))
                } else {
                    ShellIntegrationError::NotInstalled(msg)
                });
            }
            let script = integration.script(paths, when);
            script.check(&self.dotfiles.read(script.path())?)?;
        }
        Ok(())
    }

    /// Write both `~/.tabsmith/shell/` script files of a dotfile integration.
    pub(crate) fn write_scripts(
        &self,
        paths: &IntegrationPaths,
        integration: &Integration,
    ) -> Result<(), ShellIntegrationError> {
        for when in [IntegrationPhase::Pre, IntegrationPhase::Post] {
            let script = integration.script(paths, when);
            self.dotfiles.write(script.path(), script.contents())?;
        }
        Ok(())
    }

    /// SPEC §5.5 uninstall: drop our exact lines, then delete the script files (missing is fine).
    pub(crate) fn uninstall(
        &self,
        paths: &IntegrationPaths,
        integration: &Integration,
    ) -> Result<(), ShellIntegrationError> {
        if integration.whole_file_phase().is_some() {
            return self.dotfiles.remove(integration.path());
        }
        if self.dotfiles.exists(integration.path()) {
            let dotfile = Dotfile::new(self.dotfiles.read(integration.path())?);
            let kept = dotfile.without_blocks(
                &integration.source_line(paths, IntegrationPhase::Pre),
                &integration.source_line(paths, IntegrationPhase::Post),
            );
            if kept.text() != dotfile.text() {
                self.dotfiles.write(integration.path(), kept.text())?;
            }
        }
        self.dotfiles.remove(integration.script(paths, IntegrationPhase::Pre).path())?;
        self.dotfiles.remove(integration.script(paths, IntegrationPhase::Post).path())
    }

    /// SPEC §5.5 install (idempotent); an existing dotfile is first copied to `<data>/backups/<unix-secs>/`.
    pub(crate) fn install(
        &self,
        paths: &IntegrationPaths,
        integration: &Integration,
    ) -> Result<(), ShellIntegrationError> {
        if self.check(paths, integration).is_ok() {
            return Ok(());
        }
        if let Some(script) = integration.whole_file_script(paths) {
            return self.dotfiles.write(script.path(), script.contents());
        }
        let contents = if self.dotfiles.exists(integration.path()) {
            let dir = paths.data_root().join("backups").join(self.clock.now_secs().to_string());
            self.backups.back_up(integration, &dir)?;
            self.uninstall(paths, integration)?;
            self.dotfiles.read(integration.path())?
        } else {
            String::new()
        };
        self.write_scripts(paths, integration)?;
        let dotfile = Dotfile::new(contents);
        let new = dotfile.with_blocks(
            &integration.source_line(paths, IntegrationPhase::Pre),
            &integration.source_line(paths, IntegrationPhase::Post),
        );
        if new.text() != dotfile.text() {
            self.dotfiles.write(integration.path(), new.text())?;
        }
        Ok(())
    }

    /// Anything uninstall would remove is on disk: also true where `is_installed` fails (a moved binary, an edited
    /// script, lines appended after the post block).
    pub(crate) fn has_traces(&self, paths: &IntegrationPaths, integration: &Integration) -> bool {
        if integration.whole_file_phase().is_some() {
            return self.dotfiles.exists(integration.path());
        }
        [IntegrationPhase::Pre, IntegrationPhase::Post]
            .into_iter()
            .any(|w| self.dotfiles.exists(integration.script(paths, w).path()))
            || self.dotfiles.read(integration.path()).is_ok_and(|t| {
                Dotfile::new(t).has_own_line(
                    &integration.source_line(paths, IntegrationPhase::Pre),
                    &integration.source_line(paths, IntegrationPhase::Post),
                )
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shared::application::ports::clock::FixedClock;
    use crate::shell_integration::application::ports::shell_environment::RecordingShellEnvironment;
    use crate::shell_integration::domain::repositories::backup_repository::InMemoryBackupRepository;
    use crate::shell_integration::domain::repositories::dotfile_repository::InMemoryDotfileRepository;
    use crate::shell_integration::domain::value_objects::dotfile::{POST_MARKER, PRE_MARKER};
    use std::path::{Path, PathBuf};

    /// The installer over in-memory dotfiles and backups, a clock fixed at 1_700_000_123_999 ms and a recording
    /// shell environment, with the doubles kept for the asserts.
    struct TestBed {
        installer: IntegrationInstaller,
        dotfiles: Rc<InMemoryDotfileRepository>,
        backups: Rc<InMemoryBackupRepository>,
        environment: Rc<RecordingShellEnvironment>,
    }

    fn test_bed() -> TestBed {
        test_bed_with(RecordingShellEnvironment::default())
    }

    fn test_bed_with(environment: RecordingShellEnvironment) -> TestBed {
        let dotfiles = Rc::new(InMemoryDotfileRepository::default());
        let backups = Rc::new(InMemoryBackupRepository::default());
        let environment = Rc::new(environment);
        let installer = IntegrationInstaller::new(
            dotfiles.clone(),
            backups.clone(),
            Arc::new(FixedClock::new(1_700_000_123_999)),
            environment.clone(),
        );
        TestBed {
            installer,
            dotfiles,
            backups,
            environment,
        }
    }

    /// No pwsh `$PROFILE` known, so listing pwsh asks the environment.
    fn paths() -> IntegrationPaths {
        IntegrationPaths::new(
            PathBuf::from("/home/u"),
            PathBuf::from("/home/u/.tabsmith"),
            "/usr/local/bin/tabsmith".into(),
            None,
            PathBuf::from("/home/u/.config/fish"),
            None,
        )
    }

    fn zshrc() -> Integration {
        Integration::dotfile(Shell::Zsh, PathBuf::from("/home/u/.zshrc"))
    }

    fn fish_pre() -> Integration {
        Integration::whole_file(
            Shell::Fish,
            PathBuf::from("/home/u/.config/fish/conf.d/00_tabsmith_pre.fish"),
            IntegrationPhase::Pre,
        )
    }

    fn source_line(integration: &Integration, phase: IntegrationPhase) -> String {
        integration.source_line(&paths(), phase)
    }

    /// The `~/.tabsmith/shell` script file of `phase`, with the exact contents install writes.
    fn write_script(bed: &TestBed, integration: &Integration, phase: IntegrationPhase) {
        let script = integration.script(&paths(), phase);
        bed.dotfiles.write(script.path(), script.contents()).unwrap();
    }

    #[test]
    fn should_list_the_bash_login_files_that_exist_after_bashrc() {
        let bed = test_bed();
        bed.dotfiles.write(Path::new("/home/u/.bash_profile"), "").unwrap();

        let listed = bed.installer.integrations(&paths(), Shell::Bash).unwrap();

        assert_eq!(
            listed.iter().map(Integration::path).collect::<Vec<_>>(),
            [Path::new("/home/u/.bashrc"), Path::new("/home/u/.bash_profile")]
        );
    }

    /// Never cached: every listing of pwsh asks again, and no other shell asks (T26).
    #[test]
    fn should_ask_pwsh_for_its_profile_on_each_listing_of_pwsh_and_for_no_other_shell() {
        let bed = test_bed();

        for shell in [Shell::Bash, Shell::Pwsh, Shell::Zsh, Shell::Fish, Shell::Pwsh] {
            bed.installer.integrations(&paths(), shell).unwrap();
        }

        assert_eq!(bed.environment.asked(), ["pwsh_profile", "pwsh_profile"]);
    }

    #[test]
    fn should_list_the_profile_pwsh_gives_as_its_one_dotfile() {
        let bed = test_bed_with(
            RecordingShellEnvironment::default().with_pwsh_profile(Ok(PathBuf::from("/home/u/pwsh/profile.ps1"))),
        );

        let listed = bed.installer.integrations(&paths(), Shell::Pwsh).unwrap();

        assert_eq!(
            listed.iter().map(Integration::path).collect::<Vec<_>>(),
            [Path::new("/home/u/pwsh/profile.ps1")]
        );
    }

    #[test]
    fn should_not_ask_pwsh_for_the_profile_the_paths_know() {
        let bed = test_bed();
        let paths = paths().with_pwsh_profile(PathBuf::from("/home/u/known.ps1"));

        let listed = bed.installer.integrations(&paths, Shell::Pwsh).unwrap();

        assert_eq!(
            (listed[0].path(), bed.environment.asked()),
            (Path::new("/home/u/known.ps1"), vec![])
        );
    }

    #[test]
    fn should_fail_to_list_pwsh_when_pwsh_cannot_give_its_profile() {
        let bed = test_bed_with(
            RecordingShellEnvironment::default().with_pwsh_profile(Err("pwsh printed an empty $PROFILE".into())),
        );

        let error = bed.installer.integrations(&paths(), Shell::Pwsh).unwrap_err();

        assert_eq!(error.to_string(), "pwsh printed an empty $PROFILE");
    }

    #[test]
    fn should_find_a_dotfile_that_does_not_source_the_pre_script_not_installed() {
        let bed = test_bed();
        let zshrc = Integration::dotfile(Shell::Zsh, PathBuf::from("/home/u/.zshrc"));
        bed.dotfiles.write(zshrc.path(), "export A=1\n").unwrap();

        let checked = bed.installer.check(&paths(), &zshrc);

        assert!(
            matches!(&checked, Err(ShellIntegrationError::NotInstalled(m)) if m == "/home/u/.zshrc does not source pre integration"),
            "{checked:?}"
        );
    }

    #[test]
    fn should_find_a_pre_source_line_that_is_not_first_improperly_installed() {
        let bed = test_bed();
        let zshrc = zshrc();
        let (pre, post) = (source_line(&zshrc, IntegrationPhase::Pre), source_line(&zshrc, IntegrationPhase::Post));
        bed.dotfiles.write(zshrc.path(), &format!("export A=1\n{pre}\n{post}\n")).unwrap();

        let checked = bed.installer.check(&paths(), &zshrc);

        assert!(
            matches!(&checked, Err(ShellIntegrationError::Improper(m)) if m == "/home/u/.zshrc does not source pre integration first"),
            "{checked:?}"
        );
    }

    /// The pre script is read before the post line is looked at: a missing pre script wins over a missing post line.
    #[test]
    fn should_check_the_pre_script_after_its_line_and_before_the_post_line() {
        let bed = test_bed();
        let zshrc = zshrc();
        bed.dotfiles.write(zshrc.path(), &format!("{}\n", source_line(&zshrc, IntegrationPhase::Pre))).unwrap();

        let checked = bed.installer.check(&paths(), &zshrc);

        assert!(
            matches!(&checked, Err(ShellIntegrationError::FileDoesNotExist(p)) if p == Path::new("/home/u/.tabsmith/shell/zshrc.pre.zsh")),
            "{checked:?}"
        );
    }

    #[test]
    fn should_find_a_dotfile_that_does_not_source_the_post_script_not_installed() {
        let bed = test_bed();
        let zshrc = zshrc();
        bed.dotfiles.write(zshrc.path(), &format!("{}\nexport A=1\n", source_line(&zshrc, IntegrationPhase::Pre))).unwrap();
        write_script(&bed, &zshrc, IntegrationPhase::Pre);

        let checked = bed.installer.check(&paths(), &zshrc);

        assert!(
            matches!(&checked, Err(ShellIntegrationError::NotInstalled(m)) if m == "/home/u/.zshrc does not source post integration"),
            "{checked:?}"
        );
    }

    #[test]
    fn should_find_a_post_source_line_that_is_not_last_improperly_installed() {
        let bed = test_bed();
        let zshrc = zshrc();
        let (pre, post) = (source_line(&zshrc, IntegrationPhase::Pre), source_line(&zshrc, IntegrationPhase::Post));
        bed.dotfiles.write(zshrc.path(), &format!("{pre}\n{post}\nexport A=1\n")).unwrap();
        write_script(&bed, &zshrc, IntegrationPhase::Pre);

        let checked = bed.installer.check(&paths(), &zshrc);

        assert!(
            matches!(&checked, Err(ShellIntegrationError::Improper(m)) if m == "/home/u/.zshrc does not source post integration last"),
            "{checked:?}"
        );
    }

    #[test]
    fn should_find_a_script_file_holding_anything_else_improperly_installed() {
        let bed = test_bed();
        let zshrc = zshrc();
        let (pre, post) = (source_line(&zshrc, IntegrationPhase::Pre), source_line(&zshrc, IntegrationPhase::Post));
        bed.dotfiles.write(zshrc.path(), &format!("{pre}\n{post}\n")).unwrap();
        let pre_script = zshrc.script(&paths(), IntegrationPhase::Pre);
        bed.dotfiles.write(pre_script.path(), "echo edited\n").unwrap();

        let checked = bed.installer.check(&paths(), &zshrc);

        assert!(
            matches!(&checked, Err(ShellIntegrationError::Improper(m))
                if *m == format!("/home/u/.tabsmith/shell/zshrc.pre.zsh should contain:\n{}", pre_script.contents())),
            "{checked:?}"
        );
    }

    /// The lines compared are trimmed, without blank lines and comments.
    #[test]
    fn should_find_the_source_lines_first_and_last_among_comments_and_blank_lines_installed() {
        let bed = test_bed();
        let zshrc = zshrc();
        let (pre, post) = (source_line(&zshrc, IntegrationPhase::Pre), source_line(&zshrc, IntegrationPhase::Post));
        bed.dotfiles.write(zshrc.path(), &format!("# mine\n\n  {pre}\nexport A=1\n{post}  \n\n# end\n")).unwrap();
        write_script(&bed, &zshrc, IntegrationPhase::Pre);
        write_script(&bed, &zshrc, IntegrationPhase::Post);

        let checked = bed.installer.check(&paths(), &zshrc);

        assert!(checked.is_ok(), "{checked:?}");
    }

    #[test]
    fn should_take_an_opted_out_dotfile_for_installed() {
        let bed = test_bed();
        let zshrc = zshrc();
        bed.dotfiles.write(zshrc.path(), "export A=1\n  #  Tabsmith Ignore\n").unwrap();

        let checked = bed.installer.check(&paths(), &zshrc);

        assert!(checked.is_ok(), "{checked:?}");
    }

    #[test]
    fn should_fail_the_check_of_a_dotfile_it_cannot_read_on_the_dotfile() {
        let bed = test_bed();

        let checked = bed.installer.check(&paths(), &zshrc());

        assert!(
            matches!(&checked, Err(ShellIntegrationError::FileDoesNotExist(p)) if p == Path::new("/home/u/.zshrc")),
            "{checked:?}"
        );
    }

    #[test]
    fn should_find_a_whole_file_holding_exactly_its_script_installed() {
        let bed = test_bed();
        let fish_pre = fish_pre();
        let script = fish_pre.whole_file_script(&paths()).unwrap();
        bed.dotfiles.write(fish_pre.path(), script.contents()).unwrap();

        let checked = bed.installer.check(&paths(), &fish_pre);

        assert!(checked.is_ok(), "{checked:?}");
    }

    #[test]
    fn should_find_a_whole_file_holding_more_than_its_script_improperly_installed() {
        let bed = test_bed();
        let fish_pre = fish_pre();
        let script = fish_pre.whole_file_script(&paths()).unwrap();
        bed.dotfiles.write(fish_pre.path(), &format!("{}# edited\n", script.contents())).unwrap();

        let checked = bed.installer.check(&paths(), &fish_pre);

        assert!(
            matches!(&checked, Err(ShellIntegrationError::Improper(m))
                if *m == format!("/home/u/.config/fish/conf.d/00_tabsmith_pre.fish should contain:\n{}", script.contents())),
            "{checked:?}"
        );
    }

    #[test]
    fn should_write_the_pre_then_the_post_script_file_of_a_dotfile() {
        let bed = test_bed();
        let zshrc = zshrc();

        bed.installer.write_scripts(&paths(), &zshrc).unwrap();

        assert_eq!(
            bed.dotfiles.writes(),
            [
                PathBuf::from("/home/u/.tabsmith/shell/zshrc.pre.zsh"),
                PathBuf::from("/home/u/.tabsmith/shell/zshrc.post.zsh"),
            ]
        );
    }

    #[test]
    fn should_remove_the_file_of_a_whole_file_integration_on_uninstall() {
        let bed = test_bed();
        let fish_pre = fish_pre();
        bed.dotfiles.write(fish_pre.path(), "anything\n").unwrap();

        bed.installer.uninstall(&paths(), &fish_pre).unwrap();

        assert!(!bed.dotfiles.exists(fish_pre.path()));
    }

    #[test]
    fn should_take_the_tabsmith_lines_out_and_trim_the_blank_lines_around_the_rest_on_uninstall() {
        let bed = test_bed();
        let zshrc = zshrc();
        let (pre, post) = (source_line(&zshrc, IntegrationPhase::Pre), source_line(&zshrc, IntegrationPhase::Post));
        bed.dotfiles
            .write(zshrc.path(), &format!("\n\n{PRE_MARKER}\n{pre}\nexport A=1\n\n{POST_MARKER}\n{post}\n\n"))
            .unwrap();

        bed.installer.uninstall(&paths(), &zshrc).unwrap();

        assert_eq!(bed.dotfiles.read(zshrc.path()).unwrap(), "export A=1\n");
    }

    #[test]
    fn should_rewrite_an_empty_dotfile_as_a_single_newline_on_uninstall() {
        let bed = test_bed();
        let zshrc = zshrc();
        bed.dotfiles.write(zshrc.path(), "").unwrap();

        bed.installer.uninstall(&paths(), &zshrc).unwrap();

        assert_eq!(bed.dotfiles.read(zshrc.path()).unwrap(), "\n");
    }

    #[test]
    fn should_leave_a_dotfile_with_nothing_to_take_out_unwritten_on_uninstall() {
        let bed = test_bed();
        let zshrc = zshrc();
        bed.dotfiles.write(zshrc.path(), "export A=1\n").unwrap();

        bed.installer.uninstall(&paths(), &zshrc).unwrap();

        assert_eq!(bed.dotfiles.writes(), [PathBuf::from("/home/u/.zshrc")]);
    }

    #[test]
    fn should_remove_both_script_files_even_without_the_dotfile_on_uninstall() {
        let bed = test_bed();
        let zshrc = zshrc();
        write_script(&bed, &zshrc, IntegrationPhase::Pre);
        write_script(&bed, &zshrc, IntegrationPhase::Post);

        bed.installer.uninstall(&paths(), &zshrc).unwrap();

        assert_eq!(
            [
                "/home/u/.zshrc",
                "/home/u/.tabsmith/shell/zshrc.pre.zsh",
                "/home/u/.tabsmith/shell/zshrc.post.zsh",
            ]
            .map(|p| bed.dotfiles.exists(Path::new(p))),
            [false, false, false]
        );
    }

    #[test]
    fn should_put_the_blocks_into_a_dotfile_that_does_not_exist_on_install() {
        let bed = test_bed();
        let zshrc = zshrc();

        bed.installer.install(&paths(), &zshrc).unwrap();

        assert_eq!(
            bed.dotfiles.read(zshrc.path()).unwrap(),
            format!(
                "{PRE_MARKER}\n{}\n{POST_MARKER}\n{}\n",
                source_line(&zshrc, IntegrationPhase::Pre),
                source_line(&zshrc, IntegrationPhase::Post)
            )
        );
    }

    #[test]
    fn should_change_nothing_of_an_installed_integration_on_install() {
        let bed = test_bed();
        let zshrc = zshrc();
        bed.dotfiles.write(zshrc.path(), "export A=1\n").unwrap();
        bed.installer.install(&paths(), &zshrc).unwrap();
        let before = (bed.dotfiles.writes(), bed.backups.backups());

        bed.installer.install(&paths(), &zshrc).unwrap();

        assert_eq!((bed.dotfiles.writes(), bed.backups.backups()), before);
    }

    #[test]
    fn should_leave_an_opted_out_dotfile_alone_on_install() {
        let bed = test_bed();
        let zshrc = zshrc();
        bed.dotfiles.write(zshrc.path(), "# tabsmith ignore\n").unwrap();

        bed.installer.install(&paths(), &zshrc).unwrap();

        assert_eq!(
            (bed.dotfiles.writes(), bed.backups.backups()),
            (vec![PathBuf::from("/home/u/.zshrc")], vec![])
        );
    }

    #[test]
    fn should_back_up_an_existing_dotfile_into_the_dir_named_after_the_clock_seconds_on_install() {
        let bed = test_bed();
        let zshrc = zshrc();
        bed.dotfiles.write(zshrc.path(), "export A=1\n").unwrap();

        bed.installer.install(&paths(), &zshrc).unwrap();

        assert_eq!(
            bed.backups.backups(),
            [(PathBuf::from("/home/u/.zshrc"), PathBuf::from("/home/u/.tabsmith/backups/1700000123"))]
        );
    }

    #[test]
    fn should_take_no_backup_of_a_dotfile_that_does_not_exist_on_install() {
        let bed = test_bed();

        bed.installer.install(&paths(), &zshrc()).unwrap();

        assert_eq!(bed.backups.backups(), []);
    }

    #[test]
    fn should_write_the_script_files_before_the_dotfile_on_install() {
        let bed = test_bed();

        bed.installer.install(&paths(), &zshrc()).unwrap();

        assert_eq!(
            bed.dotfiles.writes(),
            [
                PathBuf::from("/home/u/.tabsmith/shell/zshrc.pre.zsh"),
                PathBuf::from("/home/u/.tabsmith/shell/zshrc.post.zsh"),
                PathBuf::from("/home/u/.zshrc"),
            ]
        );
    }

    #[test]
    fn should_put_the_user_lines_between_the_blocks_on_install() {
        let bed = test_bed();
        let zshrc = zshrc();
        bed.dotfiles.write(zshrc.path(), "#!/bin/zsh\nexport A=1\n").unwrap();

        bed.installer.install(&paths(), &zshrc).unwrap();

        assert_eq!(
            bed.dotfiles.read(zshrc.path()).unwrap(),
            format!(
                "#!/bin/zsh\n{PRE_MARKER}\n{}\nexport A=1\n{POST_MARKER}\n{}\n",
                source_line(&zshrc, IntegrationPhase::Pre),
                source_line(&zshrc, IntegrationPhase::Post)
            )
        );
    }

    /// Install takes the old blocks out before it adds them again (here the script files were missing).
    #[test]
    fn should_replace_the_blocks_already_there_rather_than_add_a_second_pair_on_install() {
        let bed = test_bed();
        let zshrc = zshrc();
        let (pre, post) = (source_line(&zshrc, IntegrationPhase::Pre), source_line(&zshrc, IntegrationPhase::Post));
        let installed = format!("{PRE_MARKER}\n{pre}\nexport A=1\n{POST_MARKER}\n{post}\n");
        bed.dotfiles.write(zshrc.path(), &installed).unwrap();

        bed.installer.install(&paths(), &zshrc).unwrap();

        assert_eq!(bed.dotfiles.read(zshrc.path()).unwrap(), installed);
    }

    #[test]
    fn should_write_a_whole_file_integration_as_its_script_on_install() {
        let bed = test_bed();
        let fish_pre = fish_pre();

        bed.installer.install(&paths(), &fish_pre).unwrap();

        assert_eq!(
            bed.dotfiles.read(fish_pre.path()).unwrap(),
            fish_pre.whole_file_script(&paths()).unwrap().contents()
        );
    }

    #[test]
    fn should_find_traces_of_a_script_file_left_behind() {
        let bed = test_bed();
        let zshrc = zshrc();
        write_script(&bed, &zshrc, IntegrationPhase::Post);

        let traces = bed.installer.has_traces(&paths(), &zshrc);

        assert!(traces);
    }

    #[test]
    fn should_find_traces_in_a_tabsmith_line_of_the_dotfile_even_with_trailing_whitespace() {
        let bed = test_bed();
        let zshrc = zshrc();
        bed.dotfiles.write(zshrc.path(), &format!("export A=1\n{POST_MARKER}  \r\n")).unwrap();

        let traces = bed.installer.has_traces(&paths(), &zshrc);

        assert!(traces);
    }

    /// Unlike the check, the trace scan does not strip a BOM before the first line.
    #[test]
    fn should_not_take_a_marker_behind_a_byte_order_mark_for_a_trace() {
        let bed = test_bed();
        let zshrc = zshrc();
        bed.dotfiles.write(zshrc.path(), &format!("\u{feff}{PRE_MARKER}\nexport A=1\n")).unwrap();

        let traces = bed.installer.has_traces(&paths(), &zshrc);

        assert!(!traces);
    }

    #[test]
    fn should_find_no_traces_in_a_dotfile_it_cannot_read() {
        let bed = test_bed();

        let traces = bed.installer.has_traces(&paths(), &zshrc());

        assert!(!traces);
    }

    #[test]
    fn should_look_only_at_its_own_file_for_traces_of_a_whole_file_integration() {
        let bed = test_bed();
        let fish_pre = fish_pre();
        write_script(&bed, &fish_pre, IntegrationPhase::Pre);

        let traces = bed.installer.has_traces(&paths(), &fish_pre);

        assert!(!traces);
    }

    #[test]
    fn should_find_traces_of_a_whole_file_integration_in_its_file() {
        let bed = test_bed();
        let fish_pre = fish_pre();
        bed.dotfiles.write(fish_pre.path(), "edited\n").unwrap();

        let traces = bed.installer.has_traces(&paths(), &fish_pre);

        assert!(traces);
    }
}
