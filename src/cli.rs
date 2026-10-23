//! Small user-facing commands: settings, theme, inline, diagnostic, completion, debug (SPEC §2.3–2.5, §2.10).
//! OWNER: cli agent.
use crate::inline::{Profile, Provider, BUILTIN_SOURCES};
use crate::integrations::When;
use crate::settings::{self, Settings, Store};
use crate::types::Shell;
use anyhow::{bail, Context, Result};
use clap::{ArgGroup, Args, Subcommand, ValueEnum};
use serde::Deserialize;
use serde_json::{json, Map, Value};
use std::io::{IsTerminal, Read, Write};
use std::path::Path;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, ValueEnum)]
pub enum OutputFormat {
    #[default]
    Plain,
    Json,
    JsonPretty,
}

impl OutputFormat {
    /// Plain → `plain()`; json → compact; json-pretty → 2-space indent (Kiro `OutputFormat::print`).
    pub fn render(self, plain: impl FnOnce() -> String, json: impl FnOnce() -> Value) -> String {
        match self {
            OutputFormat::Plain => plain(),
            OutputFormat::Json => json().to_string(),
            OutputFormat::JsonPretty => format!("{:#}", json()),
        }
    }
}

#[derive(Args)]
#[command(subcommand_negates_reqs = true, args_conflicts_with_subcommands = true)]
#[command(group(ArgGroup::new("vals").requires("key").args(["value", "format"])))]
pub struct SettingsArgs {
    #[command(subcommand)]
    pub cmd: Option<SettingsCmd>,
    /// Key of the setting
    pub key: Option<String>,
    /// Value of the setting (JSON, or a plain string)
    pub value: Option<String>,
    /// Delete a setting (wildcards * and ? allowed)
    #[arg(long, short)]
    pub delete: bool,
    /// Output format
    #[arg(long, short, value_enum, default_value_t)]
    pub format: OutputFormat,
}

#[derive(Subcommand)]
pub enum SettingsCmd {
    /// Open the settings file
    Open,
    /// List settings
    List {
        /// List all available settings with descriptions
        #[arg(long)]
        all: bool,
        #[arg(long, short, value_enum, default_value_t)]
        format: OutputFormat,
    },
    #[command(hide = true)]
    All {
        #[arg(long, short, value_enum, default_value_t)]
        format: OutputFormat,
    },
}

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

#[derive(Subcommand)]
pub enum InlineCmd {
    /// Enable inline suggestions
    Enable,
    /// Disable inline suggestions
    Disable,
    /// Show current status
    Status,
    /// Select the inline suggestion source (history, specs, history+specs or an LLM profile)
    #[command(alias = "set-source")]
    SetCustomization { name: Option<String> },
    /// Show the available inline sources
    #[command(alias = "show-sources")]
    ShowCustomizations {
        #[arg(long, short, value_enum, default_value_t)]
        format: OutputFormat,
    },
}

#[derive(Args)]
pub struct DiagnosticArgs {
    #[arg(long, short, value_enum, default_value_t)]
    pub format: OutputFormat,
}

#[derive(Args)]
pub struct CompletionArgs {
    #[arg(value_enum, default_value_t = clap_complete::Shell::Zsh)]
    pub shell: clap_complete::Shell,
}

#[derive(Subcommand)]
pub enum DebugCmd {
    /// Print the bytes of every key press (Ctrl-D exits)
    KeyTester,
    /// Start a shell with only the tabsmith integration loaded
    Shell {
        shell: Option<crate::integrations::ShellArg>,
    },
    /// Show log file paths and the end of pty.log
    Logs,
}

fn stdout_tty() -> bool {
    std::io::stdout().is_terminal()
}

fn sgr(s: &str, code: &str, on: bool) -> String {
    if on {
        format!("\x1b[{code}m{s}\x1b[0m")
    } else {
        s.to_string()
    }
}

fn bold(s: &str, on: bool) -> String {
    sgr(s, "1", on)
}

/// Print `s` + newline unless it is empty (an empty plain list prints nothing).
fn emit(s: &str) {
    if !s.is_empty() {
        println!("{s}");
    }
}

// ---------------------------------------------------------------------------------------------------------------
// settings (SPEC §2.3)

pub fn settings(args: SettingsArgs) -> Result<i32> {
    let path = settings::settings_path()?;
    match args.cmd {
        Some(SettingsCmd::Open) => return open_settings(&path),
        Some(SettingsCmd::List { all, format }) => {
            let map = Store::load(&path)?.map;
            emit(&if all {
                render_list_all(&map, format)
            } else {
                render_list(&map, format)
            });
            return Ok(0);
        }
        Some(SettingsCmd::All { format }) => {
            emit(&render_list(&Store::load(&path)?.map, format));
            return Ok(0);
        }
        None => {}
    }
    let Some(key) = args.key else {
        if args.delete {
            bail!("the argument '--delete' requires a <KEY>\n Usage: tabsmith settings --delete <KEY>");
        }
        // Same as `tabsmith settings --help`.
        if let Err(help) =
            <crate::Cli as clap::Parser>::try_parse_from(["tabsmith", "settings", "--help"])
        {
            help.print()?;
        }
        return Ok(0);
    };
    let mut store = Store::load(&path)?;
    match (args.value, args.delete) {
        (Some(_), true) => {
            bail!("the argument '--delete' cannot be used with '[VALUE]'\n Usage: tabsmith settings --delete {key}")
        }
        (None, false) => println!("{}", render_get(&key, store.map.get(&key), args.format)?),
        (Some(value), false) => {
            store.map.insert(key, parse_value(&value));
            store.save()?;
        }
        (None, true) => {
            let keys = matching_keys(&store.map, &key);
            if keys.is_empty() {
                bail!("No settings found matching {key}");
            }
            println!("{}", removal_message(&keys));
            for k in &keys {
                store.map.remove(k);
            }
            store.save()?;
        }
    }
    Ok(0)
}

/// `VALUE` as JSON, else as a JSON string (`true` → bool, `5` → number, `foo` → "foo").
fn parse_value(value: &str) -> Value {
    serde_json::from_str(value).unwrap_or_else(|_| Value::String(value.to_string()))
}

/// `settings KEY`: plain prints strings raw and other values pretty; json formats print `null` when unset.
fn render_get(key: &str, value: Option<&Value>, format: OutputFormat) -> Result<String> {
    Ok(match (value, format) {
        (None, OutputFormat::Plain) => bail!("No value associated with {key}"),
        (None, _) => "null".to_string(),
        (Some(Value::String(s)), OutputFormat::Plain) => s.clone(),
        (Some(v), OutputFormat::Plain | OutputFormat::JsonPretty) => format!("{v:#}"),
        (Some(v), OutputFormat::Json) => v.to_string(),
    })
}

fn matching_keys(map: &Map<String, Value>, pattern: &str) -> Vec<String> {
    map.keys()
        .filter(|k| settings::wildcard_match(pattern, k))
        .cloned()
        .collect()
}

fn removal_message(keys: &[String]) -> String {
    match keys {
        [one] => format!("Removing {one:?}"),
        _ => std::iter::once("Removing:".to_string())
            .chain(keys.iter().map(|k| format!("  - {k}")))
            .collect::<Vec<_>>()
            .join("\n"),
    }
}

/// `settings list`: configured keys (sorted: serde_json's Map is a BTreeMap).
fn render_list(map: &Map<String, Value>, format: OutputFormat) -> String {
    format.render(
        || {
            map.iter()
                .map(|(k, v)| format!("{k} = {v}"))
                .collect::<Vec<_>>()
                .join("\n")
        },
        || Value::Object(map.clone()),
    )
}

/// `settings list --all`: every catalog key (SPEC §3.3) with its current value.
fn render_list_all(map: &Map<String, Value>, format: OutputFormat) -> String {
    let catalog = settings::catalog();
    format.render(
        || {
            let mut out = String::from("key\ttype\tdefault\tvalue");
            for e in catalog {
                let value = map.get(e.key).map(Value::to_string).unwrap_or_default();
                out += &format!("\n{}\t{}\t{}\t{value}", e.key, e.ty, e.default);
            }
            out
        },
        || {
            catalog
                .iter()
                .map(|e| {
                    json!({
                        "key": e.key,
                        "type": e.ty,
                        "default": serde_json::from_str::<Value>(e.default).unwrap_or(Value::Null),
                        "description": e.description,
                        "value": map.get(e.key),
                    })
                })
                .collect()
        },
    )
}

/// `$EDITOR <path>` (waits), else macOS `open -t`, Windows `notepad`.
fn open_settings(path: &Path) -> Result<i32> {
    if !path.exists() {
        Store {
            path: path.to_path_buf(),
            map: Map::new(),
        }
        .save()?;
    }
    let mut cmd = if let Some(editor) = std::env::var("EDITOR")
        .ok()
        .filter(|e| !e.trim().is_empty())
    {
        // ponytail: whitespace split covers `code --wait`; quoted editor paths with spaces are not supported.
        let mut parts = editor.split_whitespace();
        let mut c = std::process::Command::new(parts.next().unwrap_or_default());
        c.args(parts);
        c
    } else if cfg!(target_os = "macos") {
        let mut c = std::process::Command::new("open");
        c.arg("-t");
        c
    } else if cfg!(windows) {
        std::process::Command::new("notepad")
    } else {
        bail!("The EDITOR environment variable is not set")
    };
    let status = cmd
        .arg(path)
        .status()
        .context("could not launch the editor")?;
    Ok(if status.success() { 0 } else { 1 })
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
    let dir = settings::home()?.join("themes");
    if args.folder {
        std::fs::create_dir_all(&dir)?;
        println!("{}", dir.display());
        return Ok(0);
    }
    if args.list {
        theme_list(&dir).iter().for_each(|t| println!("{t}"));
        return Ok(0);
    }
    let mut store = Store::load_settings()?;
    let Some(name) = args.theme else {
        println!(
            "{}",
            match store.map.get("autocomplete.theme") {
                None => "dark".to_string(),
                Some(Value::String(s)) => s.clone(),
                Some(v) => format!("{v:#}"),
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
    store
        .map
        .insert("autocomplete.theme".into(), Value::String(name));
    store.save()?;
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
// inline (SPEC §2.5)

const PROFILE_EXAMPLE: &str = r#"tabsmith settings inline.profiles.claude '{"provider":"anthropic","model":"claude-opus-5-5","api_key_env":"ANTHROPIC_API_KEY","effort":"low"}'"#;

pub fn inline(cmd: InlineCmd) -> Result<i32> {
    let color = stdout_tty();
    match cmd {
        InlineCmd::Enable => set_inline_enabled(true, color)?,
        InlineCmd::Disable => set_inline_enabled(false, color)?,
        InlineCmd::Status => {
            let on = Settings::from_map(Store::load_settings()?.map).bool("inline.enabled", true);
            println!(
                "Inline is {}",
                bold(if on { "enabled" } else { "disabled" }, color)
            );
        }
        InlineCmd::SetCustomization { name } => set_customization(name, color)?,
        InlineCmd::ShowCustomizations { format } => {
            let s = Settings::from_map(Store::load_settings()?.map);
            print!(
                "{}",
                render_sources(
                    selected_source(&s),
                    &crate::inline::profiles(&s),
                    format,
                    color
                )
            );
        }
    }
    Ok(0)
}

fn set_inline_enabled(on: bool, color: bool) -> Result<()> {
    let mut store = Store::load_settings()?;
    store.map.insert("inline.enabled".into(), Value::Bool(on));
    store.save()?;
    println!(
        "{}",
        sgr(
            if on {
                "Inline enabled"
            } else {
                "Inline disabled"
            },
            "35",
            color
        )
    );
    Ok(())
}

fn selected_source(s: &Settings) -> &str {
    s.string("inline.source").unwrap_or("history+specs")
}

fn provider_name(p: Provider) -> &'static str {
    match p {
        Provider::OpenAi => "openai",
        Provider::Anthropic => "anthropic",
    }
}

fn set_customization(name: Option<String>, color: bool) -> Result<()> {
    let mut store = Store::load_settings()?;
    // Only valid profiles can be selected (an invalid one would only make doctor fail).
    let profiles: Vec<(String, Profile)> =
        crate::inline::profiles(&Settings::from_map(store.map.clone()))
            .into_iter()
            .filter_map(|(n, p)| Some((n, p.ok()?)))
            .collect();
    let name = match name {
        Some(n) => n,
        None => match choose_source(&profiles)? {
            Some(n) => n,
            None => {
                store.map.remove("inline.source");
                store.save()?;
                println!("Customization unset");
                return Ok(());
            }
        },
    };
    if !BUILTIN_SOURCES.iter().any(|(b, _)| *b == name) && !profiles.iter().any(|(n, _)| *n == name)
    {
        bail!("Customization not found");
    }
    store
        .map
        .insert("inline.source".into(), Value::String(name.clone()));
    store.save()?;
    println!("Customization {} selected", bold(&name, color));
    Ok(())
}

/// Numbered prompt; `Ok(None)` = "None" picked. Non-tty or bad input → error.
fn choose_source(profiles: &[(String, Profile)]) -> Result<Option<String>> {
    if !std::io::stdin().is_terminal() {
        bail!("No customization selected");
    }
    let mut rows: Vec<(String, String)> = BUILTIN_SOURCES
        .iter()
        .map(|(n, d)| (n.to_string(), d.to_string()))
        .collect();
    rows.extend(profiles.iter().map(|(n, p)| {
        (
            n.clone(),
            format!("{} {}", provider_name(p.provider), p.model),
        )
    }));
    for (i, (n, d)) in rows.iter().enumerate() {
        println!("{}) {n} - {d}", i + 1);
    }
    println!("{}) None", rows.len() + 1);
    print!("Select a customization: ");
    std::io::stdout().flush()?;
    let mut input = String::new();
    std::io::stdin().read_line(&mut input)?;
    match input.trim().parse::<usize>() {
        Ok(i) if (1..=rows.len()).contains(&i) => Ok(Some(rows[i - 1].0.clone())),
        Ok(i) if i == rows.len() + 1 => Ok(None),
        _ => bail!("No customization selected"),
    }
}

/// `inline show-customizations` output (with trailing newline).
fn render_sources(
    selected: &str,
    profiles: &[(String, std::result::Result<Profile, String>)],
    format: OutputFormat,
    color: bool,
) -> String {
    let plain = || {
        let block = |n: &str, d: &str| {
            let mark = if n == selected { " (selected)" } else { "" };
            format!("{}{mark}\n{d}\n\n", bold(n, color))
        };
        let mut out: String = BUILTIN_SOURCES.iter().map(|(n, d)| block(n, d)).collect();
        for (n, p) in profiles {
            out += &match p {
                Ok(p) => block(n, &format!("{} {}", provider_name(p.provider), p.model)),
                Err(reason) => format!("{} (invalid: {reason})\n\n", bold(n, color)),
            };
        }
        if profiles.is_empty() {
            out += &format!("No LLM profiles configured. Add one with:\n  {PROFILE_EXAMPLE}\n");
        }
        out
    };
    let json = || {
        let builtins = BUILTIN_SOURCES
            .iter()
            .map(|(n, d)| json!({"name": n, "kind": "builtin", "description": d, "selected": *n == selected}));
        let profiles = profiles.iter().map(|(n, p)| match p {
            Ok(p) => json!({
                "name": n, "kind": "profile",
                "description": format!("{} {}", provider_name(p.provider), p.model),
                "selected": n == selected,
                "provider": provider_name(p.provider), "model": p.model,
                "base_url": p.base_url, "api_key_env": p.api_key_env,
            }),
            Err(reason) => json!({
                "name": n, "kind": "profile", "description": format!("invalid: {reason}"), "selected": n == selected,
            }),
        });
        builtins.chain(profiles).collect()
    };
    match format {
        OutputFormat::Plain => plain(),
        f => f.render(String::new, json) + "\n",
    }
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

    // ponytail: shell = $SHELL (Kiro uses the parent process); unset on Windows → omitted.
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
// completion, debug (SPEC §2.10)

pub fn completion(args: CompletionArgs) -> Result<i32> {
    // Buffered: clap_complete panics on a write error (e.g. `| head`).
    let mut buf = Vec::new();
    clap_complete::generate(
        args.shell,
        &mut <crate::Cli as clap::CommandFactory>::command(),
        "tabsmith",
        &mut buf,
    );
    std::io::stdout().write_all(&buf)?;
    Ok(0)
}

pub fn debug(cmd: DebugCmd) -> Result<i32> {
    match cmd {
        DebugCmd::KeyTester => key_tester(),
        DebugCmd::Shell { shell } => debug_shell(shell.map(Into::into)),
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

/// `_ pty` with only the post integration loaded (Kiro `debug/mod.rs:816-870`).
fn debug_shell(shell: Option<Shell>) -> Result<i32> {
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
    let post = crate::integrations::init_script(shell, When::Post)? + "\n";
    let (dir, cmd) = debug_shell_files(&settings::home()?, shell, &exe, &post)?;
    if shell == Shell::Zsh {
        std::env::set_var("ZDOTDIR", &dir);
    }
    // The wrapper passes the current env to the child.
    std::env::set_var("TABSMITH_DEBUG_SHELL", "1");
    let _ = writeln!(std::io::stdout(), "Starting tabsmith debug shell\n");
    let code = crate::pty::run(Some(exe), cmd);
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
    let q = |s: &str| crate::integrations::quote(shell, s);
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
    let dir = settings::home()?.join("logs");
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

/// `error: <msg>` on stderr (`error:` bold red when stderr is a tty).
pub fn print_error(err: &anyhow::Error) {
    if std::io::stderr().is_terminal() {
        eprintln!("\x1b[1;31merror:\x1b[0m {err:#}");
    } else {
        eprintln!("error: {err:#}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn map(v: Value) -> Map<String, Value> {
        v.as_object().unwrap().clone()
    }

    fn tmp_dir(name: &str) -> std::path::PathBuf {
        let d =
            std::env::temp_dir().join(format!("tabsmith-cli-test-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn settings_set_parses_json_or_string() {
        assert_eq!(parse_value("true"), json!(true));
        assert_eq!(parse_value("5"), json!(5));
        assert_eq!(parse_value("[\"git\"]"), json!(["git"]));
        assert_eq!(parse_value("\"x\""), json!("x"));
        assert_eq!(parse_value("most recent"), json!("most recent"));
        assert_eq!(parse_value("{bad"), json!("{bad"));
    }

    #[test]
    fn settings_get_formats() {
        let s = json!("dark");
        let a = json!(["git", "npm"]);
        assert_eq!(
            render_get("k", Some(&s), OutputFormat::Plain).unwrap(),
            "dark"
        );
        assert_eq!(
            render_get("k", Some(&s), OutputFormat::Json).unwrap(),
            "\"dark\""
        );
        assert_eq!(
            render_get("k", Some(&a), OutputFormat::Json).unwrap(),
            "[\"git\",\"npm\"]"
        );
        assert_eq!(
            render_get("k", Some(&a), OutputFormat::Plain).unwrap(),
            "[\n  \"git\",\n  \"npm\"\n]"
        );
        assert_eq!(render_get("k", None, OutputFormat::Json).unwrap(), "null");
        assert_eq!(
            render_get("k", None, OutputFormat::Plain)
                .unwrap_err()
                .to_string(),
            "No value associated with k"
        );
    }

    #[test]
    fn settings_delete_wildcards() {
        let m = map(
            json!({"autocomplete.theme": "dark", "autocomplete.width": 40, "inline.enabled": true}),
        );
        assert_eq!(
            matching_keys(&m, "autocomplete.*"),
            ["autocomplete.theme", "autocomplete.width"]
        );
        assert_eq!(matching_keys(&m, "inline.enabled"), ["inline.enabled"]);
        assert!(matching_keys(&m, "nope*").is_empty());
        assert_eq!(removal_message(&["a.b".into()]), "Removing \"a.b\"");
        assert_eq!(
            removal_message(&["a".into(), "b".into()]),
            "Removing:\n  - a\n  - b"
        );
    }

    #[test]
    fn settings_list_formats() {
        let m = map(json!({"b.key": [1], "a.key": "x"}));
        assert_eq!(
            render_list(&m, OutputFormat::Plain),
            "a.key = \"x\"\nb.key = [1]"
        );
        assert_eq!(
            render_list(&m, OutputFormat::Json),
            r#"{"a.key":"x","b.key":[1]}"#
        );
        assert_eq!(render_list(&Map::new(), OutputFormat::Plain), "");

        let m = map(json!({"autocomplete.theme": "light"}));
        let all = render_list_all(&m, OutputFormat::Plain);
        let lines: Vec<&str> = all.lines().collect();
        assert_eq!(lines[0], "key\ttype\tdefault\tvalue");
        assert_eq!(lines.len(), settings::catalog().len() + 1);
        assert!(lines.contains(&"autocomplete.theme\tstring\t\"dark\"\t\"light\""));
        assert!(lines.contains(&"autocomplete.width\tnumber (columns)\t40\t"));

        let v: Value = serde_json::from_str(&render_list_all(&m, OutputFormat::Json)).unwrap();
        let theme = v
            .as_array()
            .unwrap()
            .iter()
            .find(|e| e["key"] == "autocomplete.theme")
            .unwrap();
        assert_eq!(theme["default"], json!("dark"));
        assert_eq!(theme["value"], json!("light"));
        assert!(theme["description"].is_string());
        let width = v
            .as_array()
            .unwrap()
            .iter()
            .find(|e| e["key"] == "autocomplete.width")
            .unwrap();
        assert_eq!(
            (width["default"].clone(), width["value"].clone()),
            (json!(40), Value::Null)
        );
    }

    #[test]
    fn theme_list_order() {
        let d = tmp_dir("themes");
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

    fn profile(name: &str) -> Profile {
        Profile {
            name: name.into(),
            provider: Provider::Anthropic,
            model: "claude-opus-5-5".into(),
            base_url: "https://api.anthropic.com".into(),
            api_key_env: "ANTHROPIC_API_KEY".into(),
            max_tokens: 1024,
            effort: None,
        }
    }

    #[test]
    fn inline_show_customizations() {
        let profiles = vec![
            ("claude".to_string(), Ok(profile("claude"))),
            ("bad".to_string(), Err("model missing".into())),
        ];
        let out = render_sources("claude", &profiles, OutputFormat::Plain, false);
        assert!(out.starts_with("history+specs\nLocal history, then completion specs\n\nhistory\n"));
        assert!(out.contains("claude (selected)\nanthropic claude-opus-5-5\n\n"));
        assert!(out.contains("bad (invalid: model missing)\n"));
        assert!(!out.contains(PROFILE_EXAMPLE));

        let out = render_sources("history+specs", &[], OutputFormat::Plain, false);
        assert!(out.starts_with("history+specs (selected)\n"));
        assert!(out.ends_with(&format!("{PROFILE_EXAMPLE}\n")));

        let v: Value = serde_json::from_str(&render_sources(
            "claude",
            &profiles,
            OutputFormat::Json,
            false,
        ))
        .unwrap();
        let rows = v.as_array().unwrap();
        assert_eq!(rows.len(), 5);
        assert_eq!(
            rows[0],
            json!({"name":"history+specs","kind":"builtin","description":"Local history, then completion specs","selected":false})
        );
        assert_eq!(rows[3]["selected"], json!(true));
        assert_eq!(rows[3]["provider"], json!("anthropic"));
        assert_eq!(rows[3]["api_key_env"], json!("ANTHROPIC_API_KEY"));
        assert_eq!(rows[4]["description"], json!("invalid: model missing"));
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
        let home = tmp_dir("o'brien");
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
            if crate::integrations::which(shell.name()).is_none() {
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
        use portable_pty::{native_pty_system, CommandBuilder, PtySize};
        const NAME: &str = "cli::tests::debug_shell_survives_closed_terminal";
        if let Some(marker) = std::env::var_os("TABSMITH_TEST_DEBUG_SHELL") {
            let code = debug_shell(Some(Shell::Bash));
            std::fs::write(marker, format!("{code:?}")).unwrap();
            return;
        }
        if crate::integrations::which("bash").is_none() {
            return;
        }
        let home = tmp_dir("dbgclose");
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
