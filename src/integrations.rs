//! Shell integration: `init` scripts, launch decision, dotfile install/uninstall/status, setup/uninstall flows,
//! and the internal `_ should-launch|get-shell|pre-cmd` commands (SPEC §2.6, §2.7, §2.9, §2.11, §5).
//! OWNER: integrations agent. Shell scripts live in `shell/` and are embedded with `include_str!`.
use crate::shared::domain::services::shell_quoting::quote;
use crate::shared::domain::value_objects::shell::Shell;
use crate::shared::infrastructure::atomic_file::AtomicFile;
use crate::shared::infrastructure::atomic_file_error::AtomicFileError;
use crate::shared::infrastructure::data_directory_from_env::data_directory_from_env;
use crate::shared::infrastructure::json_document_store::JsonDocumentStore;
use crate::shared::infrastructure::logger;
use crate::shared::infrastructure::os::executable_lookup::which;
#[cfg(windows)]
use crate::shared::infrastructure::os::process_info::is_console;
use crate::shared::infrastructure::os::process_info::{exe_path, is_root, parent_exe};
use crate::shared::infrastructure::os::process_runner::output_with_timeout;
use crate::shared::infrastructure::output_format::OutputFormat;
use crate::shared::infrastructure::terminal_style::{epaint, paint};
use crate::types::{now_ms, now_secs, SessionCtx};
use anyhow::{bail, Context, Result};
use clap::{Args, Subcommand, ValueEnum};
use std::collections::HashMap;
use std::io::{IsTerminal, Write};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

#[derive(Clone, Copy, Debug, PartialEq, Eq, ValueEnum)]
pub enum ShellArg {
    Bash,
    Zsh,
    Fish,
    Pwsh,
}

impl From<ShellArg> for Shell {
    fn from(s: ShellArg) -> Shell {
        match s {
            ShellArg::Bash => Shell::Bash,
            ShellArg::Zsh => Shell::Zsh,
            ShellArg::Fish => Shell::Fish,
            ShellArg::Pwsh => Shell::Pwsh,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, ValueEnum)]
pub enum When {
    Pre,
    Post,
}

impl When {
    pub fn name(self) -> &'static str {
        match self {
            When::Pre => "pre",
            When::Post => "post",
        }
    }
}

#[derive(Args)]
pub struct InitArgs {
    pub shell: ShellArg,
    pub when: When,
    #[arg(long)]
    pub rcfile: Option<String>,
}

#[derive(Subcommand, Clone, Debug)]
pub enum IntegrationTarget {
    /// Shell dotfile integrations
    Dotfiles { shell: Option<ShellArg> },
    /// All integrations
    All,
}

#[derive(Subcommand)]
pub enum IntegrationsCmd {
    /// Install an integration
    Install {
        #[command(subcommand)]
        target: IntegrationTarget,
        /// Suppress status messages
        #[arg(long, short)]
        silent: bool,
    },
    /// Uninstall an integration
    Uninstall {
        #[command(subcommand)]
        target: IntegrationTarget,
        #[arg(long, short)]
        silent: bool,
    },
    /// Reinstall an integration
    Reinstall {
        #[command(subcommand)]
        target: IntegrationTarget,
        #[arg(long, short)]
        silent: bool,
    },
    /// Check the status of an integration
    Status {
        #[command(subcommand)]
        target: IntegrationTarget,
        #[arg(long, short, value_enum, default_value_t)]
        format: OutputFormat,
    },
}

#[derive(Args)]
pub struct SetupArgs {
    /// Do not prompt for confirmation
    #[arg(long)]
    pub no_confirm: bool,
    /// Force installation (e.g. as root)
    #[arg(long)]
    pub force: bool,
}

#[derive(Args)]
pub struct UninstallArgs {
    /// Do not prompt for confirmation
    #[arg(long, short = 'y')]
    pub no_confirm: bool,
    /// Also remove ~/.tabsmith
    #[arg(long)]
    pub purge: bool,
}

// ---------------------------------------------------------------------------------------------------------------
// Small shared helpers (also used by doctor)

/// Mark that existing terminals need a restart (state `doctor.prompt-restart-terminal`, printed by doctor).
pub fn mark_restart() {
    let state_file = data_directory_from_env().state_file();
    if let Some(mut s) = state_file.ok().and_then(|path| JsonDocumentStore::load(&path).ok()) {
        s.map
            .insert("doctor.prompt-restart-terminal".into(), true.into());
        let _ = s.save();
    }
}

// ---------------------------------------------------------------------------------------------------------------
// init (SPEC §2.7, §5.3)

const PRE_SH: &str = include_str!("../shell/pre.sh");
const POST_ZSH: &str = include_str!("../shell/post.zsh");
const POST_BASH: &str = include_str!("../shell/post.bash");
const PRE_FISH: &str = include_str!("../shell/pre.fish");
const POST_FISH: &str = include_str!("../shell/post.fish");
const PRE_PS1: &str = include_str!("../shell/pre.ps1");
const POST_PS1: &str = include_str!("../shell/post.ps1");

pub fn init_cmd(args: InitArgs) -> anyhow::Result<i32> {
    if let Some(rc) = &args.rcfile {
        logger::debug(&format!(
            "init {:?} {} --rcfile {rc}",
            args.shell,
            args.when.name()
        ));
    }
    let out = match init_script(args.shell.into(), args.when) {
        Ok(s) => s,
        Err(e) => format!("# Could not load source: {e:#}"),
    };
    let _ = writeln!(std::io::stdout(), "{out}");
    Ok(0)
}

/// The integration code `init <shell> <when>` prints (SPEC §5.3), without the trailing newline.
pub fn init_script(shell: Shell, when: When) -> anyhow::Result<String> {
    init_output(
        shell,
        when,
        std::env::var_os("TABSMITH_INIT_SNAPSHOT_TEST").is_some(),
    )
}

/// `snapshot` = deterministic output (SPEC §2.7): shell `/bin/zsh`, launch 0, binary `tabsmith`.
fn init_output(shell: Shell, when: When, snapshot: bool) -> Result<String> {
    let bin = if snapshot {
        "tabsmith".to_string()
    } else {
        exe_path()?
    };
    let launch = |snap| if snap || should_launch().0 { 0 } else { 1 };
    let mut lines = Vec::new();
    if when == When::Pre {
        match shell {
            Shell::Bash | Shell::Zsh => {
                let parent = if snapshot {
                    Some("/bin/zsh".to_string())
                } else {
                    parent_exe()
                };
                if let Some(p) = parent {
                    lines.push(format!("TABSMITH_SHELL={}", quote(shell, &p)));
                }
                lines.push(format!("SHOULD_TABSMITH_LAUNCH={}", launch(snapshot)));
            }
            Shell::Fish => lines.push(format!(
                "set -g SHOULD_TABSMITH_LAUNCH {}",
                launch(snapshot)
            )),
            Shell::Pwsh => lines.push(format!("$TABSMITH_SHOULD_LAUNCH = {}", launch(snapshot))),
        }
    }
    let body = match (shell, when) {
        (Shell::Bash | Shell::Zsh, When::Pre) => PRE_SH,
        (Shell::Bash, When::Post) => POST_BASH,
        (Shell::Zsh, When::Post) => POST_ZSH,
        (Shell::Fish, When::Pre) => PRE_FISH,
        (Shell::Fish, When::Post) => POST_FISH,
        (Shell::Pwsh, When::Pre) => PRE_PS1,
        (Shell::Pwsh, When::Post) => POST_PS1,
    };
    lines.push(
        body.trim_end_matches('\n')
            .replace("{{BIN}}", &quote(shell, &bin)),
    );
    Ok(lines.join("\n"))
}

// ---------------------------------------------------------------------------------------------------------------
// Launch decision (SPEC §5.2)

/// Launch decision (SPEC §5.2): (launch?, reason line).
pub fn should_launch() -> (bool, String) {
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
    launch_decision(&env, stdin_ok)
}

/// SPEC §5.2, first match wins. `stdin_ok`: stdin is a tty (Unix) / a console (Windows).
pub fn launch_decision(env: &HashMap<String, String>, stdin_ok: bool) -> (bool, String) {
    let has = |k: &str| env.contains_key(k);
    let is = |k: &str, v: &str| env.get(k).is_some_and(|x| x == v);
    let (launch, why) = if has("TABSMITH_FORCE_LAUNCH") {
        (true, "TABSMITH_FORCE_LAUNCH")
    } else if has("TABSMITH_TERM_DISABLED") {
        (false, "TABSMITH_TERM_DISABLED")
    } else if has("PROCESS_LAUNCHED_BY_TABSMITH") {
        (false, "PROCESS_LAUNCHED_BY_TABSMITH")
    } else if has("TABSMITH_TERM") {
        if has("TMUX") && !has("TABSMITH_TERM_TMUX") {
            (true, "new tmux pane")
        } else {
            (false, "already inside tabsmith")
        }
    } else if has("INSIDE_EMACS") {
        (false, "INSIDE_EMACS")
    } else if is("TERM_PROGRAM", "WarpTerminal") {
        (false, "TERM_PROGRAM = WarpTerminal")
    } else if is("__CFBundleIdentifier", "com.vandyke.SecureCRT") {
        (false, "__CFBundleIdentifier = com.vandyke.SecureCRT")
    } else if has("__PWSH_LOGIN_CHECKED") {
        (false, "__PWSH_LOGIN_CHECKED")
    } else if has("CI") {
        (false, "In CI")
    } else if has("VSCODE_RESOLVING_ENVIRONMENT") {
        (false, "VSCODE_RESOLVING_ENVIRONMENT")
    } else if cfg!(unix) && (!has("TERM") || is("TERM", "dumb")) {
        (false, "TERM is unset or dumb")
    } else if !stdin_ok {
        (
            false,
            if cfg!(windows) {
                "stdin is not a console"
            } else {
                "stdin is not a tty"
            },
        )
    } else {
        (true, "launching")
    };
    (
        launch,
        format!("{} {why}", if launch { "✅" } else { "❌" }),
    )
}

pub fn should_launch_cmd() -> anyhow::Result<i32> {
    let (launch, why) = should_launch();
    println!("{why}");
    Ok(if launch { 0 } else { 1 })
}

pub fn get_shell_cmd() -> anyhow::Result<i32> {
    match parent_exe() {
        Some(p) => {
            print!("{p}");
            let _ = std::io::stdout().flush();
            Ok(0)
        }
        None => Ok(1),
    }
}

pub fn pre_cmd(alias: Option<String>) -> anyhow::Result<i32> {
    // The id becomes a file name: accept only what the wrapper generates (hex), never a path.
    let Some(sid) = std::env::var("TABSMITH_SESSION_ID").ok().filter(|s| {
        !s.is_empty()
            && s.bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
    }) else {
        return Ok(1);
    };
    let env = std::env::vars_os()
        .filter_map(|(k, v)| Some((k.into_string().ok()?, v.into_string().ok()?)))
        .collect();
    let ctx = SessionCtx {
        env,
        alias,
        ts: now_ms(),
    };
    let path = SessionCtx::path(&sid)?;
    AtomicFile::new(&path).write_private(serde_json::to_string(&ctx)?.as_bytes())?;
    prune_run_files(path.parent().context("no parent directory")?);
    Ok(0)
}

/// Delete run files untouched for a day. A shell that exits right after a prompt leaves one behind: its background
/// `_ pre-cmd` writes the file again after the wrapper removed it. A live session rewrites its file at every prompt.
fn prune_run_files(dir: &Path) {
    for e in std::fs::read_dir(dir).into_iter().flatten().flatten() {
        let stale = e.metadata().is_ok_and(|m| {
            m.is_file()
                && m.modified()
                    .is_ok_and(|t| t.elapsed().unwrap_or_default() > Duration::from_secs(24 * 3600))
        });
        if stale {
            let _ = std::fs::remove_file(e.path());
        }
    }
}

// ---------------------------------------------------------------------------------------------------------------
// Dotfile integrations (SPEC §5.5)

pub const PRE_MARKER: &str = "# tabsmith pre block. Keep at the top of this file.";
pub const POST_MARKER: &str = "# tabsmith post block. Keep at the bottom of this file.";

/// Everything an integration operation reads, explicit so tests use a temp HOME.
#[derive(Clone, Debug)]
pub struct Ctx {
    /// The user's home directory.
    pub home: PathBuf,
    /// `~/.tabsmith`.
    pub data: PathBuf,
    /// Absolute path of the tabsmith binary written into the script files.
    pub bin: String,
    /// `$ZDOTDIR`, if set.
    pub zdotdir: Option<PathBuf>,
    /// fish config dir: `$XDG_CONFIG_HOME/fish` or `~/.config/fish`.
    pub fish_dir: PathBuf,
    /// pwsh `$PROFILE`; `None` = ask `pwsh`.
    pub pwsh_profile: Option<PathBuf>,
}

impl Ctx {
    pub fn from_env() -> Result<Ctx> {
        let home = crate::settings::user_home()?;
        let var = |k: &str| {
            std::env::var_os(k)
                .filter(|v| !v.is_empty())
                .map(PathBuf::from)
        };
        Ok(Ctx {
            data: crate::settings::home()?,
            bin: exe_path()?,
            zdotdir: var("ZDOTDIR"),
            fish_dir: var("XDG_CONFIG_HOME")
                .unwrap_or_else(|| home.join(".config"))
                .join("fish"),
            pwsh_profile: None,
            home,
        })
    }
}

/// `is_installed()` / `install()` failure (SPEC §5.5 display strings).
#[derive(Debug)]
pub enum IntegrationError {
    NotInstalled(String),
    Improper(String),
    FileDoesNotExist(PathBuf),
    PermissionDenied(PathBuf),
    Io(PathBuf, std::io::Error),
}

impl std::fmt::Display for IntegrationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            IntegrationError::NotInstalled(m) => write!(f, "Integration not installed: {m}"),
            IntegrationError::Improper(m) => write!(f, "Improper integration installation: {m}"),
            IntegrationError::FileDoesNotExist(p) => {
                write!(f, "File does not exist: {}", p.display())
            }
            IntegrationError::PermissionDenied(p) => {
                write!(f, "Permission denied: {}", p.display())
            }
            IntegrationError::Io(p, e) => write!(f, "{}: {e}", p.display()),
        }
    }
}

impl std::error::Error for IntegrationError {}

fn io_err(path: &Path, e: std::io::Error) -> IntegrationError {
    match e.kind() {
        std::io::ErrorKind::NotFound => IntegrationError::FileDoesNotExist(path.into()),
        std::io::ErrorKind::PermissionDenied => IntegrationError::PermissionDenied(path.into()),
        _ => IntegrationError::Io(path.into(), e),
    }
}

fn read(path: &Path) -> Result<String, IntegrationError> {
    std::fs::read_to_string(path).map_err(|e| io_err(path, e))
}

/// See [`AtomicFile::write_keeping_permissions`]; its failures keep their IntegrationError (the parent directory or
/// the dotfile path, as before).
fn write(path: &Path, contents: &str) -> Result<(), IntegrationError> {
    AtomicFile::new(path)
        .write_keeping_permissions(contents.as_bytes())
        .map_err(|e| match e {
            AtomicFileError::Io { path: failed, source } | AtomicFileError::Rename { path: failed, source } => {
                io_err(&failed, source)
            }
            // Not produced by this write (it never needs a parent); mapped to stay exhaustive.
            no_parent @ AtomicFileError::NoParentDirectory => io_err(path, std::io::Error::other(no_parent)),
        })
}

/// Delete a file; missing is fine.
fn remove(path: &Path) -> Result<(), IntegrationError> {
    match std::fs::remove_file(path) {
        Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(io_err(path, e)),
        _ => Ok(()),
    }
}

/// The file must hold exactly `expected`.
fn check_file(path: &Path, expected: &str) -> Result<(), IntegrationError> {
    if read(path)? == expected {
        Ok(())
    } else {
        Err(IntegrationError::Improper(format!(
            "{} should contain:\n{expected}",
            path.display()
        )))
    }
}

/// `# tabsmith ignore` (case-insensitive): the user opted this file out.
fn is_ignore_line(line: &str) -> bool {
    line.trim_start().strip_prefix('#').is_some_and(|r| {
        r.trim_start()
            .to_ascii_lowercase()
            .starts_with("tabsmith ignore")
    })
}

/// (UTF-8 BOM or "", the text after it): editors save pwsh profiles with a BOM, which must stay the first bytes.
fn split_bom(s: &str) -> (&str, &str) {
    s.strip_prefix('\u{feff}')
        .map_or(("", s), |b| ("\u{feff}", b))
}

/// First line kept on top iff it starts with `#!`.
fn split_shebang(s: &str) -> (String, &str) {
    if !s.starts_with("#!") {
        return (String::new(), s);
    }
    match s.find('\n') {
        Some(i) => (s[..=i].to_string(), &s[i + 1..]),
        None => (format!("{s}\n"), ""),
    }
}

/// One installable integration of a shell.
#[derive(Clone, Debug)]
pub struct Integration {
    pub shell: Shell,
    /// The dotfile that gets the pre/post blocks, or the whole fish `conf.d` file.
    pub path: PathBuf,
    /// `Some` for a whole-file integration (fish `conf.d`), `None` for a dotfile.
    pub file_when: Option<When>,
}

/// Integrations of `shell` (SPEC §5.5 table).
pub fn integrations(ctx: &Ctx, shell: Shell) -> Result<Vec<Integration>> {
    let dot = |path: PathBuf| Integration {
        shell,
        path,
        file_when: None,
    };
    Ok(match shell {
        Shell::Bash => {
            let extra: Vec<PathBuf> = [".profile", ".bash_login", ".bash_profile"]
                .iter()
                .map(|f| ctx.home.join(f))
                .filter(|p| p.exists())
                .collect();
            let extra = if extra.is_empty() {
                vec![ctx.home.join(".profile")]
            } else {
                extra
            };
            std::iter::once(ctx.home.join(".bashrc"))
                .chain(extra)
                .map(dot)
                .collect()
        }
        Shell::Zsh => {
            let dir = ctx.zdotdir.as_ref().unwrap_or(&ctx.home);
            vec![dot(dir.join(".zshrc")), dot(dir.join(".zprofile"))]
        }
        Shell::Fish => [
            (When::Pre, "00_tabsmith_pre.fish"),
            (When::Post, "99_tabsmith_post.fish"),
        ]
        .into_iter()
        .map(|(w, f)| Integration {
            shell,
            path: ctx.fish_dir.join("conf.d").join(f),
            file_when: Some(w),
        })
        .collect(),
        Shell::Pwsh => vec![dot(match &ctx.pwsh_profile {
            Some(p) => p.clone(),
            None => pwsh_profile()?,
        })],
    })
}

/// Prints `$PROFILE` as raw UTF-8 bytes: pwsh's Console.Out would encode it with the (shared, OEM on Windows) console
/// code page, and changing that code page would outlive the child.
const PWSH_PROFILE_CMD: &str =
    "$b = [Text.Encoding]::UTF8.GetBytes($PROFILE); $s = [Console]::OpenStandardOutput(); $s.Write($b, 0, $b.Length); $s.Flush()";

/// `$PROFILE` (CurrentUserCurrentHost) from `pwsh -NoLogo -NoProfile -Command …` (5 s timeout).
fn pwsh_profile() -> Result<PathBuf> {
    let out = output_with_timeout(
        Command::new("pwsh").args(["-NoLogo", "-NoProfile", "-Command", PWSH_PROFILE_CMD]),
        Duration::from_secs(5),
    )
    .context("could not ask pwsh for $PROFILE")?;
    match out.trim() {
        "" => bail!("pwsh printed an empty $PROFILE"),
        p => Ok(PathBuf::from(p)),
    }
}

impl Integration {
    /// `bash pre post into .bashrc` / `fish pre` (SPEC §2.6).
    pub fn describe(&self) -> String {
        match self.file_when {
            Some(w) => format!("{} {}", self.shell.name(), w.name()),
            None => format!("{} pre post into {}", self.shell.name(), self.file_name()),
        }
    }

    pub fn file_name(&self) -> String {
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

    /// `~/.tabsmith/shell/<rc>.<when>.<ext>` (dotfile integrations).
    pub fn script_path(&self, ctx: &Ctx, when: When) -> PathBuf {
        let ext = if self.shell == Shell::Pwsh {
            "ps1"
        } else {
            self.shell.name()
        };
        ctx.data
            .join("shell")
            .join(format!("{}.{}.{ext}", self.rc(), when.name()))
    }

    /// Exact bytes of a script file / fish `conf.d` file (SPEC §5.5).
    pub fn script_contents(&self, ctx: &Ctx, when: When) -> String {
        let (b, w, rc) = (quote(self.shell, &ctx.bin), when.name(), self.rc());
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
    fn source_line(&self, ctx: &Ctx, when: When) -> String {
        let p = self.script_path(ctx, when);
        let p = match p.strip_prefix(&ctx.home) {
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

    /// Write both `~/.tabsmith/shell/` script files of a dotfile integration.
    pub fn write_scripts(&self, ctx: &Ctx) -> Result<(), IntegrationError> {
        for when in [When::Pre, When::Post] {
            write(
                &self.script_path(ctx, when),
                &self.script_contents(ctx, when),
            )?;
        }
        Ok(())
    }

    /// SPEC §5.5 `is_installed()`: the first failing reason.
    pub fn is_installed(&self, ctx: &Ctx) -> Result<(), IntegrationError> {
        if let Some(when) = self.file_when {
            return check_file(&self.path, &self.script_contents(ctx, when));
        }
        let text = read(&self.path)?;
        let text = split_bom(&text).1;
        if text.lines().any(is_ignore_line) {
            return Ok(());
        }
        let lines: Vec<&str> = text
            .lines()
            .map(str::trim)
            .filter(|l| !l.is_empty() && !l.starts_with('#'))
            .collect();
        for (when, at, actual) in [
            (When::Pre, "first", lines.first()),
            (When::Post, "last", lines.last()),
        ] {
            let src = self.source_line(ctx, when);
            if actual.copied() != Some(src.as_str()) {
                let msg = format!(
                    "{} does not source {} integration",
                    self.path.display(),
                    when.name()
                );
                return Err(if lines.contains(&src.as_str()) {
                    IntegrationError::Improper(format!("{msg} {at}"))
                } else {
                    IntegrationError::NotInstalled(msg)
                });
            }
            check_file(
                &self.script_path(ctx, when),
                &self.script_contents(ctx, when),
            )?;
        }
        Ok(())
    }

    /// SPEC §5.5 install (idempotent); an existing dotfile is first copied to `<data>/backups/<unix-secs>/`.
    pub fn install(&self, ctx: &Ctx) -> Result<(), IntegrationError> {
        if self.is_installed(ctx).is_ok() {
            return Ok(());
        }
        if let Some(when) = self.file_when {
            return write(&self.path, &self.script_contents(ctx, when));
        }
        let contents = if self.path.exists() {
            let dir = ctx.data.join("backups").join(now_secs().to_string());
            std::fs::create_dir_all(&dir).map_err(|e| io_err(&dir, e))?;
            std::fs::copy(&self.path, dir.join(self.file_name()))
                .map_err(|e| io_err(&self.path, e))?;
            self.uninstall(ctx)?;
            read(&self.path)?
        } else {
            String::new()
        };
        self.write_scripts(ctx)?;
        let (bom, body) = split_bom(&contents);
        let (shebang, rest) = split_shebang(body);
        let new = format!(
            "{bom}{shebang}{PRE_MARKER}\n{}\n{rest}",
            self.source_line(ctx, When::Pre)
        );
        let new = format!(
            "{}\n{POST_MARKER}\n{}\n",
            new.trim_end_matches('\n'),
            self.source_line(ctx, When::Post)
        );
        if new != contents {
            write(&self.path, &new)?;
        }
        Ok(())
    }

    /// Is `line` one of the four dotfile lines install writes?
    fn is_own_line(&self, ctx: &Ctx, line: &str) -> bool {
        let line = line.trim_end();
        [PRE_MARKER, POST_MARKER].contains(&line)
            || [When::Pre, When::Post]
                .into_iter()
                .any(|w| self.source_line(ctx, w) == line)
    }

    /// Anything uninstall would remove is on disk: also true where `is_installed` fails (a moved binary, an edited
    /// script, lines appended after the post block).
    fn has_traces(&self, ctx: &Ctx) -> bool {
        if self.file_when.is_some() {
            return self.path.exists();
        }
        [When::Pre, When::Post]
            .into_iter()
            .any(|w| self.script_path(ctx, w).exists())
            || read(&self.path).is_ok_and(|t| t.lines().any(|l| self.is_own_line(ctx, l)))
    }

    /// SPEC §5.5 uninstall: drop our exact lines, then delete the script files (missing is fine).
    pub fn uninstall(&self, ctx: &Ctx) -> Result<(), IntegrationError> {
        if self.file_when.is_some() {
            return remove(&self.path);
        }
        if self.path.exists() {
            let text = read(&self.path)?;
            let (bom, body) = split_bom(&text);
            let kept: String = body
                .split_inclusive('\n')
                .filter(|l| !self.is_own_line(ctx, l))
                .collect();
            let new = format!("{bom}{}\n", kept.trim());
            if new != text {
                write(&self.path, &new)?;
            }
        }
        remove(&self.script_path(ctx, When::Pre))?;
        remove(&self.script_path(ctx, When::Post))
    }
}

/// `[bash, zsh, fish]`, plus pwsh when it is on PATH (SPEC §2.6).
fn default_shells() -> Vec<Shell> {
    let mut v = vec![Shell::Bash, Shell::Zsh, Shell::Fish];
    if which("pwsh").is_some() {
        v.push(Shell::Pwsh);
    }
    v
}

/// Run `op` on every integration of `shells` that `pick` selects. Ok(true) if `op` ran at least once; errors
/// are collected as `"<describe bold>: <message>"` and returned as one (SPEC §2.6).
fn apply(
    ctx: &Ctx,
    shells: &[Shell],
    pick: impl Fn(&Integration) -> bool,
    op: impl Fn(&Integration, &Ctx) -> Result<(), IntegrationError>,
) -> Result<bool> {
    let (mut ran, mut errs) = (false, Vec::new());
    for &shell in shells {
        match integrations(ctx, shell) {
            Err(e) => errs.push(format!("{}: {e:#}", shell.name())),
            Ok(list) => {
                for i in list.iter().filter(|i| pick(i)) {
                    ran = true;
                    if let Err(e) = op(i, ctx) {
                        errs.push(format!("{}: {e}", paint("1", &i.describe())));
                    }
                }
            }
        }
    }
    if !errs.is_empty() {
        bail!("\n\n{}", errs.join("\n\n"));
    }
    Ok(ran)
}

/// Install every integration of `shells` that is not installed. Ok(true) if something was installed.
pub fn install_dotfiles(ctx: &Ctx, shells: &[Shell]) -> Result<bool> {
    apply(
        ctx,
        shells,
        |i| i.is_installed(ctx).is_err(),
        Integration::install,
    )
}

/// Uninstall every integration of `shells` with something on disk. Not only the installed ones: `is_installed`
/// fails exactly when uninstall is needed (a moved binary, lines appended after the post block). Ok(true) if any.
fn uninstall_dotfiles(ctx: &Ctx, shells: &[Shell]) -> Result<bool> {
    apply(ctx, shells, |i| i.has_traces(ctx), Integration::uninstall)
}

fn target_shells(t: &IntegrationTarget) -> Vec<Shell> {
    match t {
        IntegrationTarget::Dotfiles { shell: Some(s) } => vec![(*s).into()],
        _ => default_shells(),
    }
}

pub fn integrations_cmd(cmd: IntegrationsCmd) -> anyhow::Result<i32> {
    let ctx = Ctx::from_env()?;
    let install = |target: &IntegrationTarget, silent: bool| -> Result<i32> {
        let installed = install_dotfiles(&ctx, &target_shells(target))?;
        if installed {
            mark_restart();
        }
        if !silent {
            println!(
                "{}",
                if installed {
                    "Installed!"
                } else {
                    "Already installed"
                }
            );
        }
        Ok(0)
    };
    let uninstall = |target: &IntegrationTarget, silent: bool| -> Result<i32> {
        let done = uninstall_dotfiles(&ctx, &target_shells(target))?;
        if !silent {
            println!(
                "{}",
                if done {
                    "Uninstalled!"
                } else {
                    "Not installed"
                }
            );
        }
        Ok(0)
    };
    match cmd {
        IntegrationsCmd::Install { target, silent } => install(&target, silent),
        IntegrationsCmd::Uninstall { target, silent } => uninstall(&target, silent),
        IntegrationsCmd::Reinstall { target, silent } => {
            uninstall(&target, true)?;
            install(&target, silent)
        }
        IntegrationsCmd::Status { target, format } => status(&ctx, &target_shells(&target), format),
    }
}

/// `integrations status` (SPEC §2.6); honours the SHELL argument.
fn status(ctx: &Ctx, shells: &[Shell], format: OutputFormat) -> Result<i32> {
    let (mut rows, mut errs) = (Vec::new(), Vec::new());
    for &shell in shells {
        match integrations(ctx, shell) {
            Ok(list) => rows.extend(list.into_iter().map(|i| (i.is_installed(ctx).is_ok(), i))),
            Err(e) => errs.push((shell, format!("{e:#}"))),
        }
    }
    if format == OutputFormat::Plain {
        for (ok, i) in &rows {
            println!(
                "{} {}",
                if *ok {
                    paint("32", "✔")
                } else {
                    paint("31", "✘")
                },
                i.describe()
            );
        }
        for (shell, e) in &errs {
            println!("{}: {e}", shell.name());
        }
        return Ok(0);
    }
    let v = serde_json::json!({
        "integrations": rows.iter().map(|(ok, i)| serde_json::json!({
            "installed": ok, "description": i.describe(), "shell": i.shell.name(), "file_name": i.file_name(),
        })).collect::<Vec<_>>(),
        "errors": errs.iter().map(|(s, e)| serde_json::json!({
            "shell": s.name(), "error": {"title": e, "message": null},
        })).collect::<Vec<_>>(),
    });
    let out = if format == OutputFormat::Json {
        v.to_string()
    } else {
        serde_json::to_string_pretty(&v)?
    };
    println!("{out}");
    Ok(0)
}

// ---------------------------------------------------------------------------------------------------------------
// setup / uninstall (SPEC §2.9)

/// `[Y/n]` prompt on a tty.
fn confirm(prompt: &str) -> Result<bool> {
    if !std::io::stdin().is_terminal() {
        bail!("You must run with --no-confirm if unattended");
    }
    print!("{prompt}");
    std::io::stdout().flush()?;
    answer(std::io::stdin().lock())
}

/// The `[Y/n]` answer on `input`: empty, `y` or `yes` = yes; end of input (Ctrl-D) = no.
fn answer(mut input: impl std::io::BufRead) -> Result<bool> {
    let mut line = String::new();
    if input.read_line(&mut line)? == 0 {
        println!();
        return Ok(false);
    }
    Ok(matches!(
        line.trim().to_ascii_lowercase().as_str(),
        "" | "y" | "yes"
    ))
}

pub fn setup(args: SetupArgs) -> anyhow::Result<i32> {
    if is_root() && !args.force {
        eprintln!("{}", epaint("1;31", "Installing as root is not supported."));
        eprintln!(
            "{}",
            epaint(
                "31",
                "If you know what you're doing, run the command again with --force."
            )
        );
        return Ok(1);
    }
    let auto = args.no_confirm
        || confirm("Do you want tabsmith to modify your shell config (you will have to manually do this otherwise)? [Y/n] ")?;
    let ctx = Ctx::from_env()?;
    let shells: Vec<Shell> = Shell::ALL
        .into_iter()
        .filter(|s| which(s.name()).is_some())
        .collect();
    let mut manual = !auto;
    if auto {
        match install_dotfiles(&ctx, &shells) {
            Ok(installed) => {
                if installed {
                    mark_restart();
                }
                println!(
                    "{}",
                    if installed {
                        "Installed!"
                    } else {
                        "Already installed"
                    }
                );
            }
            Err(e) => {
                println!("{}", paint("1", "Could not automatically install:"));
                println!("{e:#}");
                manual = true;
            }
        }
    }
    if manual {
        print!("{}", manual_text(&ctx, &shells));
        // Script files only: the user declined (or we failed) to edit the dotfiles themselves.
        let r = apply(
            &ctx,
            &shells,
            |i| i.file_when.is_none(),
            Integration::write_scripts,
        );
        if let Err(e) = r {
            println!("Could not install required files:");
            println!("{e:#}");
        }
    }
    crate::specs::ensure_extracted().context("could not extract the completion specs")?;
    if !manual {
        println!("Restart your terminal (or open a new tab) to start using tabsmith.");
    }
    Ok(0)
}

/// Manual install instructions: the exact lines install writes, so doctor and
/// uninstall recognise them, and their `[ -f … ]` guard keeps a later uninstall from breaking shell startup.
fn manual_text(ctx: &Ctx, shells: &[Shell]) -> String {
    let rc = |shell, file: &str| Integration {
        shell,
        path: ctx.home.join(file),
        file_when: None,
    };
    let mut rcs = vec![
        ("bash:   ", rc(Shell::Bash, ".bashrc")),
        ("zsh:    ", rc(Shell::Zsh, ".zshrc")),
    ];
    if shells.contains(&Shell::Pwsh) {
        rcs.push(("pwsh ($PROFILE):", rc(Shell::Pwsh, "profile")));
    }
    let mut s = String::from("\nTo install the integrations manually, you will have to add the following to your rc files\n");
    s += "This step is required for the application to function properly\n";
    for (when, at) in [(When::Pre, "top"), (When::Post, "bottom")] {
        s += &format!("\nAt the {at} of your .bashrc or .zshrc file:\n");
        for (label, i) in &rcs {
            s += &format!("{label} {}\n", i.source_line(ctx, when));
        }
    }
    s + "\n"
}

pub fn uninstall(args: UninstallArgs) -> anyhow::Result<i32> {
    if !args.no_confirm && !confirm("Are you sure want to continue uninstalling tabsmith? [Y/n] ")?
    {
        println!("Cancelled");
        return Ok(1);
    }
    Ok(uninstall_all(
        &Ctx::from_env()?,
        &default_shells(),
        args.purge,
    ))
}

/// Remove every integration of `shells` (and `ctx.data` on `purge`), carrying on past errors (SPEC §2.9): 1 if any.
fn uninstall_all(ctx: &Ctx, shells: &[Shell], purge: bool) -> i32 {
    let mut failed = apply(ctx, shells, |_| true, Integration::uninstall)
        .map_err(|e| eprintln!("{e:#}"))
        .is_err();
    if purge {
        match std::fs::remove_dir_all(&ctx.data) {
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => {
                eprintln!("Could not remove {}: {e}", ctx.data.display());
                failed = true;
            }
            _ => {}
        }
    }
    if failed {
        println!("Uninstalled tabsmith with errors (see above).");
        return 1;
    }
    println!("Uninstalled tabsmith.");
    if purge {
        println!("Restart open terminals to stop tabsmith in them.");
    }
    println!(
        "Remove the binary with your package manager, or delete {}.",
        ctx.bin
    );
    0
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shared::infrastructure::testing::temp_dir::TempDir;

    const ALL: [(Shell, When); 8] = [
        (Shell::Bash, When::Pre),
        (Shell::Bash, When::Post),
        (Shell::Zsh, When::Pre),
        (Shell::Zsh, When::Post),
        (Shell::Fish, When::Pre),
        (Shell::Fish, When::Post),
        (Shell::Pwsh, When::Pre),
        (Shell::Pwsh, When::Post),
    ];

    fn ctx(dir: &Path) -> Ctx {
        let home = dir.join("home");
        Ctx {
            data: home.join(".tabsmith"),
            bin: "/opt/tab smith/it's/tabsmith".into(), // space and quote exercise the quoting
            zdotdir: None,
            fish_dir: home.join(".config/fish"),
            pwsh_profile: Some(home.join(".config/powershell/Microsoft.PowerShell_profile.ps1")),
            home,
        }
    }

    fn snapshot_path(shell: Shell, when: When) -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/snapshots")
            .join(format!("init_{}_{}.snap", shell.name(), when.name()))
    }

    /// `init` output under TABSMITH_INIT_SNAPSHOT_TEST, as printed (with the trailing newline).
    fn snapshot(shell: Shell, when: When) -> String {
        init_output(shell, when, true).unwrap() + "\n"
    }

    /// Syntax-check `file` with every checker of `shell` that is installed (others are skipped).
    fn syntax_ok(shell: Shell, file: &Path) {
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

    #[test]
    fn integrations_init_snapshots() {
        let bless = std::env::var_os("TABSMITH_BLESS").is_some();
        for (shell, when) in ALL {
            let (path, got) = (snapshot_path(shell, when), snapshot(shell, when));
            if bless || !path.exists() {
                std::fs::write(&path, &got).unwrap();
            }
            assert_eq!(
                std::fs::read_to_string(&path).unwrap(),
                got,
                "{} differs (TABSMITH_BLESS=1 to update)",
                path.display()
            );
            assert!(!got.contains("{{BIN}}"));
        }
        assert!(snapshot(Shell::Zsh, When::Pre)
            .starts_with("TABSMITH_SHELL='/bin/zsh'\nSHOULD_TABSMITH_LAUNCH=0\n"));
        assert!(snapshot(Shell::Fish, When::Pre).starts_with("set -g SHOULD_TABSMITH_LAUNCH 0\n"));
        assert!(snapshot(Shell::Pwsh, When::Pre).starts_with("$TABSMITH_SHOULD_LAUNCH = 0\n"));
        assert!(snapshot(Shell::Pwsh, When::Pre).contains("& 'tabsmith' _ pty"));
    }

    #[test]
    fn integrations_init_syntax() {
        let temp_dir = TempDir::new("syntax");
        let dir = temp_dir.path().to_path_buf();
        for (shell, when) in ALL {
            let f = dir.join(format!("{}_{}", shell.name(), when.name()));
            std::fs::write(&f, snapshot(shell, when)).unwrap();
            syntax_ok(shell, &f);
        }
        let _ = std::fs::remove_dir_all(dir);
    }

    /// Source each post script in a real shell, run its prompt hook twice (re-wrap must not nest) and check the
    /// OSC 697 markers. Skipped per shell when not installed.
    #[test]
    fn integrations_post_scripts_emit_markers() {
        let temp_dir = TempDir::new("runtime");
        let dir = temp_dir.path().to_path_buf();
        let cases: [(Shell, &str, &[&str], &str); 3] = [
            (
                Shell::Bash,
                "bash",
                &["--norc", "--noprofile", "-c"],
                r#"PROMPT_COMMAND=$'history -a; '; PS1='$ '; source "$F"; (IFS=';'; echo "PC=${PROMPT_COMMAND[*]}"); _ts_first; _ts_last; _ts_first; _ts_last; printf 'PS1=%s\n' "$PS1""#,
            ),
            (
                Shell::Zsh,
                "zsh",
                &["-f", "-c"],
                r#"PS1='$ '; source "$F"; _ts_precmd; _ts_precmd; print -r -- "PS1=$PS1 last=${precmd_functions[-1]} first=${preexec_functions[1]}""#,
            ),
            (
                Shell::Fish,
                "fish",
                &["--no-config", "-c"],
                r#"function fish_prompt; echo -n '$ '; end; source $F; _ts_precmd; _ts_precmd; echo; echo PS1=(fish_prompt)"#,
            ),
        ];
        for (shell, prog, args, script) in cases {
            if which(prog).is_none() {
                continue;
            }
            let f = dir.join(format!("post.{prog}"));
            std::fs::write(&f, snapshot(shell, When::Post)).unwrap();
            let out = Command::new(prog)
                .args(args)
                .arg(script)
                .env("F", &f)
                .env("TABSMITH_SESSION_ID", "abc")
                .env_remove("_TS_POST_LOADED")
                .env_remove("VSCODE_INJECTION") // run in a VS Code terminal, the post block would load VS Code's script
                .output()
                .unwrap();
            let s = String::from_utf8_lossy(&out.stdout);
            assert!(
                out.status.success(),
                "{prog}: {s}{}",
                String::from_utf8_lossy(&out.stderr)
            );
            for p in [
                "OSCUnlock=abc\x07",
                "Dir=/",
                "ExitCode=0\x07",
                &format!("Shell={prog}\x07"),
            ] {
                assert!(
                    s.contains(&format!("\x1b]697;{p}")),
                    "{prog} did not emit {p:?}: {s:?}"
                );
            }
            let ps1 = s.rsplit("PS1=").next().unwrap_or_default();
            assert_eq!(
                ps1.matches("StartPrompt").count(),
                1,
                "{prog} prompt wrapped once: {ps1:?}"
            );
            assert!(ps1.contains("NewCmd=abc"), "{prog}: {ps1:?}");
            match shell {
                Shell::Bash => assert!(s.contains("PC=_ts_first;history -a;_ts_last\n"), "{s:?}"),
                Shell::Zsh => assert!(s.contains("last=_ts_precmd first=_ts_preexec"), "{s:?}"),
                _ => {}
            }
        }
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn integrations_dotfile_install_uninstall() {
        let temp_dir = TempDir::new("dotfile");
        let dir = temp_dir.path().to_path_buf();
        let c = ctx(&dir);
        std::fs::create_dir_all(&c.home).unwrap();
        let zshrc = c.home.join(".zshrc");
        let original = "#!/bin/zsh\nexport A=1\nalias ll='ls -l'\n";
        std::fs::write(&zshrc, original).unwrap();

        let rc = Integration {
            shell: Shell::Zsh,
            path: zshrc.clone(),
            file_when: None,
        };
        assert!(matches!(
            rc.is_installed(&c),
            Err(IntegrationError::NotInstalled(_))
        ));
        rc.install(&c).unwrap();
        rc.is_installed(&c).unwrap();
        let once = std::fs::read_to_string(&zshrc).unwrap();
        let lines: Vec<&str> = once.lines().collect();
        assert_eq!(lines[0], "#!/bin/zsh");
        assert_eq!(lines[1], PRE_MARKER);
        assert_eq!(
            lines[2],
            r#"[ -f "${HOME}/.tabsmith/shell/zshrc.pre.zsh" ] && . "${HOME}/.tabsmith/shell/zshrc.pre.zsh""#
        );
        assert_eq!(lines[lines.len() - 2], POST_MARKER);
        assert!(lines[lines.len() - 1].ends_with(r#". "${HOME}/.tabsmith/shell/zshrc.post.zsh""#));
        assert_eq!(
            std::fs::read_to_string(c.data.join("shell/zshrc.pre.zsh")).unwrap(),
            "[ -x '/opt/tab smith/it'\\''s/tabsmith' ] && eval \"$('/opt/tab smith/it'\\''s/tabsmith' init zsh pre --rcfile zshrc)\"\n"
        );

        // Idempotent: a second install changes nothing.
        rc.install(&c).unwrap();
        assert_eq!(std::fs::read_to_string(&zshrc).unwrap(), once);

        // Backup of the original.
        let backups: Vec<_> = std::fs::read_dir(c.data.join("backups"))
            .unwrap()
            .flatten()
            .collect();
        assert_eq!(backups.len(), 1);
        assert_eq!(
            std::fs::read_to_string(backups[0].path().join(".zshrc")).unwrap(),
            original
        );

        // The installed dotfile and script files still parse.
        syntax_ok(Shell::Zsh, &zshrc);

        // Uninstall restores the original and deletes the script files.
        rc.uninstall(&c).unwrap();
        assert_eq!(std::fs::read_to_string(&zshrc).unwrap(), original);
        assert!(!c.data.join("shell/zshrc.pre.zsh").exists());
        assert!(!c.data.join("shell/zshrc.post.zsh").exists());
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn integrations_dotfile_detection() {
        let temp_dir = TempDir::new("detect");
        let dir = temp_dir.path().to_path_buf();
        let c = ctx(&dir);
        let bashrc = c.home.join(".bashrc");
        let rc = Integration {
            shell: Shell::Bash,
            path: bashrc.clone(),
            file_when: None,
        };
        assert!(matches!(
            rc.is_installed(&c),
            Err(IntegrationError::FileDoesNotExist(_))
        ));

        rc.install(&c).unwrap(); // creates the file
        let installed = std::fs::read_to_string(&bashrc).unwrap();
        syntax_ok(Shell::Bash, &bashrc);

        std::fs::write(&bashrc, format!("echo first\n{installed}")).unwrap();
        let e = rc.is_installed(&c).unwrap_err().to_string();
        assert!(e.ends_with("does not source pre integration first"), "{e}");

        std::fs::write(
            &bashrc,
            format!("{installed}echo last\n# trailing comment\n\n"),
        )
        .unwrap();
        let e = rc.is_installed(&c).unwrap_err().to_string();
        assert!(
            e.starts_with("Improper integration installation: ")
                && e.ends_with("post integration last"),
            "{e}"
        );

        // A moved binary: script bytes differ.
        std::fs::write(&bashrc, &installed).unwrap();
        let moved = Ctx {
            bin: "/elsewhere/tabsmith".into(),
            ..c.clone()
        };
        let e = rc.is_installed(&moved).unwrap_err().to_string();
        assert!(e.contains("bashrc.pre.bash should contain:\n[ -n \"$BASH_VERSION\" ] && [ -x '/elsewhere/tabsmith' ]"), "{e}");
        rc.install(&moved).unwrap();
        rc.is_installed(&moved).unwrap();

        // Opt-out marker wins.
        std::fs::write(&bashrc, "  #  TABSMITH ignore\necho hi\n").unwrap();
        rc.is_installed(&c).unwrap();
        rc.install(&c).unwrap();
        assert_eq!(
            std::fs::read_to_string(&bashrc).unwrap(),
            "  #  TABSMITH ignore\necho hi\n"
        );
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn integrations_enumeration_and_whole_files() {
        let temp_dir = TempDir::new("enum");
        let dir = temp_dir.path().to_path_buf();
        let c = ctx(&dir);
        std::fs::create_dir_all(&c.home).unwrap();
        let names = |s| {
            integrations(&c, s)
                .unwrap()
                .iter()
                .map(Integration::file_name)
                .collect::<Vec<_>>()
        };
        assert_eq!(names(Shell::Bash), [".bashrc", ".profile"]);
        std::fs::write(c.home.join(".bash_profile"), "").unwrap();
        assert_eq!(names(Shell::Bash), [".bashrc", ".bash_profile"]);
        assert_eq!(names(Shell::Zsh), [".zshrc", ".zprofile"]);

        let fish = integrations(&c, Shell::Fish).unwrap();
        assert_eq!(
            fish.iter().map(Integration::describe).collect::<Vec<_>>(),
            ["fish pre", "fish post"]
        );
        for i in &fish {
            i.install(&c).unwrap();
            i.is_installed(&c).unwrap();
            syntax_ok(Shell::Fish, &i.path);
        }
        assert_eq!(
            std::fs::read_to_string(c.fish_dir.join("conf.d/00_tabsmith_pre.fish")).unwrap(),
            "test -x '/opt/tab smith/it\\'s/tabsmith'; and eval ('/opt/tab smith/it\\'s/tabsmith' init fish pre --rcfile 00_tabsmith_pre | string split0)\n"
        );
        for i in &fish {
            i.uninstall(&c).unwrap();
            assert!(!i.path.exists());
        }

        // pwsh: the profile directory is created; script files are `profile.<when>.ps1`.
        let pwsh = &integrations(&c, Shell::Pwsh).unwrap()[0];
        assert_eq!(
            pwsh.describe(),
            "pwsh pre post into Microsoft.PowerShell_profile.ps1"
        );
        pwsh.install(&c).unwrap();
        pwsh.is_installed(&c).unwrap();
        let text = std::fs::read_to_string(&pwsh.path).unwrap();
        assert!(text.contains(r#"if (Test-Path "${HOME}/.tabsmith/shell/profile.pre.ps1") { . "${HOME}/.tabsmith/shell/profile.pre.ps1" }"#));
        syntax_ok(Shell::Pwsh, &pwsh.path);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn integrations_install_dotfiles_reports() {
        let temp_dir = TempDir::new("apply");
        let dir = temp_dir.path().to_path_buf();
        let c = ctx(&dir);
        assert!(install_dotfiles(&c, &[Shell::Zsh, Shell::Fish]).unwrap());
        assert!(!install_dotfiles(&c, &[Shell::Zsh, Shell::Fish]).unwrap());
        // A dotfile that cannot be written is reported with its description.
        std::fs::create_dir_all(c.home.join(".bashrc")).unwrap(); // a directory where the file should be
        let e = format!("{:#}", install_dotfiles(&c, &[Shell::Bash]).unwrap_err());
        assert!(
            e.starts_with("\n\n")
                && e.contains("bash pre post into .bashrc")
                && !e.contains(".profile"),
            "{e}"
        );
        let _ = std::fs::remove_dir_all(dir);
    }

    #[cfg(unix)]
    #[test]
    #[allow(clippy::type_complexity)]
    fn integrations_should_launch_table() {
        let base = [("TERM", "xterm-256color")];
        let cases: &[(&[(&str, &str)], bool, bool, &str)] = &[
            (&[], true, true, "✅ launching"),
            (&[], false, false, "❌ stdin is not a tty"),
            (
                &[("TABSMITH_FORCE_LAUNCH", "1"), ("CI", "1")],
                false,
                true,
                "✅ TABSMITH_FORCE_LAUNCH",
            ),
            (
                &[("TABSMITH_TERM_DISABLED", "1")],
                true,
                false,
                "❌ TABSMITH_TERM_DISABLED",
            ),
            (
                &[("PROCESS_LAUNCHED_BY_TABSMITH", "1")],
                true,
                false,
                "❌ PROCESS_LAUNCHED_BY_TABSMITH",
            ),
            (
                &[("TABSMITH_TERM", "0.1.0")],
                true,
                false,
                "❌ already inside tabsmith",
            ),
            (
                &[("TABSMITH_TERM", "0.1.0"), ("TMUX", "/tmp/t")],
                true,
                true,
                "✅ new tmux pane",
            ),
            (
                &[
                    ("TABSMITH_TERM", "0.1.0"),
                    ("TMUX", "/tmp/t"),
                    ("TABSMITH_TERM_TMUX", "0.1.0"),
                ],
                true,
                false,
                "❌ already inside tabsmith",
            ),
            (&[("INSIDE_EMACS", "29")], true, false, "❌ INSIDE_EMACS"),
            (
                &[("TERM_PROGRAM", "WarpTerminal")],
                true,
                false,
                "❌ TERM_PROGRAM = WarpTerminal",
            ),
            (&[("TERM_PROGRAM", "iTerm.app")], true, true, "✅ launching"),
            (
                &[("__CFBundleIdentifier", "com.vandyke.SecureCRT")],
                true,
                false,
                "❌ __CFBundleIdentifier = com.vandyke.SecureCRT",
            ),
            (
                &[("__PWSH_LOGIN_CHECKED", "1")],
                true,
                false,
                "❌ __PWSH_LOGIN_CHECKED",
            ),
            (&[("CI", "true")], true, false, "❌ In CI"),
            (
                &[("VSCODE_RESOLVING_ENVIRONMENT", "1")],
                true,
                false,
                "❌ VSCODE_RESOLVING_ENVIRONMENT",
            ),
            (&[("TERM", "dumb")], true, false, "❌ TERM is unset or dumb"),
            (&[("SSH_TTY", "/dev/ttys001")], true, true, "✅ launching"),
        ];
        for (vars, stdin_ok, launch, why) in cases {
            let env: HashMap<String, String> = base
                .iter()
                .chain(vars.iter())
                .map(|(k, v)| (k.to_string(), v.to_string()))
                .collect();
            assert_eq!(
                launch_decision(&env, *stdin_ok),
                (*launch, why.to_string()),
                "{vars:?}"
            );
        }
        assert_eq!(
            launch_decision(&HashMap::new(), true),
            (false, "❌ TERM is unset or dumb".to_string())
        );
    }

    /// The file modes and the purge guard of this scenario are asserted in shared atomic_file.rs
    /// (`integrations_pre_cmd_file`); this half keeps the SessionCtx round trip.
    #[cfg(unix)]
    #[test]
    fn integrations_pre_cmd_file_session_ctx() {
        let temp_dir = TempDir::new("precmd");
        let dir = temp_dir.path().to_path_buf();
        let path = dir.join("run").join("abc123.json");
        let ctx = SessionCtx {
            env: [("A".to_string(), "1".to_string())].into(),
            alias: Some("ll='ls -l'".into()),
            ts: 7,
        };
        AtomicFile::new(&path).write_private(serde_json::to_string(&ctx).unwrap().as_bytes()).unwrap();
        let back = SessionCtx::load(&path).unwrap();
        assert_eq!(
            (back.env["A"].as_str(), back.alias.as_deref(), back.ts),
            ("1", Some("ll='ls -l'"), 7)
        );
        let _ = std::fs::remove_dir_all(dir);
    }

    /// Run `argv` in a PTY with only PATH, TERM and `env`; type `input` once `ready` appears in the output (at once
    /// when `ready` is empty). Answers cursor-position and device-attribute queries. Returns (output, exit code).
    #[cfg(unix)]
    fn pty_run(argv: &[&str], env: &[(&str, &Path)], ready: &str, input: &str) -> (String, u32) {
        use portable_pty::{native_pty_system, CommandBuilder, PtySize};
        use std::io::Read;
        use std::time::Instant;
        let pair = native_pty_system()
            .openpty(PtySize {
                rows: 24,
                cols: 100,
                pixel_width: 0,
                pixel_height: 0,
            })
            .unwrap();
        let mut cb = CommandBuilder::from_argv(argv.iter().map(Into::into).collect());
        cb.env_clear();
        cb.env("PATH", std::env::var_os("PATH").unwrap_or_default());
        cb.env("TERM", "xterm-256color");
        for (k, v) in env {
            cb.env(k, v);
        }
        let mut child = pair.slave.spawn_command(cb).unwrap();
        drop(pair.slave);
        let mut reader = pair.master.try_clone_reader().unwrap();
        let mut writer = pair.master.take_writer().unwrap();
        let (tx, rx) = std::sync::mpsc::channel::<Vec<u8>>();
        std::thread::spawn(move || {
            let mut b = [0u8; 4096];
            while let Ok(n @ 1..) = reader.read(&mut b) {
                if tx.send(b[..n].to_vec()).is_err() {
                    break;
                }
            }
        });
        let (mut out, mut sent, deadline) =
            (Vec::new(), false, Instant::now() + Duration::from_secs(40));
        loop {
            if let Some(status) = child.try_wait().unwrap() {
                std::thread::sleep(Duration::from_millis(100));
                out.extend(rx.try_iter().flatten());
                return (
                    String::from_utf8_lossy(&out).into_owned(),
                    status.exit_code(),
                );
            }
            if Instant::now() > deadline {
                let _ = child.kill();
                panic!("{argv:?} did not exit: {:?}", String::from_utf8_lossy(&out));
            }
            if let Ok(chunk) = rx.recv_timeout(Duration::from_millis(50)) {
                let has = |p: &[u8]| chunk.windows(p.len()).any(|w| w == p);
                if has(b"\x1b[6n") {
                    let _ = writer.write_all(b"\x1b[1;1R");
                }
                if has(b"\x1b[c") || has(b"\x1b[0c") {
                    let _ = writer.write_all(b"\x1b[?62;22c");
                }
                out.extend(chunk);
            }
            if !sent && String::from_utf8_lossy(&out).contains(ready) {
                let _ = writer.write_all(input.as_bytes());
                sent = true;
            }
        }
    }

    /// A fake tabsmith: `init <shell> <when>` prints `<fake>.<when>`; `_ pty` records whether stdin is a tty in
    /// `<fake>.launched` and its arguments (one per line) in `<fake>.args`, then exits 7.
    #[cfg(unix)]
    fn fake_bin(dir: &Path) -> PathBuf {
        use std::os::unix::fs::PermissionsExt;
        std::fs::create_dir_all(dir).unwrap();
        let fake = dir.join("tabsmith");
        let script = "#!/bin/sh\ncase \"$1 $2\" in\ninit*) cat \"$0.$3\" ;;\n\"_ pty\") printf '%s\\n' \"$@\" > \"$0.args\"; if [ -t 0 ]; then echo tty; else echo notty; fi > \"$0.launched\"; exit 7 ;;\nesac\n";
        std::fs::write(&fake, script).unwrap();
        std::fs::set_permissions(&fake, std::fs::Permissions::from_mode(0o755)).unwrap();
        fake
    }

    /// `init` output for `fake` (the snapshot with its binary swapped).
    fn init_for(shell: Shell, when: When, bin: &str) -> String {
        snapshot(shell, when).replace(&quote(shell, "tabsmith"), &quote(shell, bin))
    }

    /// The installed fish conf.d files really start the wrapper (stdin a tty) and see the tty in the post block.
    #[cfg(unix)]
    #[test]
    fn integrations_fish_confd_launches_in_a_pty() {
        if which("fish").is_none() {
            return;
        }
        let temp_dir = TempDir::new("fishpty");
        let dir = temp_dir.path().to_path_buf();
        let home = dir.join("home");
        let fake = fake_bin(&dir.join("bin dir"));
        let bin = fake.display().to_string();
        let c = Ctx {
            bin: bin.clone(),
            fish_dir: home.join(".config/fish"),
            ..ctx(&dir)
        };
        for i in integrations(&c, Shell::Fish).unwrap() {
            i.install(&c).unwrap();
        }
        std::fs::write(format!("{bin}.pre"), init_for(Shell::Fish, When::Pre, &bin)).unwrap();
        std::fs::write(
            format!("{bin}.post"),
            init_for(Shell::Fish, When::Post, &bin),
        )
        .unwrap();
        let env: &[(&str, &Path)] = &[("HOME", &home), ("XDG_CONFIG_HOME", &home.join(".config"))];

        let (out, code) = pty_run(&["fish", "-i"], env, "", "exit\r");
        let launched = std::fs::read_to_string(format!("{bin}.launched")).unwrap_or_default();
        assert_eq!(
            (launched.trim(), code),
            ("tty", 7),
            "the pre block must exec the wrapper on the tty: {out:?}"
        );
        let args = || std::fs::read_to_string(format!("{bin}.args")).unwrap_or_default();
        assert!(args().ends_with("_\npty\n--\n"), "{:?}", args());
        // `fish --private` stays private inside the wrapper.
        let (out, code) = pty_run(&["fish", "--private", "-i"], env, "", "exit\r");
        assert!(
            code == 7 && args().ends_with("_\npty\n--\n--private\n"),
            "{:?} {out:?}",
            args()
        );

        // Inside the wrapper (launch decision 1, a session id): the post block reads the real tty.
        let pre = init_for(Shell::Fish, When::Pre, &bin)
            .replace("SHOULD_TABSMITH_LAUNCH 0", "SHOULD_TABSMITH_LAUNCH 1");
        std::fs::write(format!("{bin}.pre"), pre).unwrap();
        let env = [env, &[("TABSMITH_SESSION_ID", Path::new("abc"))]].concat();
        let (out, code) = pty_run(&["fish", "-i"], &env, "NewCmd=abc", "exit\r");
        assert_eq!(code, 0, "{out:?}");
        assert!(
            out.contains("\x1b]697;TTY=/dev/"),
            "post block TTY: {out:?}"
        );
        let _ = std::fs::remove_dir_all(dir);
    }

    /// PROMPT_COMMAND edited after install (`; true` appended, bash-preexec's newline-separated layout) is put back
    /// to `_ts_first;…;_ts_last` (bash 5.1+: the array `_ts_first …user… _ts_last`), and PS1 is wrapped exactly
    /// once at every prompt.
    #[test]
    fn integrations_bash_prompt_command_rewrap() {
        if which("bash").is_none() {
            return;
        }
        let temp_dir = TempDir::new("bashpc");
        let dir = temp_dir.path().to_path_buf();
        let f = dir.join("post.bash");
        std::fs::write(&f, snapshot(Shell::Bash, When::Post)).unwrap();
        let script = r#"PS1='$ '; source "$F"; __bp_a() { :; }; __bp_b() { :; }
            pc() { local c; for c in "${PROMPT_COMMAND[@]}"; do eval "$c"; done; }
            prompts() { for _ in 1 2 3 4; do pc; printf '\nLEN=%s\n' "${#PS1}"; done; local IFS='|'; printf 'PC=%q\n' "${PROMPT_COMMAND[*]}"; }
            (( BASH_VERSINFO[0] > 5 || (BASH_VERSINFO[0] == 5 && BASH_VERSINFO[1] >= 1) )) && echo ARRAY
            pc; PROMPT_COMMAND="$PROMPT_COMMAND; true"; prompts
            PROMPT_COMMAND=$'__bp_a\n'"$PROMPT_COMMAND"$'\n__bp_b'; prompts"#;
        let out = Command::new("bash")
            .args(["--norc", "--noprofile", "-c", script])
            .env("F", &f)
            .env("TABSMITH_SESSION_ID", "abc")
            .env_remove("_TS_POST_LOADED")
            .env_remove("VSCODE_INJECTION") // run in a VS Code terminal, the post block would load VS Code's script
            .output()
            .unwrap();
        let s = String::from_utf8_lossy(&out.stdout);
        assert!(
            out.status.success(),
            "{s}{}",
            String::from_utf8_lossy(&out.stderr)
        );
        let lens: Vec<&str> = s.lines().filter(|l| l.starts_with("LEN=")).collect();
        assert!(
            lens.len() == 8 && lens.iter().all(|l| *l == lens[0]),
            "PS1 grows: {s}"
        );
        if s.contains("ARRAY\n") {
            // `$PROMPT_COMMAND` is element 0 of the array: the user's edits land there.
            assert!(s.contains("PC=_ts_first\\|true\\|_ts_last\n"), "{s}");
            assert!(
                s.contains("PC=$'_ts_first|__bp_a\\n__bp_b|true|_ts_last'\n"),
                "{s}"
            );
        } else {
            assert!(s.contains("PC=_ts_first\\;true\\;_ts_last\n"), "{s}");
            assert!(
                s.contains("PC=$'_ts_first;__bp_a\\ntrue;__bp_b;_ts_last'\n"),
                "{s}"
            );
        }
        let _ = std::fs::remove_dir_all(dir);
    }

    /// pwsh (when installed): PreExec comes from the PSConsoleHostReadLine wrapper for every line, including a
    /// repeated one (not for an empty / Ctrl-C line), without touching PSReadLine's history handler or $Error.
    #[test]
    fn integrations_pwsh_post_preexec() {
        if which("pwsh").is_none() {
            return;
        }
        let temp_dir = TempDir::new("pwshpost");
        let dir = temp_dir.path().to_path_buf();
        let f = dir.join("post.ps1");
        std::fs::write(&f, snapshot(Shell::Pwsh, When::Post)).unwrap();
        let script = format!(
            "$Global:__l = 'echo hi'; function Global:PSConsoleHostReadLine {{ $Global:__l }}; . {}; $a = PSConsoleHostReadLine; $b = PSConsoleHostReadLine; $Global:__l = ''; $null = PSConsoleHostReadLine; \"`nGOT=$a|$b ERR=$($Error.Count)\"; prompt",
            quote(Shell::Pwsh, &f.display().to_string())
        );
        let out = Command::new("pwsh")
            .args(["-NoProfile", "-Command", &script])
            .env("TABSMITH_SESSION_ID", "abc")
            .output()
            .unwrap();
        let s = String::from_utf8_lossy(&out.stdout);
        assert!(
            out.status.success(),
            "{s}{}",
            String::from_utf8_lossy(&out.stderr)
        );
        assert_eq!(
            s.matches("\x1b]697;OSCLock=abc\x07\x1b]697;PreExec\x07")
                .count(),
            2,
            "{s:?}"
        );
        assert!(s.contains("GOT=echo hi|echo hi ERR=0"), "{s:?}");
        assert!(
            s.contains("\x1b]697;StartPrompt\x07") && s.contains("\x1b]697;NewCmd=abc\x07"),
            "{s:?}"
        );
        let _ = std::fs::remove_dir_all(dir);
    }

    /// pwsh (when installed): the user's prompt and PSReadLine's ReadLine see the command's `$?`; a failed cmdlet
    /// reports exit 1 even after a native exit code; a prompt redefined later (`. $PROFILE`, oh-my-posh) is
    /// re-wrapped at the next ReadLine, which tracks the line already drawn; re-sourcing never double-wraps.
    #[cfg(unix)]
    #[test]
    fn integrations_pwsh_post_status_and_rewrap() {
        let temp_dir = TempDir::new("pwshstatus");
        let dir = temp_dir.path().to_path_buf();
        let script = r#"Set-StrictMode -Version Latest
            function Global:prompt { "[$?|$global:LASTEXITCODE]> " }
            function Global:PSConsoleHostReadLine { $s = $?; "line$s" }
            . $env:F
            sh -c 'exit 3'; $r1 = PSConsoleHostReadLine; Get-Item nope -ErrorAction SilentlyContinue; $p1 = prompt
            $null = PSConsoleHostReadLine; sh -c 'exit 4'; $p2 = prompt
            function Global:prompt { 'MY> ' }; "`nREWRAP"; $null = PSConsoleHostReadLine; "`nAFTER"; $p3 = prompt
            . $env:F; $r4 = PSConsoleHostReadLine
            $PSNativeCommandUseErrorActionPreference = $true; $null = PSConsoleHostReadLine; sh -c 'exit 5' 2>$null; $p5 = prompt
            "`nR=$r1|$r4 P1=$p1 P2=$p2 P3=$p3 P5=$p5 ERR=$($Error.Count)""#;
        let Some(s) = run_post(
            "pwsh",
            &["-NoProfile", "-Command"],
            &snapshot(Shell::Pwsh, When::Post),
            script,
            &dir,
        ) else {
            return;
        };
        let osc = |p: &str| format!("\x1b]697;{p}\x07");
        let last = s.rsplit("\nR=").next().unwrap_or_default();
        assert!(
            last.starts_with("lineFalse|lineTrue "),
            "ReadLine sees $?: {last:?}"
        );
        let p1 = last.split(" P1=").nth(1).unwrap_or_default();
        assert!(
            p1.starts_with(&osc("OSCUnlock=abc")) && p1.contains(&osc("ExitCode=1")),
            "stale LASTEXITCODE: {p1:?}"
        );
        assert!(
            p1.contains(&format!(
                "{}[False|3]> {}",
                osc("StartPrompt"),
                osc("EndPrompt")
            )),
            "prompt sees $?: {p1:?}"
        );
        assert!(
            p1.contains(&format!("{}{}", osc("NewCmd=abc"), osc("PwshListView=0"))),
            "{p1:?}"
        );
        let p2 = last.split(" P2=").nth(1).unwrap_or_default();
        assert!(
            p2.contains(&osc("ExitCode=4")) && p2.contains("[False|4]> "),
            "{p2:?}"
        );
        // The foreign prompt is tracked like any other: the session's shell, pid and dir come with it.
        let rewrap = s
            .split("\nREWRAP")
            .nth(1)
            .and_then(|r| r.split("\nAFTER").next())
            .unwrap_or_default();
        assert!(
            rewrap.trim_start().starts_with(&format!(
                "{}{}{}\x1b]697;PID=",
                osc("OSCUnlock=abc"),
                osc("ExitCode=0"),
                osc("Shell=pwsh")
            )),
            "{rewrap:?}"
        );
        assert!(
            rewrap.contains("\x1b]697;Dir=/")
                && rewrap.contains(&format!("{}{}", osc("NewCmd=abc"), osc("PwshListView=0"))),
            "{rewrap:?}"
        );
        assert!(!rewrap.contains("StartPrompt"), "{rewrap:?}");
        let p3 = last.split(" P3=").nth(1).unwrap_or_default();
        assert!(
            p3.contains(&format!("{}MY> {}", osc("StartPrompt"), osc("EndPrompt"))),
            "{p3:?}"
        );
        let p5 = last.split(" P5=").nth(1).unwrap_or_default();
        assert!(
            p5.contains(&osc("ExitCode=5")),
            "native failure with an error record: {p5:?}"
        );
        assert_eq!(
            s.matches(&osc("PreExec")).count(),
            5,
            "one PreExec per ReadLine: {s:?}"
        );
        assert!(
            last.contains(" ERR=2"),
            "only Get-Item's and sh's errors: {last:?}"
        );
        let _ = std::fs::remove_dir_all(dir);
    }

    /// pwsh (when installed), interactive in a PTY with PSReadLine: a line its history handler keeps out of the file
    /// (the default sensitive-line filter) sends NoHistory; ListView predictions send PwshListView=1.
    #[cfg(unix)]
    #[test]
    fn integrations_pwsh_no_history_and_listview() {
        if which("pwsh").is_none() {
            return;
        }
        let temp_dir = TempDir::new("pwshnohist");
        let dir = temp_dir.path().to_path_buf();
        let home = dir.join("home");
        std::fs::create_dir_all(&home).unwrap();
        let f = dir.join("post.ps1");
        std::fs::write(&f, snapshot(Shell::Pwsh, When::Post)).unwrap();
        let env: &[(&str, &Path)] = &[
            ("HOME", &home),
            ("F", &f),
            ("TABSMITH_SESSION_ID", Path::new("abc")),
            ("TABSMITH_HOME", &dir.join("data")),
        ];
        let lv = "Set-PSReadLineOption -PredictionSource History -PredictionViewStyle ListView";
        for (setup, want_lv) in [("$null = 1", "PwshListView=0"), (lv, "PwshListView=1")] {
            let argv = [
                "pwsh",
                "-NoProfile",
                "-NoLogo",
                "-NoExit",
                "-Command",
                &format!("{setup}; . $env:F"),
            ];
            let (out, code) = pty_run(
                &argv,
                env,
                "NewCmd=abc",
                "echo hi\r$apikey = 'sk-1'\rexit\r",
            );
            assert_eq!(code, 0, "{out:?}");
            let got: Vec<bool> = out
                .split("\x1b]697;PreExec\x07")
                .take(3)
                .map(|c| c.contains("NoHistory"))
                .collect();
            assert_eq!(got, [false, true, false], "{out:?}");
            assert!(
                out.contains(&format!(
                    "\x1b]697;OSCLock=abc\x07\x1b]697;NoHistory\x07\x1b]697;PreExec"
                )),
                "{out:?}"
            );
            assert!(
                out.contains(&format!("\x1b]697;NewCmd=abc\x07\x1b]697;{want_lv}\x07")),
                "{out:?}"
            );
        }
        let _ = std::fs::remove_dir_all(dir);
    }

    /// pwsh (when installed) under an OEM console code page: the profile script reads the UTF-8 init output (a
    /// non-ASCII binary path) intact and restores the code page; `$PROFILE` is read back as UTF-8.
    #[cfg(unix)]
    #[test]
    fn integrations_pwsh_utf8_under_oem_code_page() {
        if which("pwsh").is_none() {
            return;
        }
        let temp_dir = TempDir::new("pwshoem");
        let dir = temp_dir.path().to_path_buf();
        let bin = fake_bin(&dir.join("Jos\u{e9}")).display().to_string();
        std::fs::write(format!("{bin}.pre"), "$Global:__tsGot = 'Jos\u{e9}'\n").unwrap();
        let c = Ctx { bin, ..ctx(&dir) };
        let pwsh = &integrations(&c, Shell::Pwsh).unwrap()[0];
        let pre = dir.join("profile.pre.ps1");
        std::fs::write(&pre, pwsh.script_contents(&c, When::Pre)).unwrap();
        let oem = "[Console]::OutputEncoding = [Text.Encoding]::GetEncoding(850)";
        let script = format!(
            "{oem}; . {}; $cp = [Console]::OutputEncoding.CodePage; [Console]::OutputEncoding = [Text.UTF8Encoding]::new(); \"$cp $($Global:__tsGot -eq ('Jos' + [char]0xE9))\"",
            quote(Shell::Pwsh, &pre.display().to_string())
        );
        let out = Command::new("pwsh")
            .args(["-NoProfile", "-Command", &script])
            .output()
            .unwrap();
        assert_eq!(
            String::from_utf8_lossy(&out.stdout).trim(),
            "850 True",
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );

        let script =
            format!("{oem}; $PROFILE = '/Users/Jos' + [char]0xE9 + '/p.ps1'; {PWSH_PROFILE_CMD}");
        let out = Command::new("pwsh")
            .args(["-NoProfile", "-Command", &script])
            .output()
            .unwrap();
        assert_eq!(
            String::from_utf8(out.stdout).ok().as_deref(),
            Some("/Users/Jos\u{e9}/p.ps1")
        );

        // Both encoding setters tolerate a missing console (Windows); empty init output runs nothing, no error.
        let line = pwsh.script_contents(&c, When::Pre);
        assert!(
            line.contains(
                "try { [Console]::OutputEncoding = [Text.UTF8Encoding]::new() } catch {}"
            ),
            "{line}"
        );
        assert!(
            line.contains("try { [Console]::OutputEncoding = $__tsEnc } catch {}"),
            "{line}"
        );
        std::fs::write(format!("{}.pre", c.bin), "").unwrap();
        let script = format!(
            ". {}; \"ERR=$($Error.Count)\"",
            quote(Shell::Pwsh, &pre.display().to_string())
        );
        let out = Command::new("pwsh")
            .args(["-NoProfile", "-Command", &script])
            .output()
            .unwrap();
        assert_eq!(
            String::from_utf8_lossy(&out.stdout).trim(),
            "ERR=0",
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
        let _ = std::fs::remove_dir_all(dir);
    }

    /// A UTF-8 BOM (VS Code, ISE and Windows PowerShell save .ps1 files with one) stays the first bytes: install
    /// puts the pre block after it, is_installed sees through it, a reinstall is a no-op, uninstall keeps it.
    #[test]
    fn integrations_dotfile_bom_stays_first() {
        let temp_dir = TempDir::new("bom");
        let dir = temp_dir.path().to_path_buf();
        let c = ctx(&dir);
        let pwsh = &integrations(&c, Shell::Pwsh).unwrap()[0];
        std::fs::create_dir_all(pwsh.path.parent().unwrap()).unwrap();
        let user = "Write-Host USERLINE\r\nSet-PSReadLineOption -PredictionSource History\r\n";
        std::fs::write(&pwsh.path, format!("\u{feff}{user}")).unwrap();
        pwsh.install(&c).unwrap();
        let once = std::fs::read_to_string(&pwsh.path).unwrap();
        assert!(
            once.starts_with(&format!("\u{feff}{PRE_MARKER}\n"))
                && once.matches('\u{feff}').count() == 1,
            "{once:?}"
        );
        assert!(
            once.contains(&format!("\nWrite-Host USERLINE\r\n")),
            "{once:?}"
        );
        pwsh.is_installed(&c).unwrap();
        pwsh.install(&c).unwrap();
        assert_eq!(std::fs::read_to_string(&pwsh.path).unwrap(), once);
        syntax_ok(Shell::Pwsh, &pwsh.path);

        // An editor re-saves a plain install with a BOM: still installed; a forced reinstall keeps one block.
        pwsh.uninstall(&c).unwrap();
        let back = std::fs::read_to_string(&pwsh.path).unwrap();
        assert!(
            back.starts_with("\u{feff}Write-Host USERLINE\r\n") && !back.contains("tabsmith"),
            "{back:?}"
        );
        std::fs::write(&pwsh.path, user).unwrap();
        pwsh.install(&c).unwrap();
        let plain = std::fs::read_to_string(&pwsh.path).unwrap();
        std::fs::write(&pwsh.path, format!("\u{feff}{plain}")).unwrap();
        pwsh.is_installed(&c).unwrap();
        pwsh.uninstall(&c).unwrap();
        pwsh.install(&c).unwrap();
        let again = std::fs::read_to_string(&pwsh.path).unwrap();
        assert_eq!(again, format!("\u{feff}{plain}"));
        let _ = std::fs::remove_dir_all(dir);
    }

    /// Run `script` in `prog args… script` with `post` written to $F, session `abc`, data dir under `dir`.
    /// None when `prog` is not installed.
    fn run_post(prog: &str, args: &[&str], post: &str, script: &str, dir: &Path) -> Option<String> {
        which(prog)?;
        let f = dir.join(if prog == "pwsh" {
            "post.ps1".into()
        } else {
            format!("post.{prog}")
        }); // pwsh runs only .ps1
        std::fs::write(&f, post).unwrap();
        let out = Command::new(prog)
            .args(args)
            .arg(script)
            .env("F", &f)
            .env("TABSMITH_SESSION_ID", "abc")
            .env("TABSMITH_HOME", dir.join("data"))
            .env_remove("_TS_POST_LOADED")
            .env_remove("VSCODE_INJECTION") // run in a VS Code terminal, the post block would load VS Code's script
            .output()
            .unwrap();
        let s = String::from_utf8_lossy(&out.stdout).into_owned();
        assert!(
            out.status.success(),
            "{prog}: {s}{}",
            String::from_utf8_lossy(&out.stderr)
        );
        Some(s)
    }

    /// zsh keeps RPROMPT and RPS1 apart until both are set: a .zshrc RPROMPT survives every preexec restore and
    /// is wrapped (once) at every prompt.
    #[test]
    fn integrations_zsh_rprompt_wrapped_and_kept() {
        let temp_dir = TempDir::new("zshrp");
        let dir = temp_dir.path().to_path_buf();
        let script = r#"PS1='$ '; RPROMPT='[rp]'; source "$F"
            _ts_precmd; print -r -- "W1=$RPROMPT"; _ts_preexec x; print -r -- "R1=$RPROMPT"
            _ts_precmd; print -r -- "W2=$RPROMPT"; _ts_preexec x; print -r -- "R2=$RPROMPT""#;
        if let Some(s) = run_post(
            "zsh",
            &["-f", "-c"],
            &snapshot(Shell::Zsh, When::Post),
            script,
            &dir,
        ) {
            assert!(
                s.contains("PreExec\x07R1=[rp]\n") && s.contains("PreExec\x07R2=[rp]\n"),
                "{s:?}"
            );
            for w in ["W1=", "W2="] {
                let line = s
                    .split(w)
                    .nth(1)
                    .and_then(|r| r.lines().next())
                    .unwrap_or_default();
                assert!(
                    line.matches("StartPrompt").count() == 1 && line.contains("[rp]"),
                    "{w} {line:?}"
                );
            }
        }
        let _ = std::fs::remove_dir_all(dir);
    }

    /// bash ≥ 5.1 with bash-preexec: PROMPT_COMMAND is an array whose first element embeds `_ts_first;…;_ts_last`.
    /// _ts_first still runs once per prompt; bash-preexec's __bp_interactive_mode stays last (it arms preexec).
    #[test]
    fn integrations_bash_prompt_command_array_runs_first_once() {
        let temp_dir = TempDir::new("bashpcarr");
        let dir = temp_dir.path().to_path_buf();
        let script = r#"PS1='$ '; source "$F"; __bp_precmd_invoke_cmd() { :; }; __bp_interactive_mode() { :; }
            PROMPT_COMMAND=($'__bp_precmd_invoke_cmd\n_ts_first;:;_ts_last' __bp_interactive_mode)
            for _ in 1 2 3; do for c in "${PROMPT_COMMAND[@]}"; do eval "$c"; done; done
            printf '\n'; declare -p PROMPT_COMMAND"#;
        if let Some(s) = run_post(
            "bash",
            &["--norc", "--noprofile", "-c"],
            &snapshot(Shell::Bash, When::Post),
            script,
            &dir,
        ) {
            assert_eq!(s.matches("OSCUnlock=abc").count(), 3, "{s:?}");
            assert!(s.contains(r#"([0]="_ts_first" [1]=$'__bp_precmd_invoke_cmd\n:' [2]="_ts_last" [3]="__bp_interactive_mode")"#), "{s:?}");
        }
        let _ = std::fs::remove_dir_all(dir);
    }

    const GHOSTTY: &str = "/Applications/Ghostty.app/Contents/Resources/ghostty";
    const VSCODE_BASH: &str =
        "/Applications/Visual Studio Code.app/Contents/Resources/app/out/vs/workbench/contrib/terminal/common/scripts/shellIntegration-bash.sh";

    /// Interactive bash in a PTY with `rc` (HISTFILE=/dev/null, then `rc`, then the post block) typing `input`, with
    /// `env` added. Output, or None without bash.
    #[cfg(unix)]
    fn bash_session(dir: &Path, rc: &str, env: &[(&str, &Path)], input: &str) -> Option<String> {
        which("bash")?;
        let f = dir.join("post.bash");
        std::fs::write(
            &f,
            init_for(
                Shell::Bash,
                When::Post,
                &fake_bin(&dir.join("bin")).display().to_string(),
            ),
        )
        .unwrap();
        let rcfile = dir.join("bashrc");
        std::fs::write(
            &rcfile,
            format!("HISTFILE=/dev/null; PS1='$ '\n{rc}\nsource \"$F\"\n"),
        )
        .unwrap();
        let env = [
            env,
            &[
                ("HOME", dir),
                ("F", &f),
                ("TABSMITH_SESSION_ID", Path::new("abc")),
            ],
        ]
        .concat();
        let argv = [
            "bash",
            "--noprofile",
            "--rcfile",
            &rcfile.display().to_string(),
            "-i",
        ];
        Some(pty_run(&argv, &env, "NewCmd=abc", input).0)
    }

    /// Real bash-preexec copies (Ghostty's bundled 0.6, any `bash-preexec*.sh` in $TABSMITH_THIRD_PARTY, e.g. 0.5.0,
    /// 0.6.0 and master) sourced before the post block: preexec runs once per command, with that command; the
    /// installer does not linger in PROMPT_COMMAND (it would delete the DEBUG trap at every prompt).
    #[cfg(unix)]
    #[test]
    fn integrations_bash_preexec_hooks_run() {
        let mut copies: Vec<PathBuf> = std::env::var_os("TABSMITH_THIRD_PARTY")
            .and_then(|d| std::fs::read_dir(d).ok())
            .into_iter()
            .flatten()
            .flatten()
            .map(|e| e.path())
            .filter(|p| {
                p.file_name()
                    .is_some_and(|n| n.to_string_lossy().starts_with("bash-preexec"))
            })
            .collect();
        copies.push(Path::new(GHOSTTY).join("shell-integration/bash/bash-preexec.sh"));
        for (i, bp) in copies.iter().filter(|p| p.is_file()).enumerate() {
            let temp_dir = TempDir::new(&format!("bashpreexec{i}"));
            let dir = temp_dir.path().to_path_buf();
            let (log, out) = (dir.join("hooks.log"), dir.join("pc.out"));
            let q = |p: &Path| quote(Shell::Bash, &p.display().to_string());
            let rc = format!(
                "source {}; preexec() {{ echo \"PRE[$1]\" >> {}; }}",
                q(bp),
                q(&log)
            );
            let input = format!("echo one\recho two\r\recho three\r{{ declare -p PROMPT_COMMAND; trap -p DEBUG; }} > {}\rexit\r", q(&out));
            let Some(s) = bash_session(&dir, &rc, &[], &input) else {
                return;
            };
            let log = std::fs::read_to_string(&log).unwrap_or_default();
            let pre: Vec<&str> = log.lines().take(3).collect();
            assert_eq!(
                pre,
                ["PRE[echo one]", "PRE[echo two]", "PRE[echo three]"],
                "{bp:?}: {log}\n{s:?}"
            );
            let pc = std::fs::read_to_string(&out).unwrap_or_default();
            assert!(
                !pc.contains("__bp_install") && pc.matches("__bp_interactive_mode").count() == 1,
                "{bp:?}: {pc}"
            );
            let _ = std::fs::remove_dir_all(dir);
        }
    }

    /// Ghostty's real bash integration (it bundles bash-preexec 0.6), reloaded by the post block when Ghostty
    /// injected it: command start (OSC 133;C) is reported for every command.
    #[cfg(unix)]
    #[test]
    fn integrations_bash_ghostty_real_integration() {
        if !Path::new(GHOSTTY).is_dir() {
            return;
        }
        let temp_dir = TempDir::new("bashghostty");
        let dir = temp_dir.path().to_path_buf();
        let env: &[(&str, &Path)] = &[
            ("GHOSTTY_RESOURCES_DIR", Path::new(GHOSTTY)),
            ("TABSMITH_GHOSTTY_INJECTED", Path::new("1")),
        ];
        if let Some(s) = bash_session(&dir, "", env, "echo one\recho two\recho three\rexit\r") {
            assert!(s.matches("\x1b]133;C").count() >= 3, "{s:?}");
        }
        let _ = std::fs::remove_dir_all(dir);
    }

    /// VS Code's real bash integration: reloaded by the post block (auto-injected, with a PROMPT_COMMAND that sets
    /// PS1, or after bash-preexec), or sourced by the user's rc (manual install). Every command is reported as typed,
    /// after the prompt it was typed at (never `_ts_last`/`_ts_first`, also for an empty line), and each prompt is
    /// wrapped once by VS Code and once by us.
    #[cfg(unix)]
    #[test]
    fn integrations_bash_vscode_real_integration() {
        use std::os::unix::fs::PermissionsExt;
        if !Path::new(VSCODE_BASH).is_file() {
            return;
        }
        let temp_dir = TempDir::new("bashvscode");
        let dir = temp_dir.path().to_path_buf();
        let code = dir.join("codebin/code");
        std::fs::create_dir_all(code.parent().unwrap()).unwrap();
        std::fs::write(
            &code,
            format!("#!/bin/sh\necho {}\n", quote(Shell::Bash, VSCODE_BASH)),
        )
        .unwrap();
        std::fs::set_permissions(&code, std::fs::Permissions::from_mode(0o755)).unwrap();
        let path = format!(
            "{}:{}",
            code.parent().unwrap().display(),
            std::env::var("PATH").unwrap_or_default()
        );
        let user = r#"_p() { PS1="[$((n+=1))] \$ "; }; PROMPT_COMMAND=_p"#;
        let manual = format!("source {}", quote(Shell::Bash, VSCODE_BASH));
        let injected: &[(&str, &Path)] = &[
            ("VSCODE_INJECTION", Path::new("1")),
            ("PATH", Path::new(&path)),
        ];
        let mut cases = vec![(user.to_string(), injected), (manual, &[])];
        // With bash-preexec loaded first (atuin, ...), VS Code takes its hooks instead of a trap of its own.
        let bp = Path::new(GHOSTTY).join("shell-integration/bash/bash-preexec.sh");
        if bp.is_file() {
            cases.push((
                format!("source {}", quote(Shell::Bash, &bp.display().to_string())),
                injected,
            ));
        }
        for (rc, env) in cases {
            let Some(s) = bash_session(&dir, &rc, env, "echo one\r\recho two\rexit\r") else {
                return;
            };
            let codes: Vec<&str> = s
                .split("\x1b]633;")
                .skip(1)
                .map(|c| &c[..c.find('\x07').unwrap_or(0)])
                .collect();
            let cmds: Vec<&str> = codes
                .iter()
                .filter(|c| c.starts_with("E;"))
                .copied()
                .collect();
            assert!(cmds.iter().all(|c| !c.contains("_ts")), "{rc}: {codes:?}");
            assert!(
                cmds.contains(&"E;echo one;") && cmds.contains(&"E;echo two;"),
                "{rc}: {codes:?}"
            );
            let after_prompt = codes
                .windows(2)
                .all(|w| !w[1].starts_with("E;") || w[0] == "B");
            assert!(
                after_prompt,
                "{rc}: a command reported before its prompt: {codes:?}"
            );
            let prompts: Vec<&str> = s.split("\x1b]697;NewCmd=abc").collect();
            for p in &prompts[..prompts.len() - 1] {
                let ok =
                    p.matches("\x1b]633;A").count() == 1 && p.matches("StartPrompt").count() == 1;
                assert!(
                    ok,
                    "{rc}: each prompt wrapped once by VS Code and by us: {p:?}"
                );
            }
            if rc == user {
                assert!(
                    s.contains("\x1b]697;StartPrompt\x07[3] $ \x1b]697;EndPrompt"),
                    "{rc}: {s:?}"
                );
            }
        }
        let _ = std::fs::remove_dir_all(dir);
    }

    /// fish prompt wrappers: backslashes in the prompt stay literal, $argv (transient prompt's --final-rendering)
    /// reaches the user's function, an erased right prompt stays erased, and $last_pid survives the prompt.
    #[cfg(unix)]
    #[test]
    fn integrations_fish_prompt_wrappers() {
        let temp_dir = TempDir::new("fishprompt");
        let dir = temp_dir.path().to_path_buf();
        let bin = fake_bin(&dir.join("bin")).display().to_string();
        let script = r#"function fish_prompt; echo -n 'a\cb \o/ args='"$argv> "; end
            function fish_right_prompt; echo -n RP; end
            source $F; _ts_precmd; echo; echo P=(fish_prompt --final-rendering)
            _ts_preexec 'functions -e fish_right_prompt'; functions -e fish_right_prompt
            _ts_precmd; _ts_preexec 'echo a'; _ts_precmd
            functions -q fish_right_prompt; and echo RESURRECTED
            command sleep 2 &; set -l p $last_pid; _ts_precmd; echo; test "$last_pid" = "$p"; and echo LASTPID-KEPT
            kill $p"#;
        if let Some(s) = run_post(
            "fish",
            &["--no-config", "-c"],
            &init_for(Shell::Fish, When::Post, &bin),
            script,
            &dir,
        ) {
            assert!(s.contains("\nP=\x1b]697;StartPrompt\x07a\\cb \\o/ args=--final-rendering> \x1b]697;EndPrompt"), "{s:?}");
            assert!(
                !s.contains("RESURRECTED") && s.contains("\nLASTPID-KEPT"),
                "{s:?}"
            );
        }
        let _ = std::fs::remove_dir_all(dir);
    }

    /// Lines the shell keeps out of its own history (leading space under ignorespace / HIST_IGNORE_SPACE / fish,
    /// fish private mode) send NoHistory between OSCLock and PreExec; other lines do not. Real interactive shells.
    #[cfg(unix)]
    #[test]
    fn integrations_no_history_marker() {
        let temp_dir = TempDir::new("nohist");
        let dir = temp_dir.path().to_path_buf();
        let bin = fake_bin(&dir.join("bin")).display().to_string();
        let home = dir.join("home");
        std::fs::create_dir_all(&home).unwrap();
        let rc = "HISTFILE=/dev/null; PS1='$ '; source \"$F\"\n";
        let bash_rc = dir.join("bashrc");
        std::fs::write(&bash_rc, format!("HISTCONTROL=ignorespace; {rc}")).unwrap();
        std::fs::write(home.join(".zshrc"), format!("setopt histignorespace; {rc}")).unwrap();
        let b = bash_rc.display().to_string();
        // Not `fish --no-config`: fish 4 runs that in private mode.
        let cases: [(Shell, &[&str], &[bool]); 4] = [
            (
                Shell::Bash,
                &["bash", "--noprofile", "--rcfile", &b, "-i"],
                &[false, true, false],
            ),
            (Shell::Zsh, &["zsh", "-d", "-i"], &[false, true, false]),
            (
                Shell::Fish,
                &["fish", "-i", "-C", "source $F"],
                &[false, true, false],
            ),
            (
                Shell::Fish,
                &["fish", "--private", "-i", "-C", "source $F"],
                &[true, true, true],
            ),
        ];
        for (shell, argv, want) in cases {
            if which(argv[0]).is_none() {
                continue;
            }
            let f = dir.join(format!("post.{}", shell.name()));
            std::fs::write(&f, init_for(shell, When::Post, &bin)).unwrap();
            let env: &[(&str, &Path)] = &[
                ("HOME", &home),
                ("ZDOTDIR", &home),
                ("XDG_CONFIG_HOME", &home.join(".config")),
                ("XDG_DATA_HOME", &home.join(".local/share")),
                ("F", &f),
                ("TABSMITH_SESSION_ID", Path::new("abc")),
                ("TABSMITH_HOME", &dir.join("data")),
            ];
            let (out, _) = pty_run(
                argv,
                env,
                "NewCmd=abc",
                "echo a\r echo secret\recho b\rexit\r",
            );
            let got: Vec<bool> = out
                .split("\x1b]697;PreExec\x07")
                .take(3)
                .map(|c| c.contains("NoHistory"))
                .collect();
            assert_eq!(got, want, "{argv:?}: {out:?}");
            let locked = out.split("\x1b]697;OSCLock=abc\x07").skip(1).all(|c| {
                !c.contains("NoHistory") || c.starts_with("\x1b]697;NoHistory\x07\x1b]697;PreExec")
            });
            assert!(
                locked,
                "{argv:?}: NoHistory sits between OSCLock and PreExec: {out:?}"
            );
        }
        let _ = std::fs::remove_dir_all(dir);
    }

    /// pwsh (when installed) with the profile integration, in a PTY: a plain interactive start runs the wrapper and
    /// exits with its code; a missing wrapper leaves the shell usable; scripts and commands are never hijacked.
    #[cfg(unix)]
    #[test]
    fn integrations_pwsh_pre_launch_rules() {
        if which("pwsh").is_none() {
            return;
        }
        let temp_dir = TempDir::new("pwshpre");
        let dir = temp_dir.path().to_path_buf();
        let home = dir.join("home");
        let bin = fake_bin(&dir.join("bin")).display().to_string();
        let c = Ctx {
            bin: bin.clone(),
            ..ctx(&dir)
        };
        let pwsh = &integrations(&c, Shell::Pwsh).unwrap()[0];
        pwsh.install(&c).unwrap();
        std::fs::write(format!("{bin}.post"), "# post\n").unwrap();
        let script = dir.join("hello.ps1");
        std::fs::write(&script, "'SCRIPT RAN'; exit 3\n").unwrap();
        let env: &[(&str, &Path)] = &[("HOME", &home), ("XDG_CONFIG_HOME", &home.join(".config"))];
        let launched = || std::fs::remove_file(format!("{bin}.launched")).is_ok();

        let set_wrapper = |w: &str| {
            std::fs::write(format!("{bin}.pre"), init_for(Shell::Pwsh, When::Pre, w)).unwrap()
        };
        set_wrapper(&bin);
        let d = dir.display().to_string();
        for argv in [
            &["pwsh", "-NoLogo"][..],
            &["pwsh", "-nol", "-wd", &d, "-noe"],
        ] {
            let (out, code) = pty_run(argv, env, "", "");
            assert!(
                launched() && code == 7,
                "{argv:?}: interactive start → wrapper, its exit code ({code}): {out:?}"
            );
        }
        // The inner shell gets the outer switches, after a -NoLogo (no second banner).
        let args = std::fs::read_to_string(format!("{bin}.args")).unwrap();
        assert!(
            args.ends_with(&format!("--\n-NoLogo\n-nol\n-wd\n{d}\n-noe\n")),
            "{args:?}"
        );

        let enc = "JwBFAEMAIABSAEEATgAnADsAIABlAHgAaQB0ACAANAA="; // 'EC RAN'; exit 4 (UTF-16LE base64)
        let s = script.display().to_string();
        let cases: [(&[&str], &str, u32); 3] = [
            (&["-NoLogo", &s], "SCRIPT RAN", 3),
            (&["-NoLogo", "-ec", enc], "EC RAN", 4),
            (&["-NoLogo", "-Com", "'COM RAN'; exit 5"], "COM RAN", 5),
        ];
        for (args, want, rc) in cases {
            let argv = [&["pwsh"][..], args].concat();
            let (out, code) = pty_run(&argv, env, "", "");
            assert!(
                !launched() && out.contains(want) && code == rc,
                "{args:?} ({code}): {out:?}"
            );
        }

        set_wrapper("/nonexistent/tabsmith");
        let (out, code) = pty_run(&["pwsh", "-NoLogo"], env, "PS ", "exit 6\r");
        assert_eq!(
            code, 6,
            "a wrapper that cannot start must not end the shell: {out:?}"
        );
        let _ = std::fs::remove_dir_all(dir);
    }

    /// powerlevel10k (when installed) rebuilds PROMPT after our precmd at every prompt (its hook re-asserts itself
    /// last) and again on async re-renders: both get the markers, and _ts_u1 keeps p10k's own prompt.
    #[cfg(unix)]
    #[test]
    fn integrations_zsh_powerlevel10k_prompt_wrapped() {
        let theme = [
            "/opt/homebrew/share/powerlevel10k",
            "/usr/local/share/powerlevel10k",
            "/usr/share/zsh-theme-powerlevel10k",
        ]
        .map(|d| Path::new(d).join("powerlevel10k.zsh-theme"))
        .into_iter()
        .find(|p| p.is_file());
        let (Some(theme), Some(_)) = (theme, which("zsh")) else {
            return;
        };
        let temp_dir = TempDir::new("p10k");
        let dir = temp_dir.path().to_path_buf();
        let home = dir.join("home");
        std::fs::create_dir_all(&home).unwrap();
        let f = dir.join("post.zsh");
        std::fs::write(
            &f,
            init_for(
                Shell::Zsh,
                When::Post,
                &fake_bin(&dir.join("bin")).display().to_string(),
            ),
        )
        .unwrap();
        // The alarm stands in for an async re-render (gitstatus, the worker): p10k rebuilds PROMPT, redraws it.
        let rc = format!(
            "HISTFILE=/dev/null POWERLEVEL9K_DISABLE_CONFIGURATION_WIZARD=true POWERLEVEL9K_DISABLE_GITSTATUS=true
            POWERLEVEL9K_INSTANT_PROMPT=off POWERLEVEL9K_LEFT_PROMPT_ELEMENTS=(dir) POWERLEVEL9K_RIGHT_PROMPT_ELEMENTS=(status)
            source {}; source \"$F\"
            TMOUT=1; TRAPALRM() {{ _p9k_set_prompt; _p9k_reset_prompt; [[ $_ts_u1 == *697* || $PS1 == *StartPrompt*StartPrompt* ]] && print -n NESTED; print -n '<ALARM>'; exit }}\n",
            quote(Shell::Zsh, &theme.display().to_string())
        );
        std::fs::write(home.join(".zshrc"), rc).unwrap();
        let env: &[(&str, &Path)] = &[
            ("HOME", &home),
            ("ZDOTDIR", &home),
            ("XDG_CACHE_HOME", &home.join(".cache")),
            ("F", &f),
            ("TABSMITH_SESSION_ID", Path::new("abc")),
            ("TABSMITH_HOME", &dir.join("data")),
        ];
        let (out, _) = pty_run(&["zsh", "-d", "-i"], env, "NewCmd=abc", "true\r");
        let second = out
            .split("\x1b]697;PreExec\x07")
            .nth(1)
            .and_then(|r| r.split("<ALARM>").next())
            .unwrap_or_default();
        assert!(out.contains("<ALARM>"), "{out:?}");
        assert!(
            second.matches("NewCmd=abc").count() >= 2,
            "the next prompt and its re-render are wrapped: {second:?}"
        );
        assert!(!second.contains("NESTED"), "{second:?}");
        let _ = std::fs::remove_dir_all(dir);
    }

    /// A theme whose precmd hook sets PS1 after ours (it moves itself last too, or was added after the post block,
    /// e.g. by a .zshrc read after the .zprofile one): line-init wraps the prompt before the line is edited.
    #[cfg(unix)]
    #[test]
    fn integrations_zsh_theme_after_our_precmd_wrapped() {
        if which("zsh").is_none() {
            return;
        }
        let temp_dir = TempDir::new("zshtheme");
        let dir = temp_dir.path().to_path_buf();
        let home = dir.join("home");
        std::fs::create_dir_all(&home).unwrap();
        let f = dir.join("post.zsh");
        std::fs::write(
            &f,
            init_for(
                Shell::Zsh,
                When::Post,
                &fake_bin(&dir.join("bin")).display().to_string(),
            ),
        )
        .unwrap();
        let rc = "HISTFILE=/dev/null; source \"$F\"
            _theme() { PS1='[mine] %# '; precmd_functions=(${precmd_functions:#_theme} _theme) }; precmd_functions+=(_theme)\n";
        std::fs::write(home.join(".zshrc"), rc).unwrap();
        let env: &[(&str, &Path)] = &[
            ("HOME", &home),
            ("ZDOTDIR", &home),
            ("F", &f),
            ("TABSMITH_SESSION_ID", Path::new("abc")),
        ];
        let (out, _) = pty_run(&["zsh", "-d", "-i"], env, "", "true\rexit\r");
        let prompts: Vec<&str> = out.split("\x1b]697;PreExec\x07").take(2).collect();
        assert!(
            prompts.len() == 2
                && prompts
                    .iter()
                    .all(|p| p.contains("[mine] % \x1b]697;EndPrompt")),
            "{out:?}"
        );
        let _ = std::fs::remove_dir_all(dir);
    }

    /// Under Ghostty, kitty and VS Code the post block loads the shell integration the terminal injected into the
    /// outer shell (VS Code's with VSCODE_INJECTION unset, so it does not source the rc file again); our hooks stay
    /// outermost.
    #[cfg(unix)]
    #[test]
    fn integrations_terminal_injected_integration_reloaded() {
        use std::os::unix::fs::PermissionsExt;
        let temp_dir = TempDir::new("injected");
        let dir = temp_dir.path().to_path_buf();
        let d = dir.display().to_string();
        let files = [
            ("ghostty/shell-integration/zsh/ghostty-integration", "print -n GHOSTTY-ZSH; precmd_functions+=(_g)\n"),
            ("ghostty/shell-integration/bash/ghostty.bash", "printf GHOSTTY-BASH; PROMPT_COMMAND=\"_g;$PROMPT_COMMAND\"\n"),
            ("kitty/shell-integration/zsh/kitty-integration", "print -n KITTY-ZSH\n"),
            ("kitty/shell-integration/bash/kitty.bash", "printf KITTY-BASH\n"),
            ("vscode/rc.zsh", "print -n \"VSCODE-ZSH[$VSCODE_INJECTION]\"\n"),
            ("vscode/rc.bash", "printf 'VSCODE-BASH[%s]' \"$VSCODE_INJECTION\"\n"),
            ("bin/code", &format!("#!/bin/sh\n[ \"$1\" = --locate-shell-integration-path ] && echo {d}/vscode/rc.$2\n")),
        ];
        for (p, text) in files {
            std::fs::create_dir_all(dir.join(p).parent().unwrap()).unwrap();
            std::fs::write(dir.join(p), text).unwrap();
        }
        std::fs::set_permissions(dir.join("bin/code"), std::fs::Permissions::from_mode(0o755))
            .unwrap();
        let env = format!(
            "export GHOSTTY_RESOURCES_DIR={d}/ghostty KITTY_SHELL_INTEGRATION=enabled KITTY_INSTALLATION_DIR={d}/kitty VSCODE_INJECTION=1 PATH={d}/bin:$PATH"
        );
        // Ghostty only when it injected (the pre block saw it): never under `shell-integration = none`, nor in a
        // shell started from this one.
        for ghostty in ["export TABSMITH_GHOSTTY_INJECTED=1", ":"] {
            let zsh = format!("{env}; {ghostty}; PS1='$ '; source \"$F\"; print -r -- \" last=${{precmd_functions[-1]}} inj=$VSCODE_INJECTION g=${{TABSMITH_GHOSTTY_INJECTED-}}\"");
            if let Some(s) = run_post(
                "zsh",
                &["-f", "-c"],
                &snapshot(Shell::Zsh, When::Post),
                &zsh,
                &dir,
            ) {
                assert_eq!(s.contains("GHOSTTY-ZSH"), ghostty != ":", "{s:?}");
                assert!(
                    s.contains("KITTY-ZSH") && s.contains("VSCODE-ZSH[]"),
                    "{s:?}"
                );
                assert!(s.ends_with(" last=_ts_precmd inj=1 g=\n"), "{s:?}");
            }
            let bash = format!("{env}; {ghostty}; PS1='$ '; source \"$F\"; (IFS=';'; echo \" PC=${{PROMPT_COMMAND[*]}} inj=$VSCODE_INJECTION g=${{TABSMITH_GHOSTTY_INJECTED-}}\")");
            if let Some(s) = run_post(
                "bash",
                &["--norc", "--noprofile", "-c"],
                &snapshot(Shell::Bash, When::Post),
                &bash,
                &dir,
            ) {
                assert_eq!(s.contains("GHOSTTY-BASH"), ghostty != ":", "{s:?}");
                assert!(
                    s.contains("KITTY-BASH") && s.contains("VSCODE-BASH[]"),
                    "{s:?}"
                );
                let pc = if ghostty == ":" {
                    "_ts_first;_ts_last"
                } else {
                    "_ts_first;_g;_ts_last"
                };
                assert!(s.ends_with(&format!(" PC={pc} inj=1 g=\n")), "{s:?}");
            }
        }
        let _ = std::fs::remove_dir_all(dir);
    }

    /// The pre block tells the wrapper whether Ghostty injected its integration into this shell (zsh: Ghostty's
    /// .zshenv set `_ghostty_state`; bash: ghostty.bash is sourcing the rc files), not merely ran in Ghostty.
    #[cfg(unix)]
    #[test]
    fn integrations_pre_detects_ghostty_injection() {
        use std::os::unix::fs::PermissionsExt;
        let temp_dir = TempDir::new("ghosttypre");
        let dir = temp_dir.path().to_path_buf();
        let fake = dir.join("fake");
        let got = dir.join("got");
        std::fs::write(
            &fake,
            format!(
                "#!/bin/sh\necho \"[$TABSMITH_GHOSTTY_INJECTED]\" > {}\n",
                quote(Shell::Bash, &got.display().to_string())
            ),
        )
        .unwrap();
        std::fs::set_permissions(&fake, std::fs::Permissions::from_mode(0o755)).unwrap();
        let pre = dir.join("pre.sh");
        std::fs::write(
            &pre,
            init_for(Shell::Bash, When::Pre, &fake.display().to_string()),
        )
        .unwrap();
        let cases = [
            ("bash", "__ghostty_bash_flags=''", "[1]"),
            ("bash", ":", "[]"),
            ("zsh", "typeset -gi _ghostty_state", "[1]"),
            ("zsh", ":", "[]"),
        ];
        for (sh, setup, want) in cases {
            if which(sh).is_none() {
                continue;
            }
            let rc = dir.join(format!("rc.{sh}"));
            std::fs::write(
                &rc,
                format!(
                    "{setup}; source {}\n",
                    quote(Shell::Bash, &pre.display().to_string())
                ),
            )
            .unwrap();
            if sh == "zsh" {
                std::fs::write(dir.join(".zshrc"), std::fs::read(&rc).unwrap()).unwrap();
            }
            let _ = std::fs::remove_file(&got);
            let rcs = rc.display().to_string();
            let argv: Vec<&str> = if sh == "bash" {
                vec!["bash", "--noprofile", "--rcfile", &rcs, "-i"]
            } else {
                vec!["zsh", "-d", "-i"]
            };
            let (out, _) = pty_run(&argv, &[("HOME", &dir), ("ZDOTDIR", &dir)], "", "");
            assert_eq!(
                std::fs::read_to_string(&got).unwrap_or_default().trim(),
                want,
                "{sh} {setup}: {out:?}"
            );
        }
        let _ = std::fs::remove_dir_all(dir);
    }

    /// zsh: a theme that sets PS1 mid-edit from a `zle -F` handler and redraws it (oh-my-posh streaming with
    /// `zle reset-prompt`, pure with `zle .reset-prompt`) gets the new prompt wrapped before the line is used.
    #[cfg(unix)]
    #[test]
    fn integrations_zsh_async_prompt_rewrapped() {
        if which("zsh").is_none() {
            return;
        }
        let temp_dir = TempDir::new("zshasync");
        let dir = temp_dir.path().to_path_buf();
        let f = dir.join("post.zsh");
        std::fs::write(
            &f,
            init_for(
                Shell::Zsh,
                When::Post,
                &fake_bin(&dir.join("bin")).display().to_string(),
            ),
        )
        .unwrap();
        for reset in ["reset-prompt", ".reset-prompt"] {
            let rc = format!(
                "HISTFILE=/dev/null; PS1='$ '; source \"$F\"
                _st() {{ precmd_functions=(${{precmd_functions:#_st}}); exec {{_fd}}< <(sleep 0.3; print x); zle -F $_fd _h }}
                _h() {{ read -u $1; zle -F $1; exec {{_fd}}<&-; PS1='SLOW> '; zle {reset} }}
                precmd_functions+=(_st)\n"
            );
            std::fs::write(dir.join(".zshrc"), rc).unwrap();
            let env: &[(&str, &Path)] = &[
                ("HOME", &dir),
                ("ZDOTDIR", &dir),
                ("F", &f),
                ("TABSMITH_SESSION_ID", Path::new("abc")),
            ];
            let (out, _) = pty_run(&["zsh", "-d", "-i"], env, "SLOW> ", "echo hi\rexit\r");
            let line = out.split("\x1b]697;PreExec\x07").next().unwrap_or_default();
            assert!(
                line.contains("\x1b]697;StartPrompt\x07SLOW> \x1b]697;EndPrompt"),
                "{reset}: {out:?}"
            );
        }
        let _ = std::fs::remove_dir_all(dir);
    }

    /// bash with HISTCONTROL=ignoreboth:erasedups: a re-run of an older command reaches bash's history (the older
    /// copy goes) and ours; a space-prefixed line and an immediate repeat (ignoredups) do not.
    #[cfg(unix)]
    #[test]
    fn integrations_bash_erasedups_keeps_reruns() {
        if which("bash").is_none() {
            return;
        }
        let temp_dir = TempDir::new("erasedups");
        let dir = temp_dir.path().to_path_buf();
        let bin = fake_bin(&dir.join("bin")).display().to_string();
        let f = dir.join("post.bash");
        std::fs::write(&f, init_for(Shell::Bash, When::Post, &bin)).unwrap();
        let rc = dir.join("bashrc");
        std::fs::write(
            &rc,
            "HISTFILE=/dev/null; HISTCONTROL=ignoreboth:erasedups; PS1='$ '; source \"$F\"\n",
        )
        .unwrap();
        let env: &[(&str, &Path)] = &[
            ("HOME", &dir),
            ("F", &f),
            ("TABSMITH_SESSION_ID", Path::new("abc")),
        ];
        let argv = [
            "bash",
            "--noprofile",
            "--rcfile",
            &rc.display().to_string(),
            "-i",
        ];
        let (out, _) = pty_run(
            &argv,
            env,
            "NewCmd=abc",
            "echo a\recho b\recho a\r echo secret\recho b\recho b\rexit\r",
        );
        let got: Vec<bool> = out
            .split("\x1b]697;PreExec\x07")
            .take(6)
            .map(|c| c.contains("NoHistory"))
            .collect();
        assert_eq!(got, [false, false, false, true, false, true], "{out:?}");
        let _ = std::fs::remove_dir_all(dir);
    }

    /// bash 5.1+ (when installed): a string PROMPT_COMMAND becomes an array, so the user's prompt command still sees
    /// the command's PIPESTATUS (starship's pipestatus, for one).
    #[cfg(unix)]
    #[test]
    fn integrations_bash_prompt_command_keeps_pipestatus() {
        let new = Command::new("bash")
            .args([
                "-c",
                "(( BASH_VERSINFO[0] * 100 + BASH_VERSINFO[1] >= 501 ))",
            ])
            .status();
        if !new.is_ok_and(|s| s.success()) {
            return;
        }
        let temp_dir = TempDir::new("pipestatus");
        let dir = temp_dir.path().to_path_buf();
        let bin = fake_bin(&dir.join("bin")).display().to_string();
        let f = dir.join("post.bash");
        std::fs::write(&f, init_for(Shell::Bash, When::Post, &bin)).unwrap();
        let rc = dir.join("bashrc");
        let user = r#"_show_ps() { _ps="${PIPESTATUS[*]}"; }; PROMPT_COMMAND=_show_ps; PS1='[ps=$_ps] $ '"#;
        std::fs::write(&rc, format!("HISTFILE=/dev/null; {user}; source \"$F\"\n")).unwrap();
        let env: &[(&str, &Path)] = &[
            ("HOME", &dir),
            ("F", &f),
            ("TABSMITH_SESSION_ID", Path::new("abc")),
        ];
        let argv = [
            "bash",
            "--noprofile",
            "--rcfile",
            &rc.display().to_string(),
            "-i",
        ];
        let (out, _) = pty_run(&argv, env, "NewCmd=abc", "false | true | (exit 3)\rexit\r");
        assert!(out.contains("[ps=1 0 3] $ "), "{out:?}");
        let _ = std::fs::remove_dir_all(dir);
    }

    /// Ctrl-D (end of input) at a [Y/n] prompt is no, never yes.
    #[test]
    fn integrations_confirm_eof_is_no() {
        assert!(!answer(&b""[..]).unwrap());
        assert!(answer(&b"\n"[..]).unwrap() && answer(&b"Yes\n"[..]).unwrap());
        assert!(!answer(&b"n\n"[..]).unwrap());
    }

    /// Dotfiles are replaced atomically: whoever holds the old file sees it whole, a symlinked dotfile stays a
    /// symlink, permissions stay, no temp file is left, and a read-only file still reports PermissionDenied.
    /// The same scenario against the write itself is shared atomic_file.rs `integrations_dotfile_write_is_atomic`.
    #[cfg(unix)]
    #[test]
    fn integrations_dotfile_write_is_atomic_install() {
        use std::os::unix::fs::PermissionsExt;
        let temp_dir = TempDir::new("atomic");
        let dir = temp_dir.path().to_path_buf();
        let c = ctx(&dir);
        let real = dir.join("dotfiles").join("zshrc");
        std::fs::create_dir_all(real.parent().unwrap()).unwrap();
        std::fs::create_dir_all(&c.home).unwrap();
        std::fs::write(&real, "export A=1\n").unwrap();
        std::fs::set_permissions(&real, std::fs::Permissions::from_mode(0o600)).unwrap();
        let zshrc = c.home.join(".zshrc");
        std::os::unix::fs::symlink(&real, &zshrc).unwrap();
        let old = dir.join("old-zshrc");
        std::fs::hard_link(&real, &old).unwrap(); // the old inode, as a concurrent reader sees it
        let rc = Integration {
            shell: Shell::Zsh,
            path: zshrc.clone(),
            file_when: None,
        };
        rc.install(&c).unwrap();
        assert_eq!(
            std::fs::read_to_string(&old).unwrap(),
            "export A=1\n",
            "rewritten in place"
        );
        assert!(std::fs::symlink_metadata(&zshrc)
            .unwrap()
            .file_type()
            .is_symlink());
        assert!(std::fs::read_to_string(&real).unwrap().contains(PRE_MARKER));
        assert_eq!(
            std::fs::metadata(&real).unwrap().permissions().mode() & 0o777,
            0o600
        );
        assert_eq!(
            std::fs::read_dir(real.parent().unwrap()).unwrap().count(),
            1,
            "temp file left behind"
        );
        if !is_root() {
            std::fs::set_permissions(&real, std::fs::Permissions::from_mode(0o444)).unwrap();
            assert!(matches!(
                rc.uninstall(&c),
                Err(IntegrationError::PermissionDenied(_))
            ));
            assert!(std::fs::read_to_string(&real).unwrap().contains(PRE_MARKER));
        }
        // A dangling relative symlink (dotfiles checkout not populated yet): the link stays, its target is created.
        let bashrc = c.home.join(".bashrc");
        std::os::unix::fs::symlink("../dotfiles/bashrc", &bashrc).unwrap();
        let rc = Integration {
            shell: Shell::Bash,
            path: bashrc.clone(),
            file_when: None,
        };
        rc.install(&c).unwrap();
        assert!(std::fs::symlink_metadata(&bashrc)
            .unwrap()
            .file_type()
            .is_symlink());
        assert!(std::fs::read_to_string(dir.join("dotfiles/bashrc"))
            .unwrap()
            .contains(PRE_MARKER));
        let _ = std::fs::remove_dir_all(dir);
    }

    /// `integrations uninstall` removes integrations whose scripts name another binary (it moved): is_installed
    /// fails for all of them. With nothing left it reports nothing done.
    #[test]
    fn integrations_uninstall_after_binary_moved() {
        let temp_dir = TempDir::new("moved");
        let dir = temp_dir.path().to_path_buf();
        let c = ctx(&dir);
        let shells = [Shell::Bash, Shell::Fish];
        install_dotfiles(&c, &shells).unwrap();
        let moved = Ctx {
            bin: "/elsewhere/tabsmith".into(),
            ..c.clone()
        };
        let all: Vec<Integration> = shells
            .iter()
            .flat_map(|s| integrations(&moved, *s).unwrap())
            .collect();
        assert!(all.iter().all(|i| i.is_installed(&moved).is_err()));
        assert!(uninstall_dotfiles(&moved, &shells).unwrap());
        assert!(!std::fs::read_to_string(c.home.join(".bashrc"))
            .unwrap()
            .contains("tabsmith"));
        assert!(all.iter().all(|i| !i.has_traces(&moved)), "{all:?}");
        assert!(!uninstall_dotfiles(&moved, &shells).unwrap());
        let _ = std::fs::remove_dir_all(dir);
    }

    /// A dotfile uninstall cannot clean (read-only) makes `uninstall` fail with 1, not claim success.
    #[cfg(unix)]
    #[test]
    fn integrations_uninstall_reports_failure() {
        use std::os::unix::fs::PermissionsExt;
        if is_root() {
            return;
        }
        let temp_dir = TempDir::new("uninstfail");
        let dir = temp_dir.path().to_path_buf();
        let c = ctx(&dir);
        install_dotfiles(&c, &[Shell::Zsh]).unwrap();
        let zshrc = c.home.join(".zshrc");
        std::fs::set_permissions(&zshrc, std::fs::Permissions::from_mode(0o444)).unwrap();
        assert_eq!(uninstall_all(&c, &[Shell::Zsh], false), 1);
        assert!(std::fs::read_to_string(&zshrc)
            .unwrap()
            .contains(PRE_MARKER));
        std::fs::set_permissions(&zshrc, std::fs::Permissions::from_mode(0o644)).unwrap();
        assert_eq!(uninstall_all(&c, &[Shell::Zsh], true), 0);
        assert!(!c.data.exists());
        let _ = std::fs::remove_dir_all(dir);
    }

    /// The manual-install lines setup prints are the ones is_installed and uninstall recognise.
    #[test]
    fn integrations_manual_lines_are_recognised() {
        let temp_dir = TempDir::new("manual");
        let dir = temp_dir.path().to_path_buf();
        let c = ctx(&dir);
        std::fs::create_dir_all(&c.home).unwrap();
        let text = manual_text(&c, &[Shell::Bash, Shell::Zsh]);
        let lines: Vec<&str> = text
            .lines()
            .filter_map(|l| l.strip_prefix("zsh:"))
            .map(str::trim)
            .collect();
        assert_eq!(lines.len(), 2, "{text}");
        let zshrc = c.home.join(".zshrc");
        std::fs::write(&zshrc, format!("{}\nexport A=1\n{}\n", lines[0], lines[1])).unwrap();
        let rc = Integration {
            shell: Shell::Zsh,
            path: zshrc.clone(),
            file_when: None,
        };
        rc.write_scripts(&c).unwrap(); // what setup's manual path writes
        rc.is_installed(&c).unwrap();
        rc.uninstall(&c).unwrap();
        assert_eq!(std::fs::read_to_string(&zshrc).unwrap(), "export A=1\n");
        assert!(
            !text.contains("pwsh")
                && manual_text(&c, &[Shell::Pwsh]).contains("pwsh ($PROFILE): if (Test-Path")
        );
        let _ = std::fs::remove_dir_all(dir);
    }

    /// Run files untouched for a day go (a quick exit can leave one behind); fresh files and directories stay.
    #[test]
    fn integrations_prune_run_files() {
        let temp_dir = TempDir::new("prune");
        let dir = temp_dir.path().to_path_buf();
        let (old, new, sub) = (
            dir.join("old.json"),
            dir.join("new.json"),
            dir.join("debug-shell-1"),
        );
        std::fs::write(&old, "{}").unwrap();
        std::fs::write(&new, "{}").unwrap();
        std::fs::create_dir(&sub).unwrap();
        let day_ago = std::time::SystemTime::now() - Duration::from_secs(2 * 24 * 3600);
        std::fs::File::options()
            .write(true)
            .open(&old)
            .unwrap()
            .set_modified(day_ago)
            .unwrap();
        prune_run_files(&dir);
        assert!(!old.exists() && new.exists() && sub.exists());
        let _ = std::fs::remove_dir_all(dir);
    }
}
