//! Small user-facing commands: theme, diagnostic, debug (SPEC §2.4, §2.10).
//! OWNER: cli agent.
use crate::pty::CompletionEngineStarter;
use crate::settings::domain::repositories::settings_repository::SettingsRepository;
use crate::settings::domain::services::settings_catalog::AUTOCOMPLETE_THEME;
use crate::settings::domain::value_objects::setting_value::SettingValue;
use crate::settings::infrastructure::adapters::json_file_settings_repository::JsonFileSettingsRepository;
use crate::settings::infrastructure::adapters::setting_value_json::setting_value_to_json;
use crate::shared::domain::services::shell_quoting;
use crate::shared::domain::value_objects::shell::Shell;
use crate::shared::infrastructure::data_directory_from_env::data_directory_from_env;
use crate::shared::infrastructure::output_format::OutputFormat;
use crate::shared::infrastructure::terminal_style::{bold, sgr, stdout_tty};
use crate::shell_integration::application::render_init_script_use_case::RenderInitScriptUseCase;
use crate::shell_integration::domain::value_objects::integration_phase::IntegrationPhase;
use crate::shell_integration::infrastructure::adapters::embedded_shell_scripts::EmbeddedShellScripts;
use crate::shell_integration::infrastructure::adapters::os_shell_environment::OsShellEnvironment;
use crate::shell_integration::infrastructure::cli::init_command::{init_script, ShellArg};
use anyhow::{bail, Context, Result};
use clap::{Args, Subcommand};
use serde::Deserialize;
use serde_json::{json, Map, Value};
use std::io::{Read, Write};
use std::path::Path;
use std::rc::Rc;

#[derive(Args)]
pub struct ThemeArgs {
    /// List all available themes
    #[arg(long, conflicts_with_all = ["folder", "theme"])]
    pub list: bool,
    /// Print the theme directory path
    #[arg(long, conflicts_with = "theme")]
    pub folder: bool,
    /// Theme name: dark, light, system, or a custom theme
    pub theme: Option<String>,
}

#[derive(Args)]
pub struct DiagnosticArgs {
    #[arg(long, short, value_enum, default_value_t)]
    pub format: OutputFormat,
}

#[derive(Subcommand)]
pub enum DebugCmd {
    /// Print the bytes of every key press (Ctrl-D exits)
    KeyTester,
    /// Start a shell with only the tabsmith integration loaded
    Shell {
        shell: Option<ShellArg>,
    },
    /// Show log file paths and the end of pty.log
    Logs,
}

/// settings.json, or the file `TABSMITH_SETTINGS_PATH` names, resolved when a command reads or writes it.
fn settings_repository() -> JsonFileSettingsRepository {
    JsonFileSettingsRepository::new(data_directory_from_env())
}

// ---------------------------------------------------------------------------------------------------------------
// theme (SPEC §2.4)

const BUILTIN_THEMES: [&str; 3] = ["dark", "light", "system"];

#[derive(Deserialize)]
struct ThemeAuthor {
    name: Option<String>,
    twitter: Option<String>,
    github: Option<String>,
}

/// Custom theme file, parsed only to validate it and print the author (render.rs owns the colours).
#[derive(Deserialize)]
struct ThemeFile {
    author: Option<ThemeAuthor>,
    #[allow(dead_code)]
    version: Option<String>,
    #[allow(dead_code)]
    theme: Option<Map<String, Value>>,
    #[serde(flatten)]
    rest: Map<String, Value>,
}

fn parse_theme(text: &str) -> Result<ThemeFile> {
    let file: ThemeFile = serde_json::from_str(text)?;
    for (k, v) in &file.rest {
        if (k.starts_with("shade") || k.starts_with("accent")) && !v.is_string() {
            bail!("{k} must be a string");
        }
    }
    Ok(file)
}

pub fn theme(args: ThemeArgs) -> Result<i32> {
    let dir = data_directory_from_env().themes_dir()?;
    if args.folder {
        std::fs::create_dir_all(&dir)?;
        println!("{}", dir.display());
        return Ok(0);
    }
    if args.list {
        theme_list(&dir).iter().for_each(|t| println!("{t}"));
        return Ok(0);
    }
    let repository = settings_repository();
    let settings = repository.load()?;
    let Some(name) = args.theme else {
        println!(
            "{}",
            match settings.get(AUTOCOMPLETE_THEME) {
                Some(v) if v.as_str().is_none() => format!("{:#}", setting_value_to_json(v)),
                _ => settings
                    .string(AUTOCOMPLETE_THEME)
                    .unwrap_or_default()
                    .to_string(),
            }
        );
        return Ok(0);
    };
    let color = stdout_tty();
    let path = dir.join(format!("{name}.json"));
    if let Ok(text) = std::fs::read_to_string(&path) {
        let file =
            parse_theme(&text).with_context(|| format!("invalid theme file {}", path.display()))?;
        print!("{}", theme_banner(&name, &file, color));
    } else if BUILTIN_THEMES.contains(&name.as_str()) {
        println!("› Switching to theme '{}'", bold(&name, color));
    } else {
        bail!("'{name}' does not exist in {}", dir.display());
    }
    repository.save(&settings.with(AUTOCOMPLETE_THEME, SettingValue::Text(name)))?;
    Ok(0)
}

/// Built-ins first, then the folder's `*.json` stems sorted (built-in names not repeated).
fn theme_list(dir: &Path) -> Vec<String> {
    let mut files: Vec<String> = std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|e| {
            let p = e.path();
            if p.extension()? != "json" {
                return None;
            }
            Some(p.file_stem()?.to_str()?.to_string())
        })
        .filter(|s| !BUILTIN_THEMES.contains(&s.as_str()))
        .collect();
    files.sort();
    files.dedup();
    BUILTIN_THEMES
        .iter()
        .map(|s| s.to_string())
        .chain(files)
        .collect()
}

fn theme_banner(name: &str, file: &ThemeFile, color: bool) -> String {
    let mut out = format!("\n› Switching to theme '{}'", bold(name, color));
    if let Some(author) = &file.author {
        if let Some(n) = &author.name {
            out += &format!(" by {}", bold(n, color));
        }
        if let Some(t) = &author.twitter {
            out += &format!("\n  🐦 {t}");
        }
        if let Some(g) = &author.github {
            out += &format!("\n  💻 github.com/{g}");
        }
    }
    out + "\n\n"
}

// ---------------------------------------------------------------------------------------------------------------
// diagnostic (SPEC §2.10)

type Section = (&'static str, Vec<(String, Value)>);

pub fn diagnostic(args: DiagnosticArgs) -> Result<i32> {
    let sections = diagnostics();
    println!(
        "{}",
        args.format.render(
            || to_toml(&sections),
            || {
                Value::Object(
                    sections
                        .iter()
                        .map(|(n, rows)| {
                            (n.to_string(), Value::Object(rows.iter().cloned().collect()))
                        })
                        .collect(),
                )
            },
        )
    );
    Ok(0)
}

fn diagnostics() -> Vec<Section> {
    let user = std::env::var(if cfg!(windows) { "USERNAME" } else { "USER" }).unwrap_or_default();
    let clean = |s: String| {
        if user.is_empty() {
            s
        } else {
            s.replace(&format!("/{user}"), "/USER")
                .replace(&format!("\\{user}"), "\\USER")
        }
    };
    let env = |k: &str| std::env::var(k).ok().filter(|v| !v.is_empty());
    let opt = |v: Option<String>| v.map(|s| Value::String(clean(s))).unwrap_or(Value::Null);

    // ponytail: shell = $SHELL (not the parent process); unset on Windows → omitted.
    let shell = env("SHELL");
    let shell_version = shell
        .as_ref()
        .and_then(|sh| {
            std::process::Command::new(sh)
                .arg("--version")
                .output()
                .ok()
        })
        .and_then(|o| {
            String::from_utf8_lossy(&o.stdout)
                .lines()
                .next()
                .map(|l| l.trim().to_string())
        });
    let terminal = env("TERM_PROGRAM")
        .or_else(|| env("WT_SESSION").map(|_| "Windows Terminal".into()))
        .or_else(|| env("TERM"));
    let in_ssh = ["SSH_CLIENT", "SSH_CONNECTION", "SSH_TTY"]
        .iter()
        .any(|k| env(k).is_some());
    let in_wsl = cfg!(target_os = "linux")
        && (env("WSL_DISTRO_NAME").is_some()
            || std::fs::read_to_string("/proc/sys/kernel/osrelease")
                .is_ok_and(|s| s.to_lowercase().contains("microsoft")));

    let mut env_vars: Vec<(String, Value)> = std::env::vars_os()
        .filter_map(|(k, v)| {
            let k = k.into_string().ok()?;
            let keep = k.starts_with("TABSMITH_")
                || ["SHELL", "TERM", "COLORTERM", "ZDOTDIR", "PATH"].contains(&k.as_str());
            keep.then(|| (k, Value::String(clean(v.to_string_lossy().into_owned()))))
        })
        .collect();
    env_vars.sort_by(|a, b| a.0.cmp(&b.0));

    let row = |k: &str, v: Value| (k.to_string(), v);
    vec![
        (
            "tabsmith",
            vec![
                row("version", json!(env!("CARGO_PKG_VERSION"))),
                row(
                    "os",
                    json!(format!(
                        "{} {}",
                        std::env::consts::OS,
                        std::env::consts::ARCH
                    )),
                ),
            ],
        ),
        (
            "environment",
            vec![
                row(
                    "cwd",
                    opt(std::env::current_dir()
                        .ok()
                        .map(|p| p.display().to_string())),
                ),
                row(
                    "cli-path",
                    opt(std::env::current_exe()
                        .ok()
                        .map(|p| p.display().to_string())),
                ),
                row("shell-path", opt(shell)),
                row("shell-version", opt(shell_version)),
                row("terminal", opt(terminal)),
                row("in-ssh", json!(in_ssh)),
                row("in-ci", json!(env("CI").is_some())),
                row("in-wsl", json!(in_wsl)),
            ],
        ),
        ("env-vars", env_vars),
    ]
}

/// Hand-written TOML: JSON scalars are valid TOML values; nulls are omitted.
fn to_toml(sections: &[Section]) -> String {
    let mut out = String::new();
    for (name, rows) in sections {
        out += &format!("[{name}]\n");
        for (k, v) in rows.iter().filter(|(_, v)| !v.is_null()) {
            let bare = k
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-');
            let key = if bare {
                k.clone()
            } else {
                Value::String(k.clone()).to_string()
            };
            out += &format!("{key} = {v}\n");
        }
        out.push('\n');
    }
    out.trim_end().to_string()
}

// ---------------------------------------------------------------------------------------------------------------
// debug (SPEC §2.10)

pub fn debug(cmd: DebugCmd, start: CompletionEngineStarter) -> Result<i32> {
    match cmd {
        DebugCmd::KeyTester => key_tester(),
        DebugCmd::Shell { shell } => debug_shell(shell.map(Into::into), start),
        DebugCmd::Logs => debug_logs(),
    }
}

fn key_line(input: &[u8]) -> String {
    let hex: Vec<String> = input.iter().map(|b| format!("{b:#04x}")).collect();
    format!(
        "{} bytes : \"{}\" : [{}]",
        input.len(),
        input.escape_ascii(),
        hex.join(", ")
    )
}

/// ponytail: Windows reads through std's console stdin, so keys without VT input mode show as text only.
fn key_tester() -> Result<i32> {
    let color = stdout_tty();
    println!(
        "{} (use {} to quit)",
        bold("Testing Key Input", color),
        sgr("ctrl-d", "35", color)
    );
    crossterm::terminal::enable_raw_mode()?;
    let res = (|| -> Result<()> {
        let (mut stdin, mut stdout) = (std::io::stdin().lock(), std::io::stdout().lock());
        let mut buf = [0u8; 1024];
        loop {
            let n = stdin.read(&mut buf)?;
            if n == 0 {
                return Ok(());
            }
            write!(stdout, "{}\r\n", key_line(&buf[..n]))?;
            stdout.flush()?;
            if buf[..n] == [4] {
                return Ok(());
            }
        }
    })();
    crossterm::terminal::disable_raw_mode()?;
    res?;
    println!("ctrl-d");
    Ok(0)
}

/// `_ pty` with only the post integration loaded.
fn debug_shell(shell: Option<Shell>, start: CompletionEngineStarter) -> Result<i32> {
    let env_shell = std::env::var("SHELL").ok().filter(|s| !s.is_empty());
    let shell = match shell {
        Some(s) => s,
        None if cfg!(windows) => Shell::Pwsh,
        None => env_shell
            .as_deref()
            .and_then(Shell::from_name)
            .context("Could not determine current shell or shell not supported")?,
    };
    // The user's own binary when $SHELL is that shell, else the name (resolved on PATH by the wrapper).
    let exe = env_shell
        .filter(|s| Shell::from_name(s) == Some(shell))
        .unwrap_or_else(|| shell.name().to_string());
    let post = init_script(
        &RenderInitScriptUseCase::new(Rc::new(OsShellEnvironment), Rc::new(EmbeddedShellScripts)),
        shell,
        IntegrationPhase::Post,
    )? + "\n";
    let (dir, cmd) = debug_shell_files(&data_directory_from_env().root()?, shell, &exe, &post)?;
    if shell == Shell::Zsh {
        std::env::set_var("ZDOTDIR", &dir);
    }
    // The wrapper passes the current env to the child.
    std::env::set_var("TABSMITH_DEBUG_SHELL", "1");
    let _ = writeln!(std::io::stdout(), "Starting tabsmith debug shell\n");
    let code = crate::pty::run(Some(exe), cmd, start);
    let _ = std::fs::remove_dir_all(&dir);
    // The terminal may be gone (window closed): no panic, the shell's code still counts.
    let _ = writeln!(std::io::stdout(), "\nEnding tabsmith debug shell\n");
    Ok(code)
}

/// Write the rc file `post` into a fresh private dir under `home` (the data dir, never a shared temp dir whose name
/// another user could claim first); returns the dir (zsh's ZDOTDIR) and the shell argv that loads the rc file.
fn debug_shell_files(
    home: &Path,
    shell: Shell,
    exe: &str,
    post: &str,
) -> Result<(std::path::PathBuf, Vec<String>)> {
    let dir = home
        .join("run")
        .join(format!("debug-shell-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir); // a stale dir of ours from a crashed run with the same pid
    let mut builder = std::fs::DirBuilder::new();
    builder.recursive(true);
    #[cfg(unix)]
    std::os::unix::fs::DirBuilderExt::mode(&mut builder, 0o700);
    builder.create(&dir)?;
    let rc = dir.join(match shell {
        Shell::Bash => "bashrc",
        Shell::Zsh => ".zshrc",
        Shell::Fish => "post.fish",
        Shell::Pwsh => "post.ps1",
    });
    std::fs::write(&rc, post)?;
    let rc = rc.display().to_string();
    let q = |s: &str| shell_quoting::quote(shell, s);
    let mut cmd = vec![exe.to_string()];
    match shell {
        Shell::Bash => cmd.extend(["--noprofile".into(), "--rcfile".into(), rc]),
        Shell::Zsh => {}
        // --no-config also skips fish's default theme: without an autosuggestion colour, tabsmith takes fish's
        // suggestion for typed text.
        Shell::Fish => cmd.extend([
            "--no-config".into(),
            "-C".into(),
            format!("set -q fish_color_autosuggestion[1]; or set -g fish_color_autosuggestion brblack; source {}", q(&rc)),
        ]),
        Shell::Pwsh => cmd.extend(["-NoProfile".into(), "-NoExit".into(), "-Command".into(), format!(". {}", q(&rc))]),
    }
    Ok((dir, cmd))
}

fn debug_logs() -> Result<i32> {
    let dir = data_directory_from_env().logs_dir()?;
    let pty = dir.join("pty.log");
    println!("{}\n{}", dir.join("cli.log").display(), pty.display());
    if let Ok(bytes) = std::fs::read(&pty) {
        let text = String::from_utf8_lossy(&bytes);
        let lines: Vec<&str> = text.lines().collect();
        println!();
        lines[lines.len().saturating_sub(50)..]
            .iter()
            .for_each(|l| println!("{l}"));
    }
    Ok(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shared::infrastructure::testing::temp_dir::TempDir;

    #[test]
    fn theme_list_order() {
        let temp_dir = TempDir::new("themes");
        let d = temp_dir.path().to_path_buf();
        for f in ["zed.json", "alpha.json", "dark.json", "notes.txt"] {
            std::fs::write(d.join(f), "{}").unwrap();
        }
        assert_eq!(theme_list(&d), ["dark", "light", "system", "alpha", "zed"]);
        assert_eq!(theme_list(&d.join("missing")), ["dark", "light", "system"]);
        std::fs::remove_dir_all(&d).unwrap();
    }

    #[test]
    fn theme_file_parsing() {
        let f = parse_theme(
            r##"{"author":{"name":"Ann","github":"ann"},"version":"1.0","shade0":"#000000"}"##,
        )
        .unwrap();
        assert_eq!(
            theme_banner("ocean", &f, false),
            "\n› Switching to theme 'ocean' by Ann\n  💻 github.com/ann\n\n"
        );
        assert_eq!(
            theme_banner("ocean", &parse_theme("{}").unwrap(), false),
            "\n› Switching to theme 'ocean'\n\n"
        );
        assert!(parse_theme(r#"{"author":"Ann"}"#).is_err());
        assert!(parse_theme(r#"{"shade3":5}"#).is_err());
        assert!(parse_theme("not json").is_err());
    }

    #[test]
    fn key_tester_line() {
        assert_eq!(
            key_line(b"\x1b[A"),
            "3 bytes : \"\\x1b[A\" : [0x1b, 0x5b, 0x41]"
        );
        assert_eq!(key_line(&[4]), "1 bytes : \"\\x04\" : [0x04]");
    }

    #[test]
    fn diagnostic_toml() {
        let sections: Vec<Section> = vec![
            (
                "tabsmith",
                vec![
                    ("version".into(), json!("0.1.0")),
                    ("missing".into(), Value::Null),
                ],
            ),
            (
                "env-vars",
                vec![
                    ("PATH".into(), json!("/a:\"b\"")),
                    ("in-ci".into(), json!(false)),
                ],
            ),
        ];
        assert_eq!(
            to_toml(&sections),
            "[tabsmith]\nversion = \"0.1.0\"\n\n[env-vars]\nPATH = \"/a:\\\"b\\\"\"\nin-ci = false"
        );
    }

    /// The rc dir is private and fresh, and fish/pwsh load an rc path containing `'` (run when installed).
    #[test]
    fn debug_shell_rc_dir_and_quoting() {
        let temp_dir = TempDir::new("o'brien");
        let home = temp_dir.path().to_path_buf();
        let stale = home
            .join("run")
            .join(format!("debug-shell-{}", std::process::id()));
        std::fs::create_dir_all(&stale).unwrap();
        std::fs::write(stale.join("planted"), "").unwrap();
        let cases = [
            (Shell::Fish, "set -g __ts_m loaded\n", "; echo $__ts_m"),
            (Shell::Pwsh, "$__ts_m = 'loaded'\n", "; $__ts_m"),
        ];
        for (shell, post, show) in cases {
            let (dir, argv) = debug_shell_files(&home, shell, shell.name(), post).unwrap();
            assert_eq!(dir, stale);
            assert!(
                !dir.join("planted").exists(),
                "the dir is recreated, not reused"
            );
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                assert_eq!(
                    std::fs::metadata(&dir).unwrap().permissions().mode() & 0o777,
                    0o700
                );
            }
            if crate::shared::infrastructure::os::executable_lookup::which(shell.name()).is_none() {
                continue;
            }
            let load = format!("{}{show}", argv.last().unwrap());
            let args: Vec<&str> = match shell {
                Shell::Fish => vec!["--no-config", "-c", &load],
                _ => vec!["-NoProfile", "-Command", &load],
            };
            let out = std::process::Command::new(shell.name())
                .args(args)
                .output()
                .unwrap();
            let text = String::from_utf8_lossy(&out.stdout);
            assert!(
                text.trim() == "loaded",
                "{shell:?}: {text}{}",
                String::from_utf8_lossy(&out.stderr)
            );
            if shell == Shell::Fish {
                // --no-config skips the default theme: the debug shell still colours fish's autosuggestions.
                let load = format!("{}; echo $fish_color_autosuggestion", argv.last().unwrap());
                let out = std::process::Command::new("fish")
                    .args(["--no-config", "-c", &load])
                    .output()
                    .unwrap();
                assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), "brblack");
            }
        }
        let _ = std::fs::remove_dir_all(home);
    }

    /// Closing the terminal of a `debug shell` (its window goes away) ends it cleanly: printing the goodbye to the
    /// dead terminal does not panic, the shell's code is returned and the rc dir is removed. Re-runs this test in a
    /// child process on a PTY, then closes the PTY.
    #[cfg(unix)]
    #[test]
    fn debug_shell_survives_closed_terminal() {
        use crate::bootstrap::channel_completion_engine::start_completion_engine;
        use portable_pty::{native_pty_system, CommandBuilder, PtySize};
        const NAME: &str = "cli::tests::debug_shell_survives_closed_terminal";
        if let Some(marker) = std::env::var_os("TABSMITH_TEST_DEBUG_SHELL") {
            let code = debug_shell(Some(Shell::Bash), start_completion_engine);
            std::fs::write(marker, format!("{code:?}")).unwrap();
            return;
        }
        if crate::shared::infrastructure::os::executable_lookup::which("bash").is_none() {
            return;
        }
        let temp_dir = TempDir::new("dbgclose");
        let home = temp_dir.path().to_path_buf();
        let marker = home.join("marker");
        let pair = native_pty_system()
            .openpty(PtySize {
                rows: 24,
                cols: 80,
                pixel_width: 0,
                pixel_height: 0,
            })
            .unwrap();
        let mut cb = CommandBuilder::new(std::env::current_exe().unwrap());
        cb.args([NAME, "--exact", "--nocapture"]);
        cb.env_clear(); // no terminal integration (kitty, Ghostty, VS Code) of the terminal running the tests
        cb.env("PATH", std::env::var_os("PATH").unwrap_or_default());
        cb.env("TERM", "xterm-256color");
        cb.env("TABSMITH_TEST_DEBUG_SHELL", &marker);
        cb.env("TABSMITH_HOME", &home);
        cb.env("HOME", &home);
        cb.env("HISTFILE", "/dev/null");
        cb.env("TABSMITH_INIT_SNAPSHOT_TEST", "1"); // the post block calls `tabsmith`, not this test binary
        let mut child = pair.slave.spawn_command(cb).unwrap();
        drop(pair.slave);
        // Read the master itself (a cloned reader would keep the PTY open) until the wrapper runs (it sets the
        // title), then close it.
        let fd = pair.master.as_raw_fd().unwrap();
        let (mut out, mut b) = (Vec::new(), [0u8; 4096]);
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(20);
        while !String::from_utf8_lossy(&out).contains("\x1b]0;")
            && std::time::Instant::now() < deadline
        {
            let mut p = libc::pollfd {
                fd,
                events: libc::POLLIN,
                revents: 0,
            };
            // SAFETY: poll and read on the master fd we own, into a local buffer.
            if unsafe { libc::poll(&mut p, 1, 100) } == 1 {
                let n = unsafe { libc::read(fd, b.as_mut_ptr().cast(), b.len()) };
                if n <= 0 {
                    break;
                }
                out.extend_from_slice(&b[..n as usize]);
            }
        }
        let out = String::from_utf8_lossy(&out).into_owned();
        drop(pair.master);
        let exited = (0..200).any(|_| {
            std::thread::sleep(std::time::Duration::from_millis(50));
            child.try_wait().ok().flatten().is_some()
        });
        if !exited {
            let _ = child.kill();
        }
        assert!(exited, "{out:?}");
        assert!(
            std::fs::read_to_string(&marker)
                .unwrap_or_default()
                .starts_with("Ok("),
            "panicked: {out:?}"
        );
        let left: Vec<_> = std::fs::read_dir(home.join("run"))
            .into_iter()
            .flatten()
            .flatten()
            .map(|e| e.file_name())
            .collect();
        assert!(
            left.iter()
                .all(|n| !n.to_string_lossy().starts_with("debug-shell-")),
            "{left:?}"
        );
        let _ = std::fs::remove_dir_all(home);
    }
}
