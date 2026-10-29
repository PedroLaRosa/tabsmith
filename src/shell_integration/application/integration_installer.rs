use crate::shared::application::ports::clock::Clock;
use crate::shared::domain::value_objects::shell::Shell;
use crate::shell_integration::application::integration_failure::IntegrationFailure;
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

    /// Run `op` on every integration of `shells` that `pick` selects, carrying on past failures (SPEC §2.6). Ok(true)
    /// if `op` ran at least once; any failure, listing a shell or running `op`, discards that and they are returned.
    pub(crate) fn apply(
        &self,
        paths: &IntegrationPaths,
        shells: &[Shell],
        pick: impl Fn(&Integration) -> bool,
        op: impl Fn(&Integration) -> Result<(), ShellIntegrationError>,
    ) -> Result<bool, Vec<IntegrationFailure>> {
        let (mut ran, mut failures) = (false, Vec::new());
        for &shell in shells {
            match self.integrations(paths, shell) {
                Err(e) => failures.push(IntegrationFailure::Listing {
                    shell: shell.name().to_string(),
                    reason: e.to_string(),
                }),
                Ok(list) => {
                    for i in list.iter().filter(|i| pick(i)) {
                        ran = true;
                        if let Err(e) = op(i) {
                            failures.push(IntegrationFailure::Operation {
                                description: i.describe(),
                                reason: e.to_string(),
                            });
                        }
                    }
                }
            }
        }
        if !failures.is_empty() {
            return Err(failures);
        }
        Ok(ran)
    }

    /// Install every integration of `shells` that is not installed. Ok(true) if something was installed.
    pub(crate) fn install_missing(
        &self,
        paths: &IntegrationPaths,
        shells: &[Shell],
    ) -> Result<bool, Vec<IntegrationFailure>> {
        self.apply(
            paths,
            shells,
            |i| self.check(paths, i).is_err(),
            |i| self.install(paths, i),
        )
    }

    /// Uninstall every integration of `shells` with something on disk, not only the installed ones: a moved binary
    /// or lines appended after the post block fail the check exactly when an uninstall is needed. Ok(true) if any.
    pub(crate) fn uninstall_traced(
        &self,
        paths: &IntegrationPaths,
        shells: &[Shell],
    ) -> Result<bool, Vec<IntegrationFailure>> {
        self.apply(
            paths,
            shells,
            |i| self.has_traces(paths, i),
            |i| self.uninstall(paths, i),
        )
    }
}

#[cfg(test)]
mod tests;
