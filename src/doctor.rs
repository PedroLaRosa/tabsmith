//! `tabsmith doctor` (SPEC §2.8). OWNER: integrations agent.
use crate::integrations;
use crate::settings::domain::services::settings_catalog::{
    AUTOCOMPLETE_DEVELOPER_MODE, AUTOCOMPLETE_DISABLE, INLINE_ENABLED, INLINE_SOURCE,
};
use crate::settings::infrastructure::adapters::json_file_settings_repository::JsonFileSettingsRepository;
use crate::settings::infrastructure::load_or_defaults::load_or_defaults;
use crate::settings::domain::value_objects::settings::Settings;
use crate::shared::domain::value_objects::shell::Shell;
use crate::shared::infrastructure::data_directory_from_env::data_directory_from_env;
use crate::shared::infrastructure::json_document_store::JsonDocumentStore;
use crate::shared::infrastructure::os::executable_lookup::which;
use crate::shared::infrastructure::os::process_info;
use crate::shared::infrastructure::os::process_runner::output_with_timeout;
use crate::shared::infrastructure::terminal_style::{epaint, paint};
use crate::shell_integration::domain::entities::integration::Integration;
use crate::shell_integration::domain::shell_integration_error::ShellIntegrationError;
use crate::shell_integration::domain::value_objects::integration_paths::IntegrationPaths;
use crate::shell_integration::domain::value_objects::integration_phase::IntegrationPhase;
use crate::shell_integration::infrastructure::adapters::os_shell_environment::integration_paths_from;
use crate::types::now_secs;
use clap::Args;
use std::io::IsTerminal;
use std::path::Path;
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

#[derive(Args)]
pub struct DoctorArgs {
    /// Run all checks, print every result, never fix, never abort
    #[arg(long, short)]
    pub all: bool,
    /// Error on warnings
    #[arg(long, short)]
    pub strict: bool,
}

/// Normal; Soft (an error becomes a warning unless `--strict`); Skip (not run).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Kind {
    Normal,
    Soft,
    Skip,
}

type Fix<'a> = Box<dyn FnOnce() -> anyhow::Result<()> + 'a>;

enum Outcome<'a> {
    Ok,
    Warn(String),
    Err {
        reason: String,
        info: Vec<String>,
        fix: Option<Fix<'a>>,
    },
}

fn err<'a>(reason: impl Into<String>) -> Outcome<'a> {
    Outcome::Err {
        reason: reason.into(),
        info: Vec::new(),
        fix: None,
    }
}

struct Check<'a> {
    name: String,
    kind: Kind,
    run: Box<dyn Fn() -> Outcome<'a> + 'a>,
}

fn check<'a>(name: impl Into<String>, kind: Kind, run: impl Fn() -> Outcome<'a> + 'a) -> Check<'a> {
    Check {
        name: name.into(),
        kind,
        run: Box::new(run),
    }
}

fn print_outcome(name: &str, o: &Outcome) {
    match o {
        Outcome::Ok => println!("{} {name}", paint("32", "✔")),
        Outcome::Warn(m) => println!("{} {m}", paint("33", "●")),
        Outcome::Err { reason, info, .. } => {
            println!("{} {name}: {reason}", paint("31", "✘"));
            if !info.is_empty() {
                println!();
                for l in info {
                    println!("  {l}");
                }
            }
        }
    }
}

/// The check loop state (SPEC §2.8 flow step 3).
struct Run {
    all: bool,
    strict: bool,
    errors: usize,
    warnings: usize,
    spinner: Option<Spinner>,
}

impl Run {
    fn outcome<'a>(&self, c: &Check<'a>) -> Outcome<'a> {
        match (c.run)() {
            Outcome::Err { reason, .. } if c.kind == Kind::Soft && !self.strict => {
                Outcome::Warn(reason)
            }
            o => o,
        }
    }

    fn count(&mut self, o: &Outcome) {
        match o {
            Outcome::Ok => {}
            Outcome::Warn(_) => self.warnings += 1,
            Outcome::Err { .. } => self.errors += 1,
        }
    }

    /// Run one group; false = abort the remaining groups (an error that could not be fixed, without `--all`).
    fn group(&mut self, header: &str, checks: Vec<Check>) -> bool {
        if self.all {
            println!("{}", paint("2", header));
        }
        for c in checks.iter().filter(|c| c.kind != Kind::Skip) {
            let out = self.outcome(c);
            if self.all || !matches!(out, Outcome::Ok) {
                self.spinner = None;
                print_outcome(&c.name, &out);
            }
            if self.all {
                self.count(&out);
                continue;
            }
            let Outcome::Err { fix, .. } = out else {
                self.count(&out);
                continue;
            };
            if let Some(fix) = fix {
                println!("Attempting to fix automatically...");
                match fix() {
                    Err(e) => println!("Failed to fix: {e:#}"),
                    Ok(()) => {
                        println!("Re-running check...");
                        println!();
                        let again = self.outcome(c);
                        print_outcome(&c.name, &again);
                        if !matches!(again, Outcome::Err { .. }) {
                            self.count(&again);
                            continue;
                        }
                    }
                }
            }
            println!();
            self.errors += 1;
            return false;
        }
        if self.all {
            println!();
        }
        true
    }
}

pub fn run(args: DoctorArgs) -> anyhow::Result<i32> {
    if process_info::is_root() {
        eprintln!(
            "{}",
            epaint("1;31", "Running doctor as root is not supported.")
        );
        if !args.all {
            eprintln!(
                "{}",
                epaint(
                    "31",
                    "If you know what you're doing, run the command again with --all."
                )
            );
            return Ok(1);
        }
    }
    let ctx = integration_paths_from(&data_directory_from_env())?;
    let parent = process_info::parent_exe();
    let current = parent.as_deref().and_then(Shell::from_name);
    let mut r = Run {
        all: args.all,
        strict: args.strict,
        errors: 0,
        warnings: 0,
        spinner: if args.all { None } else { Spinner::start() },
    };
    let ok = r.group(
        "Let's check your dotfiles...",
        dotfile_checks(&ctx, current),
    ) && r.group(
        "Let's make sure tabsmith is set up correctly...",
        setup_checks(&ctx),
    ) && r.group(
        "Let's check if your environment is supported...",
        env_checks(&ctx, parent.as_deref()),
    );
    if ok {
        // Result ignored (SPEC §2.8): printed, but neither aborts nor counts.
        let (e, w) = (r.errors, r.warnings);
        r.group(
            "Let's check if your system is compatible...",
            system_checks(current, parent.as_deref()),
        );
        (r.errors, r.warnings) = (e, w);
    }
    r.spinner = None;

    let failed = r.errors > 0 || (args.strict && r.warnings > 0);
    if failed {
        println!();
        println!(
            "{} Doctor found errors. Please fix them and try again.",
            paint("31", "✘")
        );
        println!();
    } else {
        if !args.all {
            println!("{} Everything looks good!", paint("32", "✔"));
        }
        println!();
        println!("  tabsmith still not working? Check ~/.tabsmith/logs/pty.log");
        println!();
    }
    let state_file = data_directory_from_env().state_file();
    if let Some(mut state) = state_file.ok().and_then(|path| JsonDocumentStore::load(&path).ok()) {
        if state
            .map
            .get("doctor.prompt-restart-terminal")
            .and_then(|v| v.as_bool())
            == Some(true)
        {
            println!(
                "  {}",
                paint(
                    "1",
                    "PS. Autocomplete won't work in any existing terminal sessions, only new ones."
                )
            );
            println!("  (You might want to restart your terminal emulator)");
            state
                .map
                .insert("doctor.prompt-restart-terminal".into(), false.into());
            let _ = state.save();
        }
    }
    Ok(i32::from(failed))
}

/// `~/rel` when under home.
fn short_path(home: &Path, p: &Path) -> String {
    match p.strip_prefix(home) {
        Ok(rel) => format!("~/{}", rel.display()),
        Err(_) => p.display().to_string(),
    }
}

/// `Let's check your dotfiles...`: one check per integration of every shell.
fn dotfile_checks(ctx: &IntegrationPaths, current: Option<Shell>) -> Vec<Check<'_>> {
    let mut v = Vec::new();
    for shell in Shell::ALL {
        let kind = if Some(shell) == current {
            Kind::Normal
        } else if which(shell.name()).is_some() {
            Kind::Soft
        } else {
            continue; // NoCheck (and no `pwsh` spawn when it is missing)
        };
        match integrations::integrations(ctx, shell) {
            Ok(list) => v.extend(list.into_iter().map(|i| dotfile_check(ctx, i, kind))),
            Err(e) => {
                let msg = format!("{e:#}");
                v.push(check(
                    format!("{} integration check", shell.name()),
                    kind,
                    move || err(msg.clone()),
                ));
            }
        }
    }
    v
}

fn dotfile_check(ctx: &IntegrationPaths, i: Integration, kind: Kind) -> Check<'_> {
    let name = format!(
        "{} {} integration check",
        i.shell().name(),
        short_path(ctx.home(), i.path())
    );
    check(name, kind, move || {
        let e = match i.is_installed(ctx) {
            Ok(()) => return Outcome::Ok,
            Err(e) => e,
        };
        let shell = i.shell().name();
        let cmd = paint(
            "35",
            &format!("tabsmith integrations install dotfiles {shell}"),
        );
        let info = vec![format!(
            "Run {cmd} to reinstall shell integrations for {shell}"
        )];
        let reason = match &e {
            ShellIntegrationError::NotInstalled(m) | ShellIntegrationError::Improper(m) => m.clone(),
            e => e.to_string(),
        };
        if matches!(e, ShellIntegrationError::PermissionDenied(_)) {
            return Outcome::Err {
                reason,
                info,
                fix: None,
            };
        }
        let i = i.clone();
        let fix: Fix = Box::new(move || {
            i.install(ctx)?;
            integrations::mark_restart();
            Ok(())
        });
        Outcome::Err {
            reason,
            info,
            fix: Some(fix),
        }
    })
}

/// (shell, first single-quoted path) of the script files and fish `conf.d` files of the current integrations of
/// every shell on PATH. Not every file in `~/.tabsmith/shell`: one of a dotfile deleted since (or of a shell gone
/// from PATH) is sourced by nothing, and install, the fix, would never rewrite it.
fn script_bins(ctx: &IntegrationPaths) -> Vec<(Shell, String)> {
    // pwsh's script files do not depend on $PROFILE: no second `pwsh` spawn to ask for it.
    let ctx = &ctx
        .clone()
        .with_pwsh_profile(ctx.pwsh_profile().map(Path::to_path_buf).unwrap_or_default());
    let mut files = Vec::new();
    for shell in Shell::ALL.into_iter().filter(|s| which(s.name()).is_some()) {
        for i in integrations::integrations(ctx, shell).unwrap_or_default() {
            match i.whole_file_phase() {
                Some(_) => files.push((shell, i.path().to_path_buf())),
                None => {
                    files.extend([IntegrationPhase::Pre, IntegrationPhase::Post].map(|w| (shell, i.script(ctx, w).path().to_path_buf())))
                }
            }
        }
    }
    files
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

/// `Let's make sure tabsmith is set up correctly...`
fn setup_checks(ctx: &IntegrationPaths) -> Vec<Check<'_>> {
    let settings_path =
        data_directory_from_env().settings_file().unwrap_or_else(|_| ctx.data_root().join("settings.json"));
    let specs_index = ctx
        .data_root()
        .join("specs")
        .join(crate::specs::version())
        .join("index.json");
    #[cfg_attr(not(windows), allow(unused_mut))]
    let mut v = vec![
        check("tabsmith data dir exists", Kind::Normal, move || {
            if ctx.data_root().is_dir() {
                return Outcome::Ok;
            }
            let fix: Fix = Box::new(move || Ok(std::fs::create_dir_all(ctx.data_root())?));
            Outcome::Err {
                reason: format!("No directory at path {}", ctx.data_root().display()),
                info: vec![],
                fix: Some(fix),
            }
        }),
        check(
            "Integration scripts point at this binary",
            Kind::Normal,
            move || {
                let stale: Vec<(Shell, String)> = script_bins(ctx)
                    .into_iter()
                    .filter(|(_, b)| *b != ctx.binary())
                    .collect();
                let Some((_, old)) = stale.first() else {
                    return Outcome::Ok;
                };
                let mut shells: Vec<Shell> = stale.iter().map(|(s, _)| *s).collect();
                shells.dedup();
                let fix: Fix = Box::new(move || {
                    integrations::install_dotfiles(ctx, &shells)?;
                    integrations::mark_restart();
                    Ok(())
                });
                Outcome::Err {
                    reason: format!("Integration scripts reference {old}"),
                    info: vec![],
                    fix: Some(fix),
                }
            },
        ),
        check("tabsmith settings corruption", Kind::Normal, move || {
            if JsonDocumentStore::load(&settings_path).is_ok() {
                return Outcome::Ok;
            }
            let path = settings_path.clone();
            let fix: Fix = Box::new(move || {
                std::fs::copy(
                    &path,
                    path.with_extension(format!("json.bak-{}", now_secs())),
                )?;
                std::fs::write(&path, "{}\n")?;
                Ok(())
            });
            Outcome::Err {
                reason: "tabsmith settings file is corrupted".into(),
                info: vec![],
                fix: Some(fix),
            }
        }),
        check("Completion specs installed", Kind::Normal, move || {
            if specs_index.is_file() {
                return Outcome::Ok;
            }
            let fix: Fix = Box::new(|| crate::specs::ensure_extracted().map(|_| ()));
            Outcome::Err {
                reason: "Completion specs are not extracted".into(),
                info: vec![],
                fix: Some(fix),
            }
        }),
    ];
    #[cfg(windows)]
    {
        v.push(check("Windows Console Check", Kind::Normal, || {
            use std::os::windows::io::AsRawHandle;
            if process_info::is_console(std::io::stdin().as_raw_handle())
                && process_info::is_console(std::io::stdout().as_raw_handle())
            {
                return Outcome::Ok;
            }
            Outcome::Err {
                reason: "Windows Console APIs are not supported in this terminal".into(),
                info: [
                    "The pseudoterminal layer only supports the new Windows Console API.",
                    "MinTTY and other TTY implementations may not work properly.",
                    "",
                    "You can try the following fixes to get completions working:",
                    "- If using Git for Windows, reinstall and choose \"Use default console window\" instead of MinTTY",
                    "- If using Git for Windows and you really want to use MinTTY, reinstall and check \"Enable experimental support for pseudo consoles\"",
                    "- Use your shell with a supported terminal emulator like Windows Terminal.",
                    "- Launch your terminal emulator with winpty (e.g. winpty mintty). NOTE: this can lead to some UI bugs.",
                ]
                .map(String::from)
                .to_vec(),
                fix: None,
            }
        }));
        v.push(check(
            "Windows version",
            Kind::Normal,
            || match windows_build() {
                Some(b) if b < 17763 => err("Windows 10 1809 or newer is required (ConPTY)"),
                _ => Outcome::Ok,
            },
        ));
    }
    v
}

/// `Let's check if your environment is supported...`
fn env_checks<'a>(ctx: &'a IntegrationPaths, parent: Option<&str>) -> Vec<Check<'a>> {
    let var = |k: &str| std::env::var(k).ok();
    let in_cmd = parent.is_some_and(|p| {
        let base = p.rsplit(['/', '\\']).next().unwrap_or(p);
        base.eq_ignore_ascii_case("cmd.exe") || base.eq_ignore_ascii_case("cmd")
    });
    let settings = load_or_defaults(&JsonFileSettingsRepository::new(data_directory_from_env()));
    let sid = var("TABSMITH_SESSION_ID");
    let inline_kind = if settings.bool(INLINE_ENABLED) {
        Kind::Normal
    } else {
        Kind::Soft
    };
    let s2 = settings.clone();
    let s3 = settings.clone();
    vec![
        check("tabsmith terminal integrations", Kind::Normal, move || {
            let has = |k: &str| std::env::var_os(k).is_some();
            let reason = if var("TERM_PROGRAM").as_deref() == Some("WarpTerminal") {
                "WarpTerminal is not supported"
            } else if in_cmd {
                "CMD isn't supported, use PowerShell 7 (pwsh) instead"
            } else if has("INSIDE_EMACS") {
                "Emacs is not supported"
            } else if var("__CFBundleIdentifier").as_deref() == Some("com.vandyke.SecureCRT") {
                "SecureCRT is not supported"
            } else if has("PROCESS_LAUNCHED_BY_TABSMITH") {
                "tabsmith can not run in a process it launched"
            } else if has("CI") {
                "Doctor can not run in CI"
            } else {
                return match var("TABSMITH_TERM") {
                    Some(v) if v == env!("CARGO_PKG_VERSION") => Outcome::Ok,
                    None => err("tabsmith is not running in this terminal, please try restarting your terminal"),
                    Some(v) => Outcome::Err {
                        reason: "This terminal is not running with the latest integration, please restart your terminal"
                            .into(),
                        info: vec![format!("TABSMITH_TERM={v}")],
                        fix: None,
                    },
                };
            };
            err(reason)
        }),
        check(
            "Session check",
            session_kind(sid.as_deref(), parent, var("TABSMITH_SHELL").as_deref()),
            move || {
                let sid = sid.clone().unwrap_or_default();
                if ctx.data_root().join("run").join(format!("{sid}.json")).is_file() {
                    Outcome::Ok
                } else {
                    Outcome::Warn("Shell has not reported its environment yet (tabsmith _ pre-cmd did not run)".into())
                }
            },
        ),
        check("Autocomplete is enabled", Kind::Normal, move || {
            if !settings.bool(AUTOCOMPLETE_DISABLE) {
                return Outcome::Ok;
            }
            Outcome::Err {
                reason: "Autocomplete disabled.".into(),
                info: vec!["To fix run: tabsmith settings autocomplete.disable false".into()],
                fix: None,
            }
        }),
        check("Autocomplete dev mode", Kind::Normal, move || {
            if s2.bool(AUTOCOMPLETE_DEVELOPER_MODE) {
                Outcome::Warn("autocomplete.developerMode is enabled".into())
            } else {
                Outcome::Ok
            }
        }),
        check("Inline", inline_kind, move || {
            inline_outcome(&s3, |k| std::env::var_os(k).is_some_and(|v| !v.is_empty()))
        }),
    ]
}

/// The Session check runs inside a wrapped session, except pwsh, which never runs `_ pre-cmd` (SPEC §5.4.5).
/// The session shell is the parent process, else `TABSMITH_SHELL` (exported by the pre scripts).
fn session_kind(sid: Option<&str>, parent: Option<&str>, tabsmith_shell: Option<&str>) -> Kind {
    let shell = parent
        .and_then(Shell::from_name)
        .or_else(|| tabsmith_shell.and_then(Shell::from_name));
    if sid.is_some() && shell != Some(Shell::Pwsh) {
        Kind::Normal
    } else {
        Kind::Skip
    }
}

/// The `Inline` check; `env_set(name)` = the variable is set and non-empty.
fn inline_outcome<'a>(settings: &Settings, env_set: impl Fn(&str) -> bool) -> Outcome<'a> {
    use crate::inline::Source;
    if !settings.bool(INLINE_ENABLED) {
        return Outcome::Warn(
            "Inline is disabled, to re-enable run: tabsmith inline enable".into(),
        );
    }
    match crate::inline::source(settings) {
        Err(_) => err(format!(
            "Inline source '{}' is not a valid profile",
            settings.string(INLINE_SOURCE).unwrap_or_default()
        )),
        Ok(Source::Profile(p)) if !is_local(&p.base_url) && !env_set(&p.api_key_env) => {
            err(format!("{} is not set", p.api_key_env))
        }
        Ok(_) => Outcome::Ok,
    }
}

fn is_local(url: &str) -> bool {
    ["://localhost", "://127.0.0.1", "://[::1]"]
        .iter()
        .any(|h| url.contains(h))
}

/// Numbers of the first dotted version in `s` (`GNU bash, version 5.2.37(1)` → [5, 2, 37]).
fn version_of(s: &str) -> Option<Vec<u32>> {
    let v = s
        .split(|c: char| !(c.is_ascii_digit() || c == '.'))
        .find(|t| t.contains('.') && t.starts_with(|c: char| c.is_ascii_digit()))?;
    v.split('.')
        .filter(|p| !p.is_empty())
        .map(|p| p.parse().ok())
        .collect()
}

/// Version printed by `prog args` (5 s timeout).
fn version_from(prog: &str, args: &[&str]) -> Option<Vec<u32>> {
    version_of(&output_with_timeout(Command::new(prog).args(args), Duration::from_secs(5)).ok()?)
}

/// Windows build number from `cmd /C ver` (`Microsoft Windows [Version 10.0.19045.3803]`).
#[cfg(windows)]
fn windows_build() -> Option<u32> {
    // ponytail: GetVersionExW lies without an app manifest and RtlGetVersion needs another windows-sys feature.
    version_from("cmd", &["/C", "ver"])?.get(2).copied()
}

/// `Let's check if your system is compatible...`
fn system_checks<'a>(current: Option<Shell>, parent: Option<&str>) -> Vec<Check<'a>> {
    let bash = if current == Some(Shell::Bash) {
        parent.unwrap_or("bash").to_string()
    } else {
        "bash".into()
    };
    vec![
        check("OS is supported", Kind::Normal, os_outcome),
        check(
            "Bash is new enough",
            if current == Some(Shell::Bash) {
                Kind::Soft
            } else {
                Kind::Skip
            },
            move || {
                match version_from(&bash, &["--version"]) {
                Some(v) if v.as_slice() < [4, 4].as_slice() => err(format!(
                    "Bash {}.{} is not supported (needs >= 4.4 for PS0); install a newer bash or switch to zsh",
                    v[0],
                    v.get(1).unwrap_or(&0)
                )),
                _ => Outcome::Ok,
            }
            },
        ),
        check(
            "Fish is up to date",
            if which("fish").is_some() {
                Kind::Normal
            } else {
                Kind::Skip
            },
            || match version_from("fish", &["--version"]) {
                Some(v) if v.as_slice() < [3, 3, 0].as_slice() => {
                    let v: Vec<String> = v.iter().map(u32::to_string).collect();
                    err(format!(
                        "your fish version is outdated (need at least 3.3.0, found {})",
                        v.join(".")
                    ))
                }
                _ => Outcome::Ok,
            },
        ),
    ]
}

/// macOS >= 11, Linux, Windows >= 1809.
fn os_outcome<'a>() -> Outcome<'a> {
    match std::env::consts::OS {
        "linux" => Outcome::Ok,
        "macos" => match version_from("sw_vers", &["-productVersion"]) {
            Some(v) if v[0] < 11 => {
                let v: Vec<String> = v.iter().map(u32::to_string).collect();
                Outcome::Warn(format!(
                    "macOS {} is not supported (needs macOS 11 or newer)",
                    v.join(".")
                ))
            }
            _ => Outcome::Ok,
        },
        #[cfg(windows)]
        "windows" => match windows_build() {
            Some(b) if b < 17763 => {
                Outcome::Warn("Windows 10 1809 or newer is required (ConPTY)".into())
            }
            _ => Outcome::Ok,
        },
        os => Outcome::Warn(format!("{os} is not supported")),
    }
}

/// `Running checks...` on stderr (Braille dots, 80 ms) with a hidden cursor; dropping it clears the line and
/// shows the cursor. Ctrl-C while doctor runs also restores the cursor (exit 1).
struct Spinner {
    stop: Arc<AtomicBool>,
    thread: Option<std::thread::JoinHandle<()>>,
}

impl Spinner {
    fn start() -> Option<Spinner> {
        if !std::io::stderr().is_terminal() {
            return None;
        }
        restore_cursor_on_ctrl_c();
        let stop = Arc::new(AtomicBool::new(false));
        let flag = stop.clone();
        let thread = std::thread::spawn(move || {
            for frame in "⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏".chars().cycle() {
                if flag.load(Ordering::Relaxed) {
                    break;
                }
                eprint!("\x1b[?25l\r{frame} Running checks...");
                std::thread::sleep(Duration::from_millis(80));
            }
        });
        Some(Spinner {
            stop,
            thread: Some(thread),
        })
    }
}

impl Drop for Spinner {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
        eprint!("\r\x1b[2K\x1b[?25h");
    }
}

#[cfg(unix)]
fn restore_cursor_on_ctrl_c() {
    if let Ok(mut signals) = signal_hook::iterator::Signals::new([libc::SIGINT]) {
        std::thread::spawn(move || {
            if signals.forever().next().is_some() {
                eprint!("\x1b[?25h");
                std::process::exit(1);
            }
        });
    }
}

#[cfg(windows)]
fn restore_cursor_on_ctrl_c() {
    unsafe extern "system" fn handler(_ctrl: u32) -> i32 {
        eprint!("\x1b[?25h");
        std::process::exit(1)
    }
    // SAFETY: registers a plain function pointer that lives for the whole process.
    unsafe {
        windows_sys::Win32::System::Console::SetConsoleCtrlHandler(Some(handler), 1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shared::infrastructure::testing::temp_dir::TempDir;
    use std::cell::Cell;

    fn run(all: bool, strict: bool) -> Run {
        Run {
            all,
            strict,
            errors: 0,
            warnings: 0,
            spinner: None,
        }
    }

    #[test]
    fn doctor_group_soft_fix_abort() {
        // Soft error → warning (not with --strict); fixed error → continue; unfixed error → abort.
        let fixed = Cell::new(false);
        let checks = || {
            vec![
                check("soft", Kind::Soft, || err("meh")),
                check("skipped", Kind::Skip, || panic!("NoCheck must not run")),
                check("fixable", Kind::Normal, || {
                    if fixed.get() {
                        return Outcome::Ok;
                    }
                    let fix: Fix = Box::new(|| {
                        fixed.set(true);
                        Ok(())
                    });
                    Outcome::Err {
                        reason: "broken".into(),
                        info: vec![],
                        fix: Some(fix),
                    }
                }),
                check("fatal", Kind::Normal, || err("nope")),
                check("after", Kind::Normal, || {
                    panic!("must not run after an abort")
                }),
            ]
        };
        let mut r = run(false, false);
        assert!(!r.group("h", checks()));
        assert!(fixed.get());
        assert_eq!((r.errors, r.warnings), (1, 1));

        fixed.set(false);
        let mut r = run(false, true); // --strict: a Soft error stays an error
        assert!(!r.group("h", vec![check("soft", Kind::Soft, || err("meh"))]));
        assert_eq!((r.errors, r.warnings), (1, 0));

        // --all: never fixes, never aborts.
        let mut r = run(true, false);
        let mut list = checks();
        list.pop();
        assert!(r.group("h", list));
        assert!(!fixed.get());
        assert_eq!((r.errors, r.warnings), (2, 1));
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
        integrations::install_dotfiles(&c, &[Shell::Bash]).unwrap();
        std::fs::remove_file(home.join(".bash_profile")).unwrap();
        let moved = c.with_binary("/new/tabsmith".into());
        integrations::install_dotfiles(&moved, &[Shell::Bash]).unwrap(); // the check's fix
        assert!(moved.data_root().join("shell/bash_profile.pre.bash").exists());
        if which("bash").is_some() {
            let bins = script_bins(&moved);
            assert!(
                !bins.is_empty() && bins.iter().all(|(_, b)| b == "/new/tabsmith"),
                "{bins:?}"
            );
        }
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn doctor_session_check_skips_pwsh() {
        let pwsh = Some(r"C:\Program Files\PowerShell\7\pwsh.exe");
        assert_eq!(session_kind(Some("s1"), pwsh, None), Kind::Skip);
        assert_eq!(session_kind(Some("s1"), None, pwsh), Kind::Skip); // parent unknown: TABSMITH_SHELL decides
        assert_eq!(
            session_kind(Some("s1"), Some("/bin/zsh"), pwsh),
            Kind::Normal
        ); // zsh nested in pwsh runs pre-cmd
        assert_eq!(
            session_kind(Some("s1"), Some("/bin/zsh"), None),
            Kind::Normal
        );
        assert_eq!(session_kind(None, Some("/bin/zsh"), None), Kind::Skip);
    }

    #[test]
    fn doctor_version_parsing() {
        assert_eq!(
            version_of("GNU bash, version 3.2.57(1)-release (arm64-apple-darwin24)"),
            Some(vec![3, 2, 57])
        );
        assert_eq!(version_of("fish, version 3.7.1"), Some(vec![3, 7, 1]));
        assert_eq!(
            version_of("Microsoft Windows [Version 10.0.19045.3803]"),
            Some(vec![10, 0, 19045, 3803])
        );
        assert_eq!(version_of("14.5\n"), Some(vec![14, 5]));
        assert!(
            [3, 2, 57].as_slice() < [4, 4].as_slice() && [4, 4, 0].as_slice() >= [4, 4].as_slice()
        );
    }
}
