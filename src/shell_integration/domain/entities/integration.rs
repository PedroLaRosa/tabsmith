use crate::shared::domain::services::shell_quoting::quote;
use crate::shared::domain::value_objects::shell::Shell;
use crate::shell_integration::domain::value_objects::integration_paths::IntegrationPaths;
use crate::shell_integration::domain::value_objects::integration_phase::IntegrationPhase;
use crate::shell_integration::domain::value_objects::integration_script::IntegrationScript;
use std::path::{Path, PathBuf};

/// One installable integration of a shell.
#[derive(Clone, Debug)]
pub(crate) struct Integration {
    shell: Shell,
    /// The dotfile that gets the pre/post blocks, or the whole fish `conf.d` file.
    path: PathBuf,
    /// `Some` for a whole-file integration (fish `conf.d`), `None` for a dotfile.
    whole_file_phase: Option<IntegrationPhase>,
}

impl Integration {
    /// A dotfile that gets the pre and post blocks.
    pub(crate) fn dotfile(shell: Shell, path: PathBuf) -> Self {
        Integration {
            shell,
            path,
            whole_file_phase: None,
        }
    }

    /// A file tabsmith owns whole, holding the script of one phase (fish `conf.d`).
    pub(crate) fn whole_file(shell: Shell, path: PathBuf, phase: IntegrationPhase) -> Self {
        Integration {
            shell,
            path,
            whole_file_phase: Some(phase),
        }
    }

    pub(crate) fn shell(&self) -> Shell {
        self.shell
    }

    pub(crate) fn path(&self) -> &Path {
        &self.path
    }

    pub(crate) fn whole_file_phase(&self) -> Option<IntegrationPhase> {
        self.whole_file_phase
    }

    /// `bash pre post into .bashrc` / `fish pre` (SPEC §2.6).
    pub(crate) fn describe(&self) -> String {
        match self.whole_file_phase {
            Some(w) => format!("{} {}", self.shell.name(), w.name()),
            None => format!("{} pre post into {}", self.shell.name(), self.file_name()),
        }
    }

    pub(crate) fn file_name(&self) -> String {
        self.path
            .file_name()
            .map(|f| f.to_string_lossy().into_owned())
            .unwrap_or_default()
    }

    /// `--rcfile` value: `bashrc`, `zprofile`, `00_tabsmith_pre`, `profile` (pwsh).
    fn rc(&self) -> String {
        if self.shell == Shell::Pwsh {
            return "profile".into();
        }
        self.file_name()
            .trim_start_matches('.')
            .split('.')
            .next()
            .unwrap_or_default()
            .to_string()
    }

    /// The `~/.tabsmith/shell` script file of `phase` (dotfile integrations).
    pub(crate) fn script(&self, paths: &IntegrationPaths, phase: IntegrationPhase) -> IntegrationScript {
        IntegrationScript::new(self.script_path(paths, phase), self.contents(paths, phase))
    }

    /// The whole file with the script of its phase (fish `conf.d`); `None` for a dotfile.
    pub(crate) fn whole_file_script(&self, paths: &IntegrationPaths) -> Option<IntegrationScript> {
        self.whole_file_phase
            .map(|phase| IntegrationScript::new(self.path.clone(), self.contents(paths, phase)))
    }

    /// `~/.tabsmith/shell/<rc>.<phase>.<ext>` (dotfile integrations).
    fn script_path(&self, paths: &IntegrationPaths, phase: IntegrationPhase) -> PathBuf {
        let ext = if self.shell == Shell::Pwsh {
            "ps1"
        } else {
            self.shell.name()
        };
        paths.data_root()
            .join("shell")
            .join(format!("{}.{}.{ext}", self.rc(), phase.name()))
    }

    /// Exact bytes of a script file / fish `conf.d` file (SPEC §5.5).
    fn contents(&self, paths: &IntegrationPaths, phase: IntegrationPhase) -> String {
        let (b, w, rc) = (quote(self.shell, paths.binary()), phase.name(), self.rc());
        match self.shell {
            Shell::Bash => {
                format!("[ -n \"$BASH_VERSION\" ] && [ -x {b} ] && eval \"$({b} init bash {w} --rcfile {rc})\"\n")
            }
            Shell::Zsh => format!("[ -x {b} ] && eval \"$({b} init zsh {w} --rcfile {rc})\"\n"),
            // `eval (… | string split0)`, not `| source`: `source` hands the pipe to the script as stdin, so the pre
            // script's `test -t 0` fails and the wrapper never starts.
            Shell::Fish => format!("test -x {b}; and eval ({b} init fish {w} --rcfile {rc} | string split0)\n"),
            // pwsh decodes native output with [Console]::OutputEncoding (the OEM code page on Windows): read the
            // UTF-8 init output as UTF-8, then put the user's encoding back before running it. Without a console
            // (Windows) both setters throw: ignore that, and run nothing when init printed nothing.
            Shell::Pwsh => format!(
                "if (Test-Path -LiteralPath {b}) {{ $__tsEnc = [Console]::OutputEncoding; $__tsInit = $null; try {{ try {{ [Console]::OutputEncoding = [Text.UTF8Encoding]::new() }} catch {{}}; $__tsInit = (& {b} init pwsh {w} --rcfile profile) -join \"`n\" }} finally {{ try {{ [Console]::OutputEncoding = $__tsEnc }} catch {{}} }}; if ($__tsInit) {{ Invoke-Expression $__tsInit }} }}\n"
            ),
        }
    }

    /// The dotfile line that sources a script file; a leading `$HOME` is written as `${HOME}`.
    pub(crate) fn source_line(&self, paths: &IntegrationPaths, phase: IntegrationPhase) -> String {
        let p = self.script_path(paths, phase);
        let p = match p.strip_prefix(paths.home()) {
            Ok(rel) => {
                let parts: Vec<_> = rel.iter().map(|c| c.to_string_lossy()).collect();
                format!("${{HOME}}/{}", parts.join("/"))
            }
            Err(_) => p.display().to_string(),
        };
        match self.shell {
            Shell::Pwsh => format!("if (Test-Path \"{p}\") {{ . \"{p}\" }}"),
            _ => format!("[ -f \"{p}\" ] && . \"{p}\""),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shared::infrastructure::testing::temp_dir::TempDir;

    fn ctx(dir: &Path) -> IntegrationPaths {
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

    /// pwsh is not installed here, so no script test runs its files locally: their bytes are pinned instead.
    #[test]
    fn should_write_the_pwsh_script_files_byte_for_byte() {
        let temp_dir = TempDir::new("pwshfiles");
        let c = ctx(temp_dir.path());
        let pwsh = Integration::dotfile(Shell::Pwsh, c.pwsh_profile().unwrap().to_path_buf());

        let files: Vec<(PathBuf, String)> = [IntegrationPhase::Pre, IntegrationPhase::Post]
            .into_iter()
            .map(|when| {
                let script = pwsh.script(&c, when);
                (script.path().to_path_buf(), script.contents().to_string())
            })
            .collect();

        assert_eq!(
            files,
            [
                (
                    c.data_root().join("shell").join("profile.pre.ps1"),
                    "if (Test-Path -LiteralPath '/opt/tab smith/it''s/tabsmith') { $__tsEnc = [Console]::OutputEncoding; $__tsInit = $null; try { try { [Console]::OutputEncoding = [Text.UTF8Encoding]::new() } catch {}; $__tsInit = (& '/opt/tab smith/it''s/tabsmith' init pwsh pre --rcfile profile) -join \"`n\" } finally { try { [Console]::OutputEncoding = $__tsEnc } catch {} }; if ($__tsInit) { Invoke-Expression $__tsInit } }\n".to_string()
                ),
                (
                    c.data_root().join("shell").join("profile.post.ps1"),
                    "if (Test-Path -LiteralPath '/opt/tab smith/it''s/tabsmith') { $__tsEnc = [Console]::OutputEncoding; $__tsInit = $null; try { try { [Console]::OutputEncoding = [Text.UTF8Encoding]::new() } catch {}; $__tsInit = (& '/opt/tab smith/it''s/tabsmith' init pwsh post --rcfile profile) -join \"`n\" } finally { try { [Console]::OutputEncoding = $__tsEnc } catch {} }; if ($__tsInit) { Invoke-Expression $__tsInit } }\n".to_string()
                ),
            ]
        );
    }

    /// fish `conf.d`: the integration's own file holds the script of its phase; a dotfile has no such file.
    #[test]
    fn should_give_only_a_whole_file_integration_its_own_file_with_the_script_of_its_phase() {
        let temp_dir = TempDir::new("wholefile");
        let c = ctx(temp_dir.path());
        let fish_pre_file = c.fish_config_dir().join("conf.d/00_tabsmith_pre.fish");
        let fish_pre = Integration::whole_file(Shell::Fish, fish_pre_file.clone(), IntegrationPhase::Pre);
        let zshrc = Integration::dotfile(Shell::Zsh, c.home().join(".zshrc"));

        let scripts = [fish_pre, zshrc].map(|i| {
            i.whole_file_script(&c)
                .map(|s| (s.path().to_path_buf(), s.contents().to_string()))
        });

        assert_eq!(
            scripts,
            [
                Some((
                    fish_pre_file,
                    "test -x '/opt/tab smith/it\\'s/tabsmith'; and eval ('/opt/tab smith/it\\'s/tabsmith' init fish pre --rcfile 00_tabsmith_pre | string split0)\n".to_string()
                )),
                None,
            ]
        );
    }
}
