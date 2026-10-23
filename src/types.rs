//! Types shared between the wrapper (main thread), the UI and the completion engine thread (SPEC §4.3).
use crate::settings::Settings;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Shell {
    Bash,
    Zsh,
    Fish,
    Pwsh,
}

impl Shell {
    pub const ALL: [Shell; 4] = [Shell::Bash, Shell::Zsh, Shell::Fish, Shell::Pwsh];

    pub fn name(self) -> &'static str {
        match self {
            Shell::Bash => "bash",
            Shell::Zsh => "zsh",
            Shell::Fish => "fish",
            Shell::Pwsh => "pwsh",
        }
    }

    /// Accepts a name or path: `zsh`, `/bin/zsh`, `-zsh` (login), `pwsh.exe`, `powershell`.
    pub fn from_name(s: &str) -> Option<Shell> {
        let base = s.rsplit(['/', '\\']).next().unwrap_or(s);
        let base = base.trim_start_matches('-');
        let base = base.strip_suffix(".exe").unwrap_or(base);
        let base = base.split_whitespace().next().unwrap_or(base); // "zsh (tabsmith)"
        Some(match base.to_ascii_lowercase().as_str() {
            "bash" => Shell::Bash,
            "zsh" => Shell::Zsh,
            "fish" => Shell::Fish,
            "pwsh" | "powershell" => Shell::Pwsh,
            _ => return None,
        })
    }
}

/// Suggestion row type (Fig `type`, plus internal `history` and `auto-execute`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ItemType {
    Subcommand,
    Option,
    Arg,
    File,
    Folder,
    Special,
    Shortcut,
    History,
    AutoExecute,
}

impl ItemType {
    /// Fig type string → type (`mixin` → Special, unknown → Arg).
    pub fn from_fig(s: &str) -> ItemType {
        match s {
            "subcommand" => ItemType::Subcommand,
            "option" => ItemType::Option,
            "file" => ItemType::File,
            "folder" => ItemType::Folder,
            "special" | "mixin" => ItemType::Special,
            "shortcut" => ItemType::Shortcut,
            "history" => ItemType::History,
            "auto-execute" => ItemType::AutoExecute,
            _ => ItemType::Arg,
        }
    }
}

/// Bytes-level description of an insertion (SPEC §6.8, §7.3). Counts are in chars.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Insertion {
    /// Chars to delete before the cursor (sent as 0x7f each).
    pub delete: usize,
    /// Text to type (no trailing "\n"; `{cursor}` already removed).
    pub text: String,
    /// Chars to move the cursor left after typing (`ESC [ D` each).
    pub cursor_back: usize,
    /// Send "\r" afterwards.
    pub execute: bool,
}

/// Identity of a generator (hash of arg identity + generator index).
pub type GenKey = u64;

/// One dropdown row, already filtered, ranked and with its insertion precomputed by the engine.
#[derive(Clone, Debug)]
pub struct Item {
    pub names: Vec<String>,
    pub display_name: Option<String>,
    pub description: Option<String>,
    pub kind: ItemType,
    /// For auto-execute rows: the type of the row it was derived from.
    pub original_kind: Option<ItemType>,
    pub priority: f64,
    pub is_dangerous: bool,
    /// e.g. `<file> [mode]...` (SPEC §7.4.2), empty if none.
    pub args_hint: String,
    /// Char indexes into `title()` to highlight as matching (SPEC §7.4.2).
    pub highlight: Vec<usize>,
    /// Query term this item was matched against (SPEC §6.7.3 `query(s)`).
    pub query: String,
    pub insert: Insertion,
    /// Generator that produced it, if any.
    pub generator: Option<GenKey>,
    /// Matched by prefix (not only fuzzy) — used by the spec ghost source (SPEC §6.11).
    pub prefix_match: bool,
    /// `context.templateType` of the generator result (`filepaths`/`folders`/`help`), if any.
    pub template_type: Option<String>,
}

impl Item {
    /// Display title: `display_name` or names joined by ", ".
    pub fn title(&self) -> String {
        self.display_name.clone().unwrap_or_else(|| self.names.join(", "))
    }

    pub fn first_name(&self) -> &str {
        self.names.first().map(String::as_str).unwrap_or("")
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct ArgInfo {
    pub name: String,
    pub description: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    /// First reply: static suggestions (+ stale generator results).
    Parse,
    /// Follow-up reply after triggered generators finished.
    Generators,
}

/// Engine → wrapper reply (SPEC §4.3).
#[derive(Clone, Debug)]
pub struct Completion {
    pub req: u64,
    pub phase: Phase,
    pub error: bool,
    /// The buffer/cursor the reply was computed from (for insertion reconciliation, SPEC §7.3).
    pub buffer: String,
    pub cursor: usize,
    /// Token texts of the current command (after alias expansion).
    pub tokens: Vec<String>,
    pub command_text: String,
    pub command_index: usize,
    /// Identity of (current arg, current subcommand) — changes when a new argument starts.
    pub arg_key: u64,
    pub current_arg: Option<ArgInfo>,
    pub search_term: String,
    pub triggered_generators: Vec<GenKey>,
    pub items: Vec<Item>,
    /// Some generator is still running.
    pub loading: bool,
    /// Local inline ghost text for this buffer (SPEC §6.11).
    pub ghost: Option<String>,
    /// Effective fuzzy mode for this reply.
    pub fuzzy: bool,
}

impl Completion {
    /// Error reply: no suggestions, menu error-reset.
    pub fn error(req: u64, buffer: String, cursor: usize) -> Completion {
        Completion {
            req,
            phase: Phase::Parse,
            error: true,
            buffer,
            cursor,
            tokens: Vec::new(),
            command_text: String::new(),
            command_index: 0,
            arg_key: 0,
            current_arg: None,
            search_term: String::new(),
            triggered_generators: Vec::new(),
            items: Vec::new(),
            loading: false,
            ghost: None,
            fuzzy: false,
        }
    }
}

/// Wrapper → engine completion request.
#[derive(Clone, Debug)]
pub struct Request {
    pub id: u64,
    pub buffer: String,
    /// Byte offset into `buffer`.
    pub cursor: usize,
    pub cwd: PathBuf,
    pub shell: Shell,
    pub history_mode: bool,
}

/// One line of `~/.tabsmith/history.jsonl` (SPEC §3.5).
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct HistoryEntry {
    pub command: String,
    pub shell: String,
    pub pid: Option<i32>,
    pub session_id: String,
    pub cwd: Option<String>,
    pub start_time: u64,
    pub end_time: Option<u64>,
    pub exit_code: Option<i32>,
    pub hostname: Option<String>,
}

/// `~/.tabsmith/run/<sid>.json`, written by `tabsmith _ pre-cmd` (SPEC §2.11, D17).
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct SessionCtx {
    #[serde(default)]
    pub env: HashMap<String, String>,
    #[serde(default)]
    pub alias: Option<String>,
    #[serde(default)]
    pub ts: u64,
}

impl SessionCtx {
    pub fn path(session_id: &str) -> anyhow::Result<PathBuf> {
        Ok(crate::settings::home()?.join("run").join(format!("{session_id}.json")))
    }

    pub fn load(path: &Path) -> Option<SessionCtx> {
        serde_json::from_str(&std::fs::read_to_string(path).ok()?).ok()
    }
}

/// Output of a generator process (`executeCommand` result shape).
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct ExecOutput {
    pub stdout: String,
    pub stderr: String,
    pub status: i32,
}

/// Messages to the wrapper's main loop.
pub enum Msg {
    Stdin(Vec<u8>),
    Pty(Vec<u8>),
    PtyEof,
    Resize(u16, u16),
    ChildExit(i32),
    Term,
    Completion(Box<Completion>),
    Ghost { req: u64, text: Option<String> },
}

/// Messages to the engine thread.
pub enum EngineMsg {
    Complete(Request),
    CommandFinished(HistoryEntry),
    /// A command finished but must not be recorded (NoHistory).
    CommandEnded,
    RecordInsert { cmd: String, name: String },
    Llm { req: u64, buffer: String, cwd: PathBuf, shell: Shell },
    Settings(Settings),
    Session(SessionCtx),
    ToggleFuzzy,
    /// Internal (js host): a spawned process finished.
    ExecDone { id: u64, out: Result<ExecOutput, String> },
    /// Internal (js host): a `setTimeout`/debounce timer fired.
    Timer { id: u64 },
    Shutdown,
}

pub fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

pub fn now_secs() -> u64 {
    now_ms() / 1000
}

/// The headless terminal model the wrapper feeds with PTY output (alacritty_terminal, SPEC D4).
pub type Model = alacritty_terminal::Term<alacritty_terminal::event::VoidListener>;
