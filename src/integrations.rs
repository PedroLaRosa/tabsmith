//! Shell integration: `init` scripts, launch decision, dotfile install/uninstall/status, setup/uninstall flows,
//! and the internal `_ should-launch|get-shell|pre-cmd` commands (SPEC §2.6, §2.7, §2.9, §2.11, §5).
//! OWNER: integrations agent. Shell scripts live in `shell/` and are embedded with `include_str!`.
use crate::cli::OutputFormat;
use crate::types::{now_ms, now_secs, SessionCtx, Shell};
use anyhow::{bail, Context, Result};
use clap::{Args, Subcommand, ValueEnum};
use std::collections::HashMap;
use std::io::{IsTerminal, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

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
    Dotfiles {
        shell: Option<ShellArg>,
    },
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
        format: crate::cli::OutputFormat,
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

/// `s` wrapped in SGR `code` when stdout is a terminal.
pub fn paint(code: &str, s: &str) -> String {
    if std::io::stdout().is_terminal() {
        format!("\x1b[{code}m{s}\x1b[0m")
    } else {
        s.to_string()
    }
}

/// `s` wrapped in SGR `code` when stderr is a terminal.
pub fn epaint(code: &str, s: &str) -> String {
    if std::io::stderr().is_terminal() {
        format!("\x1b[{code}m{s}\x1b[0m")
    } else {
        s.to_string()
    }
}

/// Unix euid 0.
pub fn is_root() -> bool {
    #[cfg(unix)]
    {
        // SAFETY: geteuid has no preconditions.
        unsafe { libc::geteuid() == 0 }
    }
    #[cfg(not(unix))]
    {
        false
    }
}

/// `GetConsoleMode` succeeds on `handle`.
#[cfg(windows)]
pub fn is_console(handle: std::os::windows::io::RawHandle) -> bool {
    let mut mode = 0;
    // SAFETY: plain query on a handle we own for the process lifetime; `mode` is a valid out pointer.
    unsafe { windows_sys::Win32::System::Console::GetConsoleMode(handle as _, &mut mode) != 0 }
}

/// First `name` executable on PATH (Windows: tries PATHEXT extensions).
pub fn which(name: &str) -> Option<PathBuf> {
    let exts: Vec<String> = if cfg!(windows) {
        std::env::var("PATHEXT")
            .unwrap_or_else(|_| ".EXE;.CMD;.BAT;.COM".into())
            .split(';')
            .map(String::from)
            .collect()
    } else {
        vec![String::new()]
    };
    std::env::split_paths(&std::env::var_os("PATH")?)
        .find_map(|dir| exts.iter().map(|e| dir.join(format!("{name}{e}"))).find(|p| p.is_file()))
}

/// Run `cmd` (stdin null, stderr ignored) and return its stdout; killed after `timeout`.
pub fn output_with_timeout(cmd: &mut Command, timeout: Duration) -> Result<String> {
    let mut child = cmd.stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::null()).spawn()?;
    let start = Instant::now();
    while child.try_wait()?.is_none() {
        if start.elapsed() > timeout {
            let _ = child.kill();
            let _ = child.wait();
            bail!("timed out after {}s", timeout.as_secs());
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    let mut out = String::new();
    if let Some(mut s) = child.stdout.take() {
        s.read_to_string(&mut out)?;
    }
    Ok(out)
}

/// Absolute path of the running tabsmith binary.
pub fn exe_path() -> Result<String> {
    Ok(std::env::current_exe().context("could not locate the tabsmith executable")?.to_string_lossy().into_owned())
}

/// Executable path of the parent process (SPEC §2.11 `_ get-shell`).
pub fn parent_exe() -> Option<String> {
    #[cfg(target_os = "linux")]
    {
        // SAFETY: getppid has no preconditions.
        let ppid = unsafe { libc::getppid() };
        std::fs::read_link(format!("/proc/{ppid}/exe")).ok().map(|p| p.to_string_lossy().into_owned())
    }
    #[cfg(target_os = "macos")]
    {
        // SAFETY: getppid has no preconditions; proc_pidpath writes at most `buf.len()` bytes into `buf`.
        let ppid = unsafe { libc::getppid() };
        let mut buf = vec![0u8; libc::PROC_PIDPATHINFO_MAXSIZE as usize];
        let n = unsafe { libc::proc_pidpath(ppid, buf.as_mut_ptr().cast(), buf.len() as u32) };
        (n > 0).then(|| String::from_utf8_lossy(&buf[..n as usize]).into_owned())
    }
    // ponytail: Windows needs Toolhelp32 (windows-sys feature `Win32_System_Diagnostics_ToolHelp`) to read the
    // parent; pwsh passes its own path, so only doctor's "parent is cmd.exe" check is lost.
    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    {
        None
    }
}

/// Mark that existing terminals need a restart (state `doctor.prompt-restart-terminal`, printed by doctor).
pub fn mark_restart() {
    if let Ok(mut s) = crate::settings::Store::load_state() {
        s.map.insert("doctor.prompt-restart-terminal".into(), true.into());
        let _ = s.save();
    }
}

/// Single-quote `s` for `shell` (SPEC §5.3): POSIX `'\''`, fish `\'` + `\\`, pwsh `''`.
pub fn quote(shell: Shell, s: &str) -> String {
    match shell {
        Shell::Pwsh => format!("'{}'", s.replace('\'', "''")),
        Shell::Fish => format!("'{}'", s.replace('\\', "\\\\").replace('\'', "\\'")),
        Shell::Bash | Shell::Zsh => format!("'{}'", s.replace('\'', r"'\''")),
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
        crate::logger::debug(&format!("init {:?} {} --rcfile {rc}", args.shell, args.when.name()));
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
    init_output(shell, when, std::env::var_os("TABSMITH_INIT_SNAPSHOT_TEST").is_some())
}

/// `snapshot` = deterministic output (SPEC §2.7): shell `/bin/zsh`, launch 0, binary `tabsmith`.
fn init_output(shell: Shell, when: When, snapshot: bool) -> Result<String> {
    let bin = if snapshot { "tabsmith".to_string() } else { exe_path()? };
    let launch = |snap| if snap || should_launch().0 { 0 } else { 1 };
    let mut lines = Vec::new();
    if when == When::Pre {
        match shell {
            Shell::Bash | Shell::Zsh => {
                let parent = if snapshot { Some("/bin/zsh".to_string()) } else { parent_exe() };
                if let Some(p) = parent {
                    lines.push(format!("TABSMITH_SHELL={}", quote(shell, &p)));
                }
                lines.push(format!("SHOULD_TABSMITH_LAUNCH={}", launch(snapshot)));
            }
            Shell::Fish => lines.push(format!("set -g SHOULD_TABSMITH_LAUNCH {}", launch(snapshot))),
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
    lines.push(body.trim_end_matches('\n').replace("{{BIN}}", &quote(shell, &bin)));
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
        (false, if cfg!(windows) { "stdin is not a console" } else { "stdin is not a tty" })
    } else {
        (true, "launching")
    };
    (launch, format!("{} {why}", if launch { "✅" } else { "❌" }))
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
    let Some(sid) = std::env::var("TABSMITH_SESSION_ID")
        .ok()
        .filter(|s| !s.is_empty() && s.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_'))
    else {
        return Ok(1);
    };
    let env = std::env::vars_os().filter_map(|(k, v)| Some((k.into_string().ok()?, v.into_string().ok()?))).collect();
    let ctx = SessionCtx { env, alias, ts: now_ms() };
    write_private(&SessionCtx::path(&sid)?, serde_json::to_string(&ctx)?.as_bytes())?;
    Ok(0)
}

/// Atomic write (temp + rename); Unix: file 0600, parent dir 0700 (SPEC §2.11).
fn write_private(path: &Path, bytes: &[u8]) -> Result<()> {
    let dir = path.parent().context("no parent directory")?;
    std::fs::create_dir_all(dir)?;
    let tmp = path.with_extension(format!("tmp-{}", std::process::id()));
    let mut opts = std::fs::OpenOptions::new();
    opts.write(true).create(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
        std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o700))?;
        opts.mode(0o600);
    }
    opts.open(&tmp)?.write_all(bytes)?;
    std::fs::rename(&tmp, path).with_context(|| format!("writing {}", path.display()))?;
    Ok(())
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
        let var = |k: &str| std::env::var_os(k).filter(|v| !v.is_empty()).map(PathBuf::from);
        Ok(Ctx {
            data: crate::settings::home()?,
            bin: exe_path()?,
            zdotdir: var("ZDOTDIR"),
            fish_dir: var("XDG_CONFIG_HOME").unwrap_or_else(|| home.join(".config")).join("fish"),
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
            IntegrationError::FileDoesNotExist(p) => write!(f, "File does not exist: {}", p.display()),
            IntegrationError::PermissionDenied(p) => write!(f, "Permission denied: {}", p.display()),
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

/// Write through symlinks, creating the parent directory.
fn write(path: &Path, contents: &str) -> Result<(), IntegrationError> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| io_err(dir, e))?;
    }
    std::fs::write(path, contents).map_err(|e| io_err(path, e))
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
        Err(IntegrationError::Improper(format!("{} should contain:\n{expected}", path.display())))
    }
}

/// `# tabsmith ignore` (case-insensitive): the user opted this file out.
fn is_ignore_line(line: &str) -> bool {
    line.trim_start()
        .strip_prefix('#')
        .is_some_and(|r| r.trim_start().to_ascii_lowercase().starts_with("tabsmith ignore"))
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
    let dot = |path: PathBuf| Integration { shell, path, file_when: None };
    Ok(match shell {
        Shell::Bash => {
            let extra: Vec<PathBuf> = [".profile", ".bash_login", ".bash_profile"]
                .iter()
                .map(|f| ctx.home.join(f))
                .filter(|p| p.exists())
                .collect();
            let extra = if extra.is_empty() { vec![ctx.home.join(".profile")] } else { extra };
            std::iter::once(ctx.home.join(".bashrc")).chain(extra).map(dot).collect()
        }
        Shell::Zsh => {
            let dir = ctx.zdotdir.as_ref().unwrap_or(&ctx.home);
            vec![dot(dir.join(".zshrc")), dot(dir.join(".zprofile"))]
        }
        Shell::Fish => [(When::Pre, "00_tabsmith_pre.fish"), (When::Post, "99_tabsmith_post.fish")]
            .into_iter()
            .map(|(w, f)| Integration { shell, path: ctx.fish_dir.join("conf.d").join(f), file_when: Some(w) })
            .collect(),
        Shell::Pwsh => vec![dot(match &ctx.pwsh_profile {
            Some(p) => p.clone(),
            None => pwsh_profile()?,
        })],
    })
}

/// `$PROFILE` (CurrentUserCurrentHost) from `pwsh -NoLogo -NoProfile -Command '$PROFILE'` (5 s timeout).
fn pwsh_profile() -> Result<PathBuf> {
    let out = output_with_timeout(
        Command::new("pwsh").args(["-NoLogo", "-NoProfile", "-Command", "$PROFILE"]),
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
        self.path.file_name().map(|f| f.to_string_lossy().into_owned()).unwrap_or_default()
    }

    /// `--rcfile` value: `bashrc`, `zprofile`, `00_tabsmith_pre`, `profile` (pwsh).
    fn rc(&self) -> String {
        if self.shell == Shell::Pwsh {
            return "profile".into();
        }
        self.file_name().trim_start_matches('.').split('.').next().unwrap_or_default().to_string()
    }

    /// `~/.tabsmith/shell/<rc>.<when>.<ext>` (dotfile integrations).
    pub fn script_path(&self, ctx: &Ctx, when: When) -> PathBuf {
        let ext = if self.shell == Shell::Pwsh { "ps1" } else { self.shell.name() };
        ctx.data.join("shell").join(format!("{}.{}.{ext}", self.rc(), when.name()))
    }

    /// Exact bytes of a script file / fish `conf.d` file (SPEC §5.5).
    pub fn script_contents(&self, ctx: &Ctx, when: When) -> String {
        let (b, w, rc) = (quote(self.shell, &ctx.bin), when.name(), self.rc());
        match self.shell {
            Shell::Bash => {
                format!("[ -n \"$BASH_VERSION\" ] && [ -x {b} ] && eval \"$({b} init bash {w} --rcfile {rc})\"\n")
            }
            Shell::Zsh => format!("[ -x {b} ] && eval \"$({b} init zsh {w} --rcfile {rc})\"\n"),
            Shell::Fish => format!("test -x {b}; and {b} init fish {w} --rcfile {rc} | source\n"),
            Shell::Pwsh => format!(
                "if (Test-Path -LiteralPath {b}) {{ Invoke-Expression ((& {b} init pwsh {w} --rcfile profile) -join \"`n\") }}\n"
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
            write(&self.script_path(ctx, when), &self.script_contents(ctx, when))?;
        }
        Ok(())
    }

    /// SPEC §5.5 `is_installed()`: the first failing reason.
    pub fn is_installed(&self, ctx: &Ctx) -> Result<(), IntegrationError> {
        if let Some(when) = self.file_when {
            return check_file(&self.path, &self.script_contents(ctx, when));
        }
        let text = read(&self.path)?;
        if text.lines().any(is_ignore_line) {
            return Ok(());
        }
        let lines: Vec<&str> = text.lines().map(str::trim).filter(|l| !l.is_empty() && !l.starts_with('#')).collect();
        for (when, at, actual) in [(When::Pre, "first", lines.first()), (When::Post, "last", lines.last())] {
            let src = self.source_line(ctx, when);
            if actual.copied() != Some(src.as_str()) {
                let msg = format!("{} does not source {} integration", self.path.display(), when.name());
                return Err(if lines.contains(&src.as_str()) {
                    IntegrationError::Improper(format!("{msg} {at}"))
                } else {
                    IntegrationError::NotInstalled(msg)
                });
            }
            check_file(&self.script_path(ctx, when), &self.script_contents(ctx, when))?;
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
            std::fs::copy(&self.path, dir.join(self.file_name())).map_err(|e| io_err(&self.path, e))?;
            self.uninstall(ctx)?;
            read(&self.path)?
        } else {
            String::new()
        };
        self.write_scripts(ctx)?;
        let (shebang, rest) = split_shebang(&contents);
        let new = format!("{shebang}{PRE_MARKER}\n{}\n{rest}", self.source_line(ctx, When::Pre));
        let new = format!("{}\n{POST_MARKER}\n{}\n", new.trim_end_matches('\n'), self.source_line(ctx, When::Post));
        if new != contents {
            write(&self.path, &new)?;
        }
        Ok(())
    }

    /// SPEC §5.5 uninstall: drop our exact lines, then delete the script files (missing is fine).
    pub fn uninstall(&self, ctx: &Ctx) -> Result<(), IntegrationError> {
        if self.file_when.is_some() {
            return remove(&self.path);
        }
        if self.path.exists() {
            let text = read(&self.path)?;
            let ours = [
                PRE_MARKER.to_string(),
                self.source_line(ctx, When::Pre),
                POST_MARKER.to_string(),
                self.source_line(ctx, When::Post),
            ];
            let kept: String = text.split_inclusive('\n').filter(|l| !ours.iter().any(|o| o == l.trim_end())).collect();
            let new = format!("{}\n", kept.trim());
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
    apply(ctx, shells, |i| i.is_installed(ctx).is_err(), Integration::install)
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
            println!("{}", if installed { "Installed!" } else { "Already installed" });
        }
        Ok(0)
    };
    let uninstall = |target: &IntegrationTarget, silent: bool| -> Result<i32> {
        let done = apply(&ctx, &target_shells(target), |i| i.is_installed(&ctx).is_ok(), Integration::uninstall)?;
        if !silent {
            println!("{}", if done { "Uninstalled!" } else { "Not installed" });
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
            println!("{} {}", if *ok { paint("32", "✔") } else { paint("31", "✘") }, i.describe());
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
    let out = if format == OutputFormat::Json { v.to_string() } else { serde_json::to_string_pretty(&v)? };
    println!("{out}");
    Ok(0)
}

// ---------------------------------------------------------------------------------------------------------------
// setup / uninstall (SPEC §2.9)

/// `[Y/n]` prompt on a tty; empty, `y` or `yes` = yes.
fn confirm(prompt: &str) -> Result<bool> {
    if !std::io::stdin().is_terminal() {
        bail!("You must run with --no-confirm if unattended");
    }
    print!("{prompt}");
    std::io::stdout().flush()?;
    let mut line = String::new();
    std::io::stdin().read_line(&mut line)?;
    Ok(matches!(line.trim().to_ascii_lowercase().as_str(), "" | "y" | "yes"))
}

pub fn setup(args: SetupArgs) -> anyhow::Result<i32> {
    if is_root() && !args.force {
        eprintln!("{}", epaint("1;31", "Installing as root is not supported."));
        eprintln!("{}", epaint("31", "If you know what you're doing, run the command again with --force."));
        return Ok(1);
    }
    let auto = args.no_confirm
        || confirm("Do you want tabsmith to modify your shell config (you will have to manually do this otherwise)? [Y/n] ")?;
    let ctx = Ctx::from_env()?;
    let shells: Vec<Shell> = Shell::ALL.into_iter().filter(|s| which(s.name()).is_some()).collect();
    let mut manual = !auto;
    if auto {
        match install_dotfiles(&ctx, &shells) {
            Ok(installed) => {
                if installed {
                    mark_restart();
                }
                println!("{}", if installed { "Installed!" } else { "Already installed" });
            }
            Err(e) => {
                println!("{}", paint("1", "Could not automatically install:"));
                println!("{e:#}");
                manual = true;
            }
        }
    }
    if manual {
        print_manual(&ctx);
        // Script files only: the user declined (or we failed) to edit the dotfiles themselves.
        let r = apply(&ctx, &shells, |i| i.file_when.is_none(), Integration::write_scripts);
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

/// Manual install instructions (Kiro `installation.rs` text, tabsmith paths).
fn print_manual(ctx: &Ctx) {
    let dir = ctx.data.join("shell");
    let dir = match dir.strip_prefix(&ctx.home) {
        Ok(rel) => format!("$HOME/{}", rel.iter().map(|c| c.to_string_lossy()).collect::<Vec<_>>().join("/")),
        Err(_) => dir.display().to_string(),
    };
    println!();
    println!("To install the integrations manually, you will have to add the following to your rc files");
    println!("This step is required for the application to function properly");
    println!();
    println!("At the top of your .bashrc or .zshrc file:");
    println!("bash:    . \"{dir}/bashrc.pre.bash\"");
    println!("zsh:     . \"{dir}/zshrc.pre.zsh\"");
    println!();
    println!("At the bottom of your .bashrc or .zshrc file:");
    println!("bash:    . \"{dir}/bashrc.post.bash\"");
    println!("zsh:     . \"{dir}/zshrc.post.zsh\"");
    println!();
}

pub fn uninstall(args: UninstallArgs) -> anyhow::Result<i32> {
    if !args.no_confirm && !confirm("Are you sure want to continue uninstalling tabsmith? [Y/n] ")? {
        println!("Cancelled");
        return Ok(1);
    }
    let ctx = Ctx::from_env()?;
    if let Err(e) = apply(&ctx, &default_shells(), |_| true, Integration::uninstall) {
        eprintln!("{e:#}");
    }
    if args.purge {
        match std::fs::remove_dir_all(&ctx.data) {
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => {
                eprintln!("Could not remove {}: {e}", ctx.data.display())
            }
            _ => {}
        }
    }
    println!("Uninstalled tabsmith.");
    println!("Remove the binary with your package manager, or delete {}.", ctx.bin);
    Ok(0)
}

#[cfg(test)]
mod tests {
    use super::*;

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

    /// A fresh, unique temp dir (never the real home).
    fn tmp(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("tabsmith-integrations-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

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
                ("shellcheck", vec!["-s".into(), "bash".into(), "-e".into(), "SC1090,SC1091,SC2148".into(), file.display().to_string()]),
            ],
            Shell::Zsh => vec![("zsh", vec!["-n".into(), f])],
            Shell::Fish => vec![("fish", vec!["--no-execute".into(), f])],
            Shell::Pwsh => vec![(
                "pwsh",
                vec![
                    "-NoProfile".into(),
                    "-Command".into(),
                    format!("$null = [ScriptBlock]::Create((Get-Content -Raw -LiteralPath {}))", quote(Shell::Pwsh, &f)),
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
            assert_eq!(std::fs::read_to_string(&path).unwrap(), got, "{} differs (TABSMITH_BLESS=1 to update)", path.display());
            assert!(!got.contains("{{BIN}}"));
        }
        assert!(snapshot(Shell::Zsh, When::Pre).starts_with("TABSMITH_SHELL='/bin/zsh'\nSHOULD_TABSMITH_LAUNCH=0\n"));
        assert!(snapshot(Shell::Fish, When::Pre).starts_with("set -g SHOULD_TABSMITH_LAUNCH 0\n"));
        assert!(snapshot(Shell::Pwsh, When::Pre).starts_with("$TABSMITH_SHOULD_LAUNCH = 0\n"));
        assert!(snapshot(Shell::Pwsh, When::Pre).contains("& 'tabsmith' _ pty"));
    }

    #[test]
    fn integrations_init_syntax() {
        let dir = tmp("syntax");
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
        let dir = tmp("runtime");
        let cases: [(Shell, &str, &[&str], &str); 3] = [
            (
                Shell::Bash,
                "bash",
                &["--norc", "--noprofile", "-c"],
                r#"PROMPT_COMMAND=$'history -a; '; PS1='$ '; source "$F"; echo "PC=$PROMPT_COMMAND"; _ts_first; _ts_last; _ts_first; _ts_last; printf 'PS1=%s\n' "$PS1""#,
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
                .output()
                .unwrap();
            let s = String::from_utf8_lossy(&out.stdout);
            assert!(out.status.success(), "{prog}: {s}{}", String::from_utf8_lossy(&out.stderr));
            for p in ["OSCUnlock=abc\x07", "Dir=/", "ExitCode=0\x07", &format!("Shell={prog}\x07")] {
                assert!(s.contains(&format!("\x1b]697;{p}")), "{prog} did not emit {p:?}: {s:?}");
            }
            let ps1 = s.rsplit("PS1=").next().unwrap_or_default();
            assert_eq!(ps1.matches("StartPrompt").count(), 1, "{prog} prompt wrapped once: {ps1:?}");
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
    fn integrations_quote() {
        assert_eq!(quote(Shell::Zsh, "/a b/it's"), r"'/a b/it'\''s'");
        assert_eq!(quote(Shell::Fish, r"/a\it's"), r"'/a\\it\'s'");
        assert_eq!(quote(Shell::Pwsh, r"C:\it's"), r"'C:\it''s'");
    }

    #[test]
    fn integrations_dotfile_install_uninstall() {
        let dir = tmp("dotfile");
        let c = ctx(&dir);
        std::fs::create_dir_all(&c.home).unwrap();
        let zshrc = c.home.join(".zshrc");
        let original = "#!/bin/zsh\nexport A=1\nalias ll='ls -l'\n";
        std::fs::write(&zshrc, original).unwrap();

        let rc = Integration { shell: Shell::Zsh, path: zshrc.clone(), file_when: None };
        assert!(matches!(rc.is_installed(&c), Err(IntegrationError::NotInstalled(_))));
        rc.install(&c).unwrap();
        rc.is_installed(&c).unwrap();
        let once = std::fs::read_to_string(&zshrc).unwrap();
        let lines: Vec<&str> = once.lines().collect();
        assert_eq!(lines[0], "#!/bin/zsh");
        assert_eq!(lines[1], PRE_MARKER);
        assert_eq!(lines[2], r#"[ -f "${HOME}/.tabsmith/shell/zshrc.pre.zsh" ] && . "${HOME}/.tabsmith/shell/zshrc.pre.zsh""#);
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
        let backups: Vec<_> = std::fs::read_dir(c.data.join("backups")).unwrap().flatten().collect();
        assert_eq!(backups.len(), 1);
        assert_eq!(std::fs::read_to_string(backups[0].path().join(".zshrc")).unwrap(), original);

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
        let dir = tmp("detect");
        let c = ctx(&dir);
        let bashrc = c.home.join(".bashrc");
        let rc = Integration { shell: Shell::Bash, path: bashrc.clone(), file_when: None };
        assert!(matches!(rc.is_installed(&c), Err(IntegrationError::FileDoesNotExist(_))));

        rc.install(&c).unwrap(); // creates the file
        let installed = std::fs::read_to_string(&bashrc).unwrap();
        syntax_ok(Shell::Bash, &bashrc);

        std::fs::write(&bashrc, format!("echo first\n{installed}")).unwrap();
        let e = rc.is_installed(&c).unwrap_err().to_string();
        assert!(e.ends_with("does not source pre integration first"), "{e}");

        std::fs::write(&bashrc, format!("{installed}echo last\n# trailing comment\n\n")).unwrap();
        let e = rc.is_installed(&c).unwrap_err().to_string();
        assert!(e.starts_with("Improper integration installation: ") && e.ends_with("post integration last"), "{e}");

        // A moved binary: script bytes differ.
        std::fs::write(&bashrc, &installed).unwrap();
        let moved = Ctx { bin: "/elsewhere/tabsmith".into(), ..c.clone() };
        let e = rc.is_installed(&moved).unwrap_err().to_string();
        assert!(e.contains("bashrc.pre.bash should contain:\n[ -n \"$BASH_VERSION\" ] && [ -x '/elsewhere/tabsmith' ]"), "{e}");
        rc.install(&moved).unwrap();
        rc.is_installed(&moved).unwrap();

        // Opt-out marker wins.
        std::fs::write(&bashrc, "  #  TABSMITH ignore\necho hi\n").unwrap();
        rc.is_installed(&c).unwrap();
        rc.install(&c).unwrap();
        assert_eq!(std::fs::read_to_string(&bashrc).unwrap(), "  #  TABSMITH ignore\necho hi\n");
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn integrations_enumeration_and_whole_files() {
        let dir = tmp("enum");
        let c = ctx(&dir);
        std::fs::create_dir_all(&c.home).unwrap();
        let names = |s| integrations(&c, s).unwrap().iter().map(Integration::file_name).collect::<Vec<_>>();
        assert_eq!(names(Shell::Bash), [".bashrc", ".profile"]);
        std::fs::write(c.home.join(".bash_profile"), "").unwrap();
        assert_eq!(names(Shell::Bash), [".bashrc", ".bash_profile"]);
        assert_eq!(names(Shell::Zsh), [".zshrc", ".zprofile"]);

        let fish = integrations(&c, Shell::Fish).unwrap();
        assert_eq!(fish.iter().map(Integration::describe).collect::<Vec<_>>(), ["fish pre", "fish post"]);
        for i in &fish {
            i.install(&c).unwrap();
            i.is_installed(&c).unwrap();
            syntax_ok(Shell::Fish, &i.path);
        }
        assert_eq!(
            std::fs::read_to_string(c.fish_dir.join("conf.d/00_tabsmith_pre.fish")).unwrap(),
            "test -x '/opt/tab smith/it\\'s/tabsmith'; and '/opt/tab smith/it\\'s/tabsmith' init fish pre --rcfile 00_tabsmith_pre | source\n"
        );
        for i in &fish {
            i.uninstall(&c).unwrap();
            assert!(!i.path.exists());
        }

        // pwsh: the profile directory is created; script files are `profile.<when>.ps1`.
        let pwsh = &integrations(&c, Shell::Pwsh).unwrap()[0];
        assert_eq!(pwsh.describe(), "pwsh pre post into Microsoft.PowerShell_profile.ps1");
        pwsh.install(&c).unwrap();
        pwsh.is_installed(&c).unwrap();
        let text = std::fs::read_to_string(&pwsh.path).unwrap();
        assert!(text.contains(r#"if (Test-Path "${HOME}/.tabsmith/shell/profile.pre.ps1") { . "${HOME}/.tabsmith/shell/profile.pre.ps1" }"#));
        syntax_ok(Shell::Pwsh, &pwsh.path);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn integrations_install_dotfiles_reports() {
        let dir = tmp("apply");
        let c = ctx(&dir);
        assert!(install_dotfiles(&c, &[Shell::Zsh, Shell::Fish]).unwrap());
        assert!(!install_dotfiles(&c, &[Shell::Zsh, Shell::Fish]).unwrap());
        // A dotfile that cannot be written is reported with its description.
        std::fs::create_dir_all(c.home.join(".bashrc")).unwrap(); // a directory where the file should be
        let e = format!("{:#}", install_dotfiles(&c, &[Shell::Bash]).unwrap_err());
        assert!(e.starts_with("\n\n") && e.contains("bash pre post into .bashrc") && !e.contains(".profile"), "{e}");
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
            (&[("TABSMITH_FORCE_LAUNCH", "1"), ("CI", "1")], false, true, "✅ TABSMITH_FORCE_LAUNCH"),
            (&[("TABSMITH_TERM_DISABLED", "1")], true, false, "❌ TABSMITH_TERM_DISABLED"),
            (&[("PROCESS_LAUNCHED_BY_TABSMITH", "1")], true, false, "❌ PROCESS_LAUNCHED_BY_TABSMITH"),
            (&[("TABSMITH_TERM", "0.1.0")], true, false, "❌ already inside tabsmith"),
            (&[("TABSMITH_TERM", "0.1.0"), ("TMUX", "/tmp/t")], true, true, "✅ new tmux pane"),
            (&[("TABSMITH_TERM", "0.1.0"), ("TMUX", "/tmp/t"), ("TABSMITH_TERM_TMUX", "0.1.0")], true, false, "❌ already inside tabsmith"),
            (&[("INSIDE_EMACS", "29")], true, false, "❌ INSIDE_EMACS"),
            (&[("TERM_PROGRAM", "WarpTerminal")], true, false, "❌ TERM_PROGRAM = WarpTerminal"),
            (&[("TERM_PROGRAM", "iTerm.app")], true, true, "✅ launching"),
            (&[("__CFBundleIdentifier", "com.vandyke.SecureCRT")], true, false, "❌ __CFBundleIdentifier = com.vandyke.SecureCRT"),
            (&[("__PWSH_LOGIN_CHECKED", "1")], true, false, "❌ __PWSH_LOGIN_CHECKED"),
            (&[("CI", "true")], true, false, "❌ In CI"),
            (&[("VSCODE_RESOLVING_ENVIRONMENT", "1")], true, false, "❌ VSCODE_RESOLVING_ENVIRONMENT"),
            (&[("TERM", "dumb")], true, false, "❌ TERM is unset or dumb"),
            (&[("SSH_TTY", "/dev/ttys001")], true, true, "✅ launching"),
        ];
        for (vars, stdin_ok, launch, why) in cases {
            let env: HashMap<String, String> =
                base.iter().chain(vars.iter()).map(|(k, v)| (k.to_string(), v.to_string())).collect();
            assert_eq!(launch_decision(&env, *stdin_ok), (*launch, why.to_string()), "{vars:?}");
        }
        assert_eq!(launch_decision(&HashMap::new(), true), (false, "❌ TERM is unset or dumb".to_string()));
    }

    #[cfg(unix)]
    #[test]
    fn integrations_pre_cmd_file() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tmp("precmd");
        let path = dir.join("run").join("abc123.json");
        let ctx = SessionCtx { env: [("A".to_string(), "1".to_string())].into(), alias: Some("ll='ls -l'".into()), ts: 7 };
        write_private(&path, serde_json::to_string(&ctx).unwrap().as_bytes()).unwrap();
        let back = SessionCtx::load(&path).unwrap();
        assert_eq!((back.env["A"].as_str(), back.alias.as_deref(), back.ts), ("1", Some("ll='ls -l'"), 7));
        assert_eq!(std::fs::metadata(&path).unwrap().permissions().mode() & 0o777, 0o600);
        assert_eq!(std::fs::metadata(dir.join("run")).unwrap().permissions().mode() & 0o777, 0o700);
        let _ = std::fs::remove_dir_all(dir);
    }
}
