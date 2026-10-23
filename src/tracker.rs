//! OSC 697 splitter, shell-integration marker tracker (cell tagging via injected OSC 8), edit-buffer and cursor
//! extraction from the model (SPEC §5.1, §5.6), and the filter for bytes sent to the real terminal (SPEC §7.5.4).
//! OWNER: wrapper agent.
use crate::types::{now_secs, HistoryEntry, Model, Shell};
use alacritty_terminal::grid::Dimensions;
use alacritty_terminal::index::{Column, Line};
use alacritty_terminal::term::cell::{Cell, Flags};
use alacritty_terminal::vte::ansi::{Color, Handler, Hyperlink, Processor};
use std::collections::VecDeque;
use std::path::PathBuf;

/// An unterminated OSC longer than this is flushed as plain bytes (SPEC §5.1.1).
const OSC_MAX: usize = 64 * 1024;
/// An unfinished CSI/ESC sequence longer than this passes through instead of being carried (no quadratic rescans).
const SEQ_MAX: usize = 4096;
const TAG_PROMPT: &str = "tabsmith:p";

// ---------------------------------------------------------------------------------------------------------------
// Splitter (SPEC §5.1.1)

#[derive(Debug, PartialEq)]
pub enum Piece {
    Bytes(Vec<u8>),
    /// OSC 697 payload (everything after `697;`, terminator stripped).
    Marker(String),
}

#[derive(Clone, Copy, Default, PartialEq)]
enum State {
    #[default]
    Ground,
    Esc,
    Osc,
    OscEsc,
}

/// Splits PTY output into plain bytes and OSC 697 markers at the exact byte offset; stateful across chunks.
#[derive(Default)]
pub struct Splitter {
    state: State,
    osc: Vec<u8>,
}

impl Splitter {
    pub fn feed(&mut self, data: &[u8]) -> Vec<Piece> {
        let mut pieces = Vec::new();
        let mut bytes = Vec::with_capacity(data.len());
        for &b in data {
            self.step(b, &mut bytes, &mut pieces);
        }
        if !bytes.is_empty() {
            pieces.push(Piece::Bytes(bytes));
        }
        pieces
    }

    fn step(&mut self, b: u8, bytes: &mut Vec<u8>, pieces: &mut Vec<Piece>) {
        match self.state {
            State::Ground if b == 0x1b => self.state = State::Esc,
            State::Ground => bytes.push(b),
            State::Esc if b == b']' => {
                self.state = State::Osc;
                self.osc.clear();
            }
            State::Esc => {
                bytes.push(0x1b);
                self.state = State::Ground;
                self.step(b, bytes, pieces);
            }
            State::Osc => match b {
                0x07 => self.finish(b"\x07", bytes, pieces),
                0x1b => self.state = State::OscEsc,
                _ => {
                    self.osc.push(b);
                    if self.osc.len() > OSC_MAX {
                        self.abandon(bytes);
                    }
                }
            },
            State::OscEsc if b == b'\\' => self.finish(b"\x1b\\", bytes, pieces),
            State::OscEsc => {
                // `ESC x` inside an OSC: give up on it and re-process the ESC.
                self.abandon(bytes);
                self.state = State::Esc;
                self.step(b, bytes, pieces);
            }
        }
    }

    /// Emit the collected OSC prefix as plain bytes.
    fn abandon(&mut self, bytes: &mut Vec<u8>) {
        bytes.extend_from_slice(b"\x1b]");
        bytes.append(&mut self.osc);
        self.state = State::Ground;
    }

    fn finish(&mut self, terminator: &[u8], bytes: &mut Vec<u8>, pieces: &mut Vec<Piece>) {
        self.state = State::Ground;
        if self.osc.starts_with(b"697;") {
            if !bytes.is_empty() {
                pieces.push(Piece::Bytes(std::mem::take(bytes)));
            }
            pieces.push(Piece::Marker(String::from_utf8_lossy(&self.osc[4..]).into_owned()));
            self.osc.clear();
        } else {
            bytes.extend_from_slice(b"\x1b]");
            bytes.append(&mut self.osc);
            bytes.extend_from_slice(terminator);
        }
    }
}

// ---------------------------------------------------------------------------------------------------------------
// Suggestion colours (SPEC §5.1.3)

/// A colour as an xterm-256 index; `None` = the terminal's default colour.
type Col = Option<u8>;

/// Colour the shell draws its own suggestions in. Any listed colour matches; an empty list matches every colour.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SuggestionColor {
    pub fg: Vec<Col>,
    pub bg: Vec<Col>,
}

impl SuggestionColor {
    /// zsh highlight style: `fg=8`, `fg=#aaaaaa,bg=…`, `fg=245,bold`, `fg=yellow`.
    pub fn parse_zsh(spec: &str) -> Option<SuggestionColor> {
        let mut c = SuggestionColor::default();
        for part in spec.split(',').map(str::trim) {
            if let Some(v) = part.strip_prefix("fg=") {
                c.fg.extend(parse_color(v, false));
            } else if let Some(v) = part.strip_prefix("bg=") {
                c.bg.extend(parse_color(v, false));
            }
        }
        (!c.fg.is_empty() || !c.bg.is_empty()).then_some(c)
    }

    /// fish `set_color` arguments: every non-option token is a colour (`555 brblack`, `--bold 666`).
    pub fn parse_fish(spec: &str) -> Option<SuggestionColor> {
        let mut c = SuggestionColor::default();
        let mut tokens = spec.split_whitespace();
        while let Some(t) = tokens.next() {
            if t == "-b" || t == "--background" {
                tokens.next(); // ponytail: background colours ignored (fish autosuggestions never set one)
            } else if !t.starts_with('-') {
                c.fg.extend(parse_color(t, true));
            }
        }
        (!c.fg.is_empty()).then_some(c)
    }

    pub fn matches(&self, cell: &Cell) -> bool {
        (self.fg.is_empty() || self.fg.contains(&cell_col(cell.fg)))
            && (self.bg.is_empty() || self.bg.contains(&cell_col(cell.bg)))
    }
}

/// One colour token: names (+ fish `br*` variants), zsh `0..255`, hex `#rgb`/`#rrggbb` (fish: `#` optional).
fn parse_color(v: &str, fish: bool) -> Option<Col> {
    const NAMES: [&str; 8] = ["black", "red", "green", "yellow", "blue", "magenta", "cyan", "white"];
    let v = v.trim().to_ascii_lowercase();
    if v == "default" || v == "normal" {
        return Some(None);
    }
    if let Some(i) = NAMES.iter().position(|n| *n == v) {
        return Some(Some(i as u8));
    }
    if let Some(i) = v.strip_prefix("br").and_then(|n| NAMES.iter().position(|x| *x == n)) {
        return Some(Some(i as u8 + 8));
    }
    if !fish {
        if let Ok(i) = v.parse::<u8>() {
            return Some(Some(i));
        }
    }
    let hex = match v.strip_prefix('#') {
        Some(h) => h,
        None if fish => v.as_str(),
        None => return None,
    };
    let digit = |i: usize, len: usize| u8::from_str_radix(hex.get(i * len..(i + 1) * len)?, 16).ok();
    let (r, g, b) = match hex.len() {
        3 => (digit(0, 1)? * 17, digit(1, 1)? * 17, digit(2, 1)? * 17),
        6 => (digit(0, 2)?, digit(1, 2)?, digit(2, 2)?),
        _ => return None,
    };
    Some(Some(rgb_to_256(r, g, b)))
}

fn cell_col(c: Color) -> Col {
    match c {
        Color::Indexed(i) => Some(i),
        Color::Spec(rgb) => Some(rgb_to_256(rgb.r, rgb.g, rgb.b)),
        Color::Named(n) => (n as usize).try_into().ok().filter(|i: &u8| *i < 16),
    }
}

/// Nearest xterm-256 index (6×6×6 cube or 24 greys).
fn rgb_to_256(r: u8, g: u8, b: u8) -> u8 {
    const LEVELS: [i32; 6] = [0, 95, 135, 175, 215, 255];
    let near = |v: u8| (0..6).min_by_key(|&i| (LEVELS[i] - v as i32).abs()).unwrap_or(0);
    let (ri, gi, bi) = (near(r), near(g), near(b));
    let dist = |x: i32, y: i32, z: i32| (x - r as i32).pow(2) + (y - g as i32).pow(2) + (z - b as i32).pow(2);
    let grey = ((r as i32 + g as i32 + b as i32) / 3 - 3).clamp(0, 230) / 10; // levels 8, 18, …, 238
    let gv = 8 + 10 * grey;
    if dist(gv, gv, gv) < dist(LEVELS[ri], LEVELS[gi], LEVELS[bi]) {
        232 + grey as u8
    } else {
        (16 + 36 * ri + 6 * gi + bi) as u8
    }
}

// ---------------------------------------------------------------------------------------------------------------
// Output filter (SPEC §7.5.4, §4.5)

/// Filters the bytes written to the real terminal: strips keyboard-protocol upgrades outside the alternate screen,
/// tracks DECSTBM (alacritty keeps its scroll region private) and holds back incomplete trailing escape sequences
/// and UTF-8 so an overlay frame appended to a batch can never split them.
pub struct OutFilter {
    carry: Vec<u8>,
    rows: usize,
    alt: bool,
    partial_region: bool,
    /// Inside a DCS/SOS/PM/APC string body (`ESC P`/`X`/`^`/`_` … ST): no frame may be appended.
    in_string: bool,
    /// Inside an over-long CSI/ESC sequence already passed through: the bytes that continue it (params/intermediates).
    long_seq: Option<std::ops::RangeInclusive<u8>>,
    /// Windows console without `DISABLE_NEWLINE_AUTO_RETURN`: bare LF → `ESC D`.
    pub lf_to_ind: bool,
}

impl OutFilter {
    pub fn new(rows: usize) -> OutFilter {
        OutFilter { carry: Vec::new(), rows, alt: false, partial_region: false, in_string: false, long_seq: None, lf_to_ind: false }
    }

    /// Terminal resized: real terminals and the model reset the scroll region.
    pub fn resize(&mut self, rows: usize) {
        self.rows = rows;
        self.partial_region = false;
    }

    /// A non-full scroll region is active (render guard, SPEC §7.5.1).
    pub fn scroll_region(&self) -> bool {
        self.partial_region
    }

    /// A DCS/APC/PM/SOS string or an over-long CSI/ESC sequence is open on the terminal (render guard: a frame
    /// would land inside it).
    pub fn in_string(&self) -> bool {
        self.in_string || self.long_seq.is_some()
    }

    pub fn feed(&mut self, bytes: &[u8], out: &mut Vec<u8>) {
        let mut data = std::mem::take(&mut self.carry);
        data.extend_from_slice(bytes);
        let mut i = 0;
        while i < data.len() {
            if self.in_string {
                // The body passes through uncarried (sixel can be megabytes). Like the VT parser, any ESC (ST is
                // `ESC \`) or CAN/SUB ends the string; the ESC is then handled as a normal sequence.
                let end = data[i..].iter().position(|b| matches!(b, 0x1b | 0x18 | 0x1a)).map_or(data.len(), |k| i + k);
                out.extend_from_slice(&data[i..end]);
                i = end;
                if i < data.len() {
                    self.in_string = false;
                    if data[i] != 0x1b {
                        out.push(data[i]);
                        i += 1;
                    }
                }
                continue;
            }
            if let Some(r) = &self.long_seq {
                let end = data[i..].iter().position(|b| !r.contains(b)).map(|k| i + k + 1);
                if end.is_some() {
                    self.long_seq = None;
                }
                let end = end.unwrap_or(data.len());
                out.extend_from_slice(&data[i..end]);
                i = end;
                continue;
            }
            let b = data[i];
            if b == 0x1b {
                // Final byte (inclusive end): CSI params/intermediates then a final; ESC intermediates then a final.
                let skip = |from: usize, r: std::ops::RangeInclusive<u8>| {
                    (from..data.len()).find(|&k| !r.contains(&data[k])).unwrap_or(data.len())
                };
                let j = match data.get(i + 1) {
                    None => data.len(),
                    Some(b'[') => skip(i + 2, 0x20..=0x3f),
                    Some(0x20..=0x2f) => skip(i + 1, 0x20..=0x2f),
                    Some(_) => i + 1,
                };
                if j >= data.len() {
                    if data.len() - i > SEQ_MAX {
                        // ponytail: no keyboard-protocol upgrade is this long, so it is never stripped.
                        self.long_seq = Some(if data[i + 1] == b'[' { 0x20..=0x3f } else { 0x20..=0x2f });
                        out.extend_from_slice(&data[i..]);
                        return self.hold_utf8(out);
                    }
                    self.carry = data[i..].to_vec();
                    return self.hold_utf8(out);
                }
                let seq = &data[i..=j];
                if seq == b"\x1bc" {
                    self.alt = false;
                    self.partial_region = false;
                }
                self.in_string = matches!(seq, [0x1b, b'P' | b'X' | b'^' | b'_']);
                if seq[1] != b'[' || self.keep_csi(&seq[2..seq.len() - 1], seq[seq.len() - 1]) {
                    out.extend_from_slice(seq);
                }
                i = j + 1;
                continue;
            }
            if b == b'\n' && self.lf_to_ind && (i == 0 || data[i - 1] != b'\r') {
                out.extend_from_slice(b"\x1bD");
            } else {
                out.push(b);
            }
            i += 1;
        }
        self.hold_utf8(out);
    }

    /// Track alt screen / DECSTBM; false = strip (keyboard-protocol upgrade outside the alternate screen).
    fn keep_csi(&mut self, body: &[u8], fin: u8) -> bool {
        let numeric = |s: &[u8]| s.iter().all(|c| c.is_ascii_digit() || *c == b';');
        let private = body.first() == Some(&b'?');
        if private && (fin == b'h' || fin == b'l') && body[1..].split(|c| *c == b';').any(|p| matches!(p, b"1049" | b"1047" | b"47"))
        {
            self.alt = fin == b'h';
        }
        if fin == b'r' && numeric(body) {
            let mut it = body.split(|c| *c == b';').map(|p| std::str::from_utf8(p).ok()?.parse::<usize>().ok());
            let top = it.next().flatten().filter(|t| *t > 0).unwrap_or(1);
            let bottom = it.next().flatten().filter(|b| *b > 0).unwrap_or(self.rows);
            if top < bottom {
                self.partial_region = top > 1 || bottom < self.rows;
            }
        }
        if self.alt {
            return true;
        }
        let rest = body.get(1..).unwrap_or_default();
        let strip = match (body.first(), fin) {
            (Some(b'?'), b'h' | b'l') => rest == b"9001",
            (Some(b'?'), b'u') => rest.is_empty(),
            (Some(b'=' | b'>' | b'<'), b'u') => numeric(rest),
            (Some(b'>'), b'm') => numeric(rest),
            _ => false,
        };
        !strip
    }

    /// Move an incomplete UTF-8 sequence at the end of `out` into the carry (prepended to the next batch).
    fn hold_utf8(&mut self, out: &mut Vec<u8>) {
        let n = out.len();
        for k in 1..=3.min(n) {
            let c = out[n - k];
            if c & 0xc0 == 0x80 {
                continue;
            }
            let need = match c {
                0xf0.. => 4,
                0xe0.. => 3,
                0xc0.. => 2,
                _ => 1,
            };
            if need > k {
                let mut tail = out.split_off(n - k);
                tail.append(&mut self.carry);
                self.carry = tail;
            }
            return;
        }
    }
}

// ---------------------------------------------------------------------------------------------------------------
// Tracker (SPEC §5.1, §5.1.2, §5.1.4)

#[derive(Clone, Copy, Debug, PartialEq)]
enum Deferred {
    EndPrompt,
    NewCmd,
}

/// What the wrapper must react to after a batch.
#[derive(Debug, PartialEq)]
pub enum Event {
    /// New prompt: menu error-reset, settings re-check; carries the finished command, if any (SPEC §3.5).
    NewCmd(Option<HistoryEntry>),
    /// New prompt after a command that must not be recorded (`NoHistory`): finished like `NewCmd(Some)`.
    CommandEnded,
    /// A command started: hide menu + ghost, cancel the insertion lock.
    PreExec,
}

/// Edit buffer and cursor read from the model (SPEC §5.6).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Extracted {
    pub buffer: String,
    /// Byte offset into `buffer`.
    pub cursor: usize,
    /// The shell already draws its own suggestion after the cursor (SPEC §8.1).
    pub shell_ghost: bool,
    /// The block around the cursor starts on a prompt row: false for an inline TUI a zle widget draws below the
    /// prompt (fzf `--height`), whose cells carry the open cycle's tag too.
    pub at_prompt: bool,
}

/// Shell-integration state of one wrapper session; owns the PTY output path (splitter → model → filter).
pub struct Tracker {
    sid: String,
    /// Windows ConPTY deferral of EndPrompt/NewCmd (SPEC §5.1.4); `cfg!(windows)`, switchable for tests.
    pub defer: bool,
    pub osc_lock: bool,
    pub has_seen_prompt: bool,
    pub preexec: bool,
    /// Current command cycle `n` (None before the first NewCmd).
    pub cycle: Option<u64>,
    pub cwd: Option<PathBuf>,
    pub shell: Option<Shell>,
    pub shell_name: Option<String>,
    pub shell_path: Option<String>,
    pub pid: Option<i32>,
    pub tty: Option<String>,
    pub user: Option<String>,
    pub zsh_color: Option<SuggestionColor>,
    pub fish_color: Option<SuggestionColor>,
    /// Drained by the wrapper after every batch.
    pub events: Vec<Event>,
    pub filter: OutFilter,
    /// Buffer the user submitted with Enter (set by the wrapper); preferred over a scrape that has shell output
    /// (`!!` echo, zsh correction prompt) after it. Cleared on NewCmd.
    pub submitted: Option<String>,
    /// The shell keeps the next command out of its history (`NoHistory` marker): tabsmith does too. Reset on NewCmd.
    pub no_history: bool,
    /// PSReadLine ListView predictions own the rows below the prompt (`PwshListView=1`/`0`, every pwsh prompt).
    /// Cleared when a command starts, not on NewCmd: the marker may precede NewCmd in the same prompt.
    pub pwsh_list_view: bool,
    /// PSReadLine `InlinePredictionColor` (colour + required DIM/ITALIC); None = PSReadLine's default dim/italic test.
    pwsh_color: Option<(SuggestionColor, Flags)>,
    /// PSReadLine ContinuationPrompt and PromptText(s): drawn after EndPrompt, so stripped from the buffer.
    pwsh_cont: String,
    pwsh_prompt_text: Vec<String>,
    /// Running commands, innermost last: nested integrated shells share the session id (finished by PID).
    pending: Vec<HistoryEntry>,
    exit_code: Option<i32>,
    deferred: VecDeque<Deferred>,
    batch_markers: usize,
    splitter: Splitter,
    proc: Processor,
}

impl Tracker {
    pub fn new(sid: String, shell: Option<Shell>, rows: usize) -> Tracker {
        Tracker {
            sid,
            defer: cfg!(windows),
            osc_lock: false,
            has_seen_prompt: false,
            preexec: false,
            cycle: None,
            cwd: None,
            shell,
            shell_name: shell.map(|s| s.name().to_string()),
            shell_path: None,
            pid: None,
            tty: None,
            user: None,
            zsh_color: None,
            fish_color: None,
            events: Vec::new(),
            filter: OutFilter::new(rows),
            submitted: None,
            no_history: false,
            pwsh_list_view: false,
            pwsh_color: None,
            pwsh_cont: ">> ".into(), // PSReadLine's default, until the prompt reports it
            pwsh_prompt_text: Vec::new(),
            pending: Vec::new(),
            exit_code: None,
            deferred: VecDeque::new(),
            batch_markers: 0,
            splitter: Splitter::default(),
            proc: Processor::new(),
        }
    }

    /// One PTY batch (SPEC §4.5): markers handled at their byte offset, everything else fed to the model and
    /// appended (filtered) to `out` for the real terminal.
    pub fn feed(&mut self, model: &mut Model, batch: &[u8], out: &mut Vec<u8>) {
        for piece in self.splitter.feed(batch) {
            match piece {
                Piece::Bytes(b) => {
                    self.advance(model, &b);
                    self.filter.feed(&b, out);
                }
                Piece::Marker(p) => self.on_marker(&p, model),
            }
        }
        // Windows: apply deferred markers once a batch brings no new ones (FIFO; Kiro pops LIFO).
        if self.batch_markers == 0 {
            self.flush_deferred(model);
        }
        self.batch_markers = 0;
    }

    /// Feed bytes to the model only. Synchronized updates are applied at once: the model mirrors terminal state.
    pub fn advance(&mut self, model: &mut Model, bytes: &[u8]) {
        self.proc.advance(model, bytes);
        if self.proc.sync_timeout().sync_timeout().is_some() {
            self.proc.stop_sync(model);
        }
    }

    /// At a prompt with a known command cycle (an edit buffer exists).
    pub fn at_prompt(&self) -> bool {
        self.cycle.is_some() && !self.preexec
    }

    pub fn on_marker(&mut self, payload: &str, model: &mut Model) {
        self.batch_markers += 1;
        let (key, val) = match payload.split_once('=') {
            Some((k, v)) => (k, Some(v)),
            None => (payload, None),
        };
        if val == Some("") {
            return; // empty values are ignored (Kiro)
        }
        if self.osc_lock && !matches!(key, "NewCmd" | "PreExec" | "NoHistory" | "OSCLock" | "OSCUnlock" | "User") {
            return;
        }
        let v = val.unwrap_or("").trim();
        match key {
            "StartPrompt" => {
                self.has_seen_prompt = true;
                tag(model, Some(TAG_PROMPT.to_string()));
            }
            "EndPrompt" if self.defer => self.deferred.push_back(Deferred::EndPrompt),
            "EndPrompt" => self.end_prompt(model),
            // A nested session's, or a bare one (any program output could forge it; our scripts always send the sid).
            "NewCmd" if val != Some(self.sid.as_str()) => {}
            "NewCmd" if self.defer => self.deferred.push_back(Deferred::NewCmd),
            "NewCmd" => self.new_cmd(model),
            "PreExec" if !self.preexec => self.pre_exec(model),
            "NoHistory" if !self.preexec => {
                self.flush_deferred(model); // a queued NewCmd (which resets the flag) belongs before it
                self.no_history = true;
            }
            "OSCLock" if v == self.sid => self.osc_lock = true,
            "OSCUnlock" if v == self.sid => self.osc_lock = false,
            "Dir" => self.cwd = Some(PathBuf::from(val.unwrap_or(""))),
            "ExitCode" => self.exit_code = v.parse().ok(),
            "Shell" => {
                self.shell = Shell::from_name(v);
                self.shell_name = Some(v.to_string());
            }
            "ShellPath" => self.shell_path = Some(v.to_string()),
            "PID" => self.pid = v.parse().ok(),
            "TTY" => self.tty = Some(v.to_string()),
            "User" => self.user = Some(v.to_string()),
            "ZshAutosuggestionColor" => self.zsh_color = SuggestionColor::parse_zsh(v),
            "FishSuggestionColor" => self.fish_color = SuggestionColor::parse_fish(v),
            "PwshPredictionColor" => self.pwsh_color = parse_sgr(v),
            "PwshListView" => self.pwsh_list_view = v == "1",
            "PwshPrompts" => {
                // `<ContinuationPrompt>\x1f<PromptText>…` (untrimmed: the prompts end in spaces).
                let mut it = val.unwrap_or("").split('\x1f').map(String::from);
                self.pwsh_cont = it.next().unwrap_or_default();
                self.pwsh_prompt_text = it.filter(|p| !p.is_empty()).collect();
            }
            _ => {}
        }
    }

    fn end_prompt(&mut self, model: &mut Model) {
        let open = self.cycle.filter(|_| !self.preexec);
        tag(model, open.map(|n| format!("tabsmith:c{n}")));
    }

    fn new_cmd(&mut self, model: &mut Model) {
        let redraw = self.cycle.filter(|_| !self.preexec);
        let n = self.cycle.map_or(1, |n| n + 1);
        self.cycle = Some(n);
        self.preexec = false;
        self.no_history = false;
        tag(model, Some(format!("tabsmith:c{n}")));
        // A redraw of the prompt alone keeps the line's cells after it (bash readline: CUF over a vi-mode switch,
        // DCH at the end of an i-search): they are this cycle's text now.
        if let Some(o) = redraw {
            retag_redraw(model, &format!("tabsmith:c{o}"));
        }
        // The prompt finishes this shell's command (and those of nested shells that exited under it); a nested
        // shell's first prompt leaves the outer command running.
        let exit = self.exit_code.take();
        let Some(k) = self.pending.iter().rposition(|p| p.pid == self.pid) else {
            return self.events.push(Event::NewCmd(None));
        };
        self.submitted = None; // not on a redraw: a transient prompt redraws between Enter and PreExec
        for mut h in self.pending.drain(k..).rev() {
            h.exit_code = exit;
            h.end_time = Some(now_secs());
            // ponytail: an empty command (NoHistory, or nothing scraped) is the "do not record" mark.
            self.events.push(if h.command.is_empty() { Event::CommandEnded } else { Event::NewCmd(Some(h)) });
        }
    }

    fn pre_exec(&mut self, model: &mut Model) {
        self.flush_deferred(model); // a queued NewCmd belongs before this PreExec
        let scraped = self.extract(model).map(|e| e.buffer.trim().to_string()).unwrap_or_default();
        // Lines the shell printed after Enter (`!!` echo, correction prompt) are not part of the command, nor is a
        // transient prompt redrawn without markers before it; a line the shell rewrote (abbreviation, PS2 loop) is
        // taken from the screen.
        let then_output = |s: &str| {
            scraped.match_indices(s).any(|(i, _)| {
                let rest = scraped[i + s.len()..].trim_start_matches(' ');
                (i == 0 || scraped[..i].ends_with(' ')) && (rest.is_empty() || rest.starts_with('\n'))
            })
        };
        let command = match self.submitted.take().map(|s| s.trim().to_string()) {
            Some(s) if !s.is_empty() && then_output(&s) => s,
            _ => scraped,
        };
        // Still pending (its prompt finishes it, SPEC §3.5), but blank: not recorded.
        let command = if std::mem::take(&mut self.no_history) { String::new() } else { command };
        self.pending.push(HistoryEntry {
            command,
            shell: self.shell_name.clone().unwrap_or_default(),
            pid: self.pid,
            session_id: self.sid.clone(),
            cwd: self.cwd.as_ref().map(|p| p.display().to_string()),
            start_time: now_secs(),
            ..Default::default()
        });
        self.preexec = true;
        self.pwsh_list_view = false; // a nested shell does not inherit it; the next pwsh prompt re-reports it
        tag(model, None);
        self.events.push(Event::PreExec);
    }

    fn flush_deferred(&mut self, model: &mut Model) {
        while let Some(d) = self.deferred.pop_front() {
            match d {
                Deferred::EndPrompt => self.end_prompt(model),
                Deferred::NewCmd => self.new_cmd(model),
            }
        }
    }

    /// Suggestion-coloured cell (SPEC §5.1.3; pwsh: the reported prediction colour, else dim or italic).
    fn suggestion(&self, cell: &Cell) -> bool {
        match self.shell {
            Some(Shell::Zsh) => self.zsh_color.as_ref().is_some_and(|c| c.matches(cell)),
            Some(Shell::Fish) => self.fish_color.as_ref().is_some_and(|c| c.matches(cell)),
            Some(Shell::Pwsh) => match &self.pwsh_color {
                Some((c, f)) => c.matches(cell) && cell.flags.contains(*f),
                None => cell.flags.intersects(Flags::DIM | Flags::ITALIC),
            },
            _ => false,
        }
    }

    /// Edit buffer + cursor from the tagged cells around the cursor (SPEC §5.6). None while no cycle is open.
    pub fn extract(&self, model: &Model) -> Option<Extracted> {
        let n = self.cycle.filter(|_| !self.preexec)?;
        let tag = format!("tabsmith:c{n}");
        let grid = model.grid();
        let cols = grid.columns();
        let cur = &grid.cursor;
        // Pending wrap = the cursor sits past the last column.
        let (cl, cc) = (cur.point.line.0, if cur.input_needs_wrap { cols } else { cur.point.column.0 });
        let row = |r: i32| &grid[Line(r)][..Column(cols)];
        let has = |c: &Cell, t: &str| c.hyperlink().is_some_and(|h| h.uri() == t);
        let wraps = |r: i32| row(r)[cols - 1].flags.contains(Flags::WRAPLINE);
        let tagged = |r: i32| row(r).iter().any(|c| has(c, &tag));

        let mut start = cl;
        while start > grid.topmost_line().0 && cl - start < 100 && (wraps(start - 1) || tagged(start - 1)) {
            start -= 1;
        }
        // Below the cursor only wrap continuations and PS2 rows count (not zsh completion listings, not PSReadLine's
        // ListView, whose padded rows can look like wrap continuations).
        let pwsh = self.shell == Some(Shell::Pwsh);
        let mut end = cl;
        while !(pwsh && self.pwsh_list_view)
            && end < grid.bottommost_line().0
            && (wraps(end) || (has(&row(end + 1)[0], TAG_PROMPT) && tagged(end + 1)))
        {
            end += 1;
        }

        // ponytail: an empty PS1, or a prompt more than 100 rows up, reads as not at the prompt; recording the
        // NewCmd row with scroll tracking (Kiro's cmd_cursor) is the upgrade. A widget TUI drawn after typed text
        // (`vim ` + Ctrl-T) still joins the prompt row's block.
        let mut e = Extracted { at_prompt: row(start).iter().any(|c| has(c, TAG_PROMPT)), ..Extracted::default() };
        let mut cursor = None;
        let (mut pad, mut line_has_cmd) = (0, false);
        // Blanks between command cells and the cursor are buffer text (zsh skips them with CUF on a redraw).
        let mut at_cursor = |e: &mut Extracted, pad: &mut usize, line_has_cmd: bool| {
            if line_has_cmd {
                e.buffer.extend(std::iter::repeat_n(' ', *pad));
                *pad = 0;
            }
            cursor = Some(e.buffer.len());
        };
        for r in start..=end {
            if r == start || !wraps(r - 1) {
                // ponytail: untagged cells before the first command cell of a line (prompt) are not padding.
                pad = 0;
                line_has_cmd = false;
            }
            let mut row_has_cmd = false;
            for (c, cell) in row(r).iter().enumerate() {
                if (r, c) == (cl, cc) {
                    at_cursor(&mut e, &mut pad, line_has_cmd);
                }
                if cell.flags.intersects(Flags::WIDE_CHAR_SPACER | Flags::LEADING_WIDE_CHAR_SPACER) {
                    continue;
                }
                let after = (r, c) >= (cl, cc);
                let cmd = has(cell, &tag);
                if cmd && !(after && self.suggestion(cell)) {
                    if line_has_cmd {
                        e.buffer.extend(std::iter::repeat_n(' ', pad));
                    }
                    pad = 0;
                    line_has_cmd = true;
                    row_has_cmd = true;
                    e.buffer.push(cell.c);
                    e.buffer.extend(cell.zerowidth().into_iter().flatten());
                } else {
                    pad += 1;
                    e.shell_ghost |= cmd;
                }
            }
            if r == cl && cc >= cols {
                at_cursor(&mut e, &mut pad, line_has_cmd);
            }
            if r < end && !wraps(r) && (row_has_cmd || !e.buffer.is_empty()) {
                e.buffer.push('\n');
            }
        }
        e.cursor = cursor.unwrap_or(e.buffer.len());
        if pwsh {
            self.strip_pwsh_prompts(&mut e);
        }
        // Blank command cells after the cursor are drawing leftovers (Kiro trims them too).
        let keep = e.buffer.trim_end().len().max(e.cursor);
        e.buffer.truncate(keep);
        Some(e)
    }

    /// PSReadLine draws its ContinuationPrompt (each line after a real `\n`) and a re-rendered PromptText (first
    /// line) after EndPrompt, so they carry the command tag: drop them, keeping the cursor on the same character.
    /// ponytail: a first line the user typed starting with the PromptText loses it too.
    fn strip_pwsh_prompts(&self, e: &mut Extracted) {
        let (mut out, mut off, mut shift) = (String::with_capacity(e.buffer.len()), 0, 0);
        for (i, line) in e.buffer.split('\n').enumerate() {
            let p = if i == 0 {
                self.pwsh_prompt_text.iter().find(|p| line.starts_with(p.as_str())).map_or(0, |p| p.len())
            } else if !self.pwsh_cont.is_empty() && line.starts_with(&self.pwsh_cont) {
                self.pwsh_cont.len()
            } else {
                0
            };
            if i > 0 {
                out.push('\n');
            }
            out.push_str(&line[p..]);
            shift += p.min(e.cursor.saturating_sub(off));
            off += line.len() + 1;
        }
        e.buffer = out;
        e.cursor -= shift;
    }
}

/// SGR parameters of a pwsh `InlinePredictionColor` (ESC stripped: `[97;2;3m`, `38;5;8`, `[38;2;r;g;bm[3m`): its
/// colours plus the DIM/ITALIC a prediction cell must carry. None when it sets neither.
fn parse_sgr(spec: &str) -> Option<(SuggestionColor, Flags)> {
    let p: Vec<u16> = spec.split([';', ':', '[', 'm']).filter_map(|s| s.trim().parse().ok()).collect();
    let byte = |k: usize| p.get(k).and_then(|&n| u8::try_from(n).ok());
    let (mut c, mut flags, mut i) = (SuggestionColor::default(), Flags::empty(), 0);
    while i < p.len() {
        // 38/48;5;n and 38/48;2;r;g;b: (colour, parameters used).
        let ext = match p.get(i + 1) {
            Some(5) => (byte(i + 2), 3),
            Some(2) => ((|| Some(rgb_to_256(byte(i + 2)?, byte(i + 3)?, byte(i + 4)?)))(), 5),
            _ => (None, 1),
        };
        let mut step = 1;
        match p[i] {
            0 => (c, flags) = (SuggestionColor::default(), Flags::empty()),
            2 => flags |= Flags::DIM,
            3 => flags |= Flags::ITALIC,
            n @ 30..=37 => c.fg = vec![Some(n as u8 - 30)],
            n @ 90..=97 => c.fg = vec![Some(n as u8 - 90 + 8)],
            39 => c.fg = vec![None],
            n @ 40..=47 => c.bg = vec![Some(n as u8 - 40)],
            n @ 100..=107 => c.bg = vec![Some(n as u8 - 100 + 8)],
            49 => c.bg = vec![None],
            n @ (38 | 48) => {
                step = ext.1;
                if let Some(col) = ext.0 {
                    *(if n == 38 { &mut c.fg } else { &mut c.bg }) = vec![Some(col)];
                }
            }
            _ => {}
        }
        i += step;
    }
    (!c.fg.is_empty() || !c.bg.is_empty() || !flags.is_empty()).then_some((c, flags))
}

/// Inject a cell tag into the model only (OSC 8 hyperlink on the cursor template; SPEC D3).
fn tag(model: &mut Model, uri: Option<String>) {
    model.set_hyperlink(uri.map(|uri| Hyperlink { id: None, uri }));
}

/// A redraw at the cursor: cells tagged `from` at and after it (its line and wrap continuations) get the cursor
/// template's tag; before it on its row they are prompt (readline redraws only a multi-line prompt's last line,
/// without its StartPrompt).
fn retag_redraw(model: &mut Model, from: &str) {
    let prompt = Some(alacritty_terminal::term::cell::Hyperlink::new(None::<String>, TAG_PROMPT.into()));
    let link = model.grid().cursor.template.hyperlink();
    let (cols, bottom) = (model.grid().columns(), model.grid().bottommost_line());
    let (mut line, mut split) = (model.grid().cursor.point.line, model.grid().cursor.point.column.0);
    loop {
        let row = &mut model.grid_mut()[line];
        for c in 0..cols {
            let cell = &mut row[Column(c)];
            if cell.hyperlink().is_some_and(|h| h.uri() == from) {
                cell.set_hyperlink(if c < split { prompt.clone() } else { link.clone() });
            }
        }
        if line >= bottom || !row[Column(cols - 1)].flags.contains(Flags::WRAPLINE) {
            break;
        }
        (line, split) = (line + 1, 0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alacritty_terminal::event::VoidListener;
    use alacritty_terminal::term::test::TermSize;
    use alacritty_terminal::term::{Config, Term};

    const SID: &str = "0123456789abcdef0123456789abcdef";
    const SP: &str = "\x1b]697;StartPrompt\x07";
    const EP: &str = "\x1b]697;EndPrompt\x07";

    fn nc() -> String {
        format!("\x1b]697;NewCmd={SID}\x07")
    }

    fn osc(p: &str) -> String {
        format!("\x1b]697;{p}\x1b\\")
    }

    fn setup(cols: usize, rows: usize, shell: Shell) -> (Tracker, Model) {
        let mut t = Tracker::new(SID.into(), Some(shell), rows);
        t.defer = false;
        (t, Term::new(Config::default(), &TermSize::new(cols, rows), VoidListener))
    }

    fn feed(t: &mut Tracker, m: &mut Model, s: &str) -> Vec<u8> {
        let mut out = Vec::new();
        t.feed(m, s.as_bytes(), &mut out);
        out
    }

    fn buf(t: &Tracker, m: &Model) -> (String, usize) {
        let e = t.extract(m).expect("at prompt");
        (e.buffer, e.cursor)
    }

    fn prompt(p: &str) -> String {
        format!("{SP}{p}{EP}{}", nc())
    }

    /// A precmd's markers: unlock, then `extra` markers, then the prompt.
    fn precmd(extra: &[&str], p: &str) -> String {
        let markers: String = extra.iter().map(|m| osc(m)).collect();
        format!("{}{markers}{}", osc(&format!("OSCUnlock={SID}")), prompt(p))
    }

    fn split_all(chunks: &[&[u8]]) -> Vec<Piece> {
        let mut s = Splitter::default();
        chunks.iter().flat_map(|c| s.feed(c)).collect()
    }

    fn bytes_of(pieces: &[Piece]) -> Vec<u8> {
        pieces.iter().flat_map(|p| if let Piece::Bytes(b) = p { b.clone() } else { vec![] }).collect()
    }

    fn markers_of(pieces: &[Piece]) -> Vec<String> {
        pieces.iter().filter_map(|p| if let Piece::Marker(m) = p { Some(m.clone()) } else { None }).collect()
    }

    #[test]
    fn splitter_chunk_boundaries() {
        let whole = b"ab\x1b]697;Dir=/a;b\x07cd";
        for cut in 0..=whole.len() {
            let p = split_all(&[&whole[..cut], &whole[cut..]]);
            assert_eq!(bytes_of(&p), b"abcd", "cut {cut}");
            assert_eq!(markers_of(&p), ["Dir=/a;b"], "cut {cut}");
        }
        // Marker sits at the exact byte offset.
        let p = split_all(&[b"x\x1b]697;NewCmd\x07y"]);
        assert_eq!(p, [Piece::Bytes(b"x".to_vec()), Piece::Marker("NewCmd".into()), Piece::Bytes(b"y".to_vec())]);
    }

    #[test]
    fn splitter_bel_st_and_passthrough() {
        let p = split_all(&[b"\x1b]697;A\x07\x1b]697;B\x1b\\"]);
        assert_eq!(markers_of(&p), ["A", "B"]);
        // Non-697 OSCs and other escapes pass through byte-exact (also split across chunks).
        let raw: &[u8] = b"\x1b]0;title\x07\x1b]8;;http://x\x1b\\l\x1b]8;;\x1b\\\x1b[31m";
        assert_eq!(bytes_of(&split_all(&[raw])), raw);
        assert_eq!(bytes_of(&split_all(&[&raw[..3], &raw[3..12], &raw[12..]])), raw);
        // ESC + other byte inside an OSC abandons it and re-processes the ESC.
        let p = split_all(&[b"\x1b]697;Di\x1b[1m\x1b]697;X\x07"]);
        assert_eq!(bytes_of(&p), b"\x1b]697;Di\x1b[1m");
        assert_eq!(markers_of(&p), ["X"]);
        // Trailing lone ESC is held until the next chunk.
        let mut s = Splitter::default();
        assert!(s.feed(b"a\x1b").iter().all(|p| p == &Piece::Bytes(b"a".to_vec())));
        assert_eq!(s.feed(b"[m"), [Piece::Bytes(b"\x1b[m".to_vec())]);
    }

    #[test]
    fn splitter_flushes_huge_osc() {
        let mut s = Splitter::default();
        let mut big = b"\x1b]697;".to_vec();
        big.extend(std::iter::repeat_n(b'x', OSC_MAX + 10));
        big.push(0x07);
        let p = s.feed(&big);
        assert!(markers_of(&p).is_empty());
        assert_eq!(bytes_of(&p), big);
    }

    #[test]
    fn markers_stripped_from_output() {
        let (mut t, mut m) = setup(40, 5, Shell::Zsh);
        let out = feed(&mut t, &mut m, &format!("{}\x1b]0;t\x07{}x", osc("Dir=/tmp"), prompt("% ")));
        assert_eq!(out, b"\x1b]0;t\x07% x");
        assert_eq!(t.cwd, Some(PathBuf::from("/tmp")));
        assert_eq!(buf(&t, &m), ("x".into(), 1));
    }

    #[test]
    fn lock_and_foreign_session_ids() {
        let (mut t, mut m) = setup(40, 5, Shell::Zsh);
        feed(&mut t, &mut m, &format!("{}{}", osc("OSCLock=other"), osc("Dir=/a")));
        assert!(!t.osc_lock);
        assert_eq!(t.cwd, Some(PathBuf::from("/a")));
        feed(&mut t, &mut m, &format!("{}{}{}", osc(&format!("OSCLock={SID}")), osc("Dir=/b"), osc("OSCUnlock=other")));
        assert!(t.osc_lock);
        assert_eq!(t.cwd, Some(PathBuf::from("/a")));
        feed(&mut t, &mut m, &format!("{}{}", osc(&format!("OSCUnlock={SID}")), osc("Dir=/c")));
        assert!(!t.osc_lock);
        assert_eq!(t.cwd, Some(PathBuf::from("/c")));
        // Empty values are ignored.
        feed(&mut t, &mut m, &osc("Dir="));
        assert_eq!(t.cwd, Some(PathBuf::from("/c")));
        // NewCmd from a nested session, or without a sid, is ignored; only ours opens a cycle.
        feed(&mut t, &mut m, &osc("NewCmd=ffff"));
        assert_eq!(t.cycle, None);
        feed(&mut t, &mut m, &osc("NewCmd"));
        assert_eq!(t.cycle, None);
        feed(&mut t, &mut m, &nc());
        assert_eq!(t.cycle, Some(1));
    }

    #[test]
    fn bare_newcmd_cannot_fake_a_prompt_while_locked() {
        let (mut t, mut m) = setup(40, 5, Shell::Bash);
        feed(&mut t, &mut m, &prompt("$ "));
        feed(&mut t, &mut m, &format!("cat x\r\n{}{}", osc(&format!("OSCLock={SID}")), osc("PreExec")));
        assert!(t.preexec);
        // `cat` of untrusted output prints a bare NewCmd: the command keeps running.
        feed(&mut t, &mut m, "\x1b]697;NewCmd\x07");
        assert!(t.preexec && !t.at_prompt());
        assert_eq!(t.cycle, Some(1));
    }

    #[test]
    fn command_cycle_produces_history() {
        let (mut t, mut m) = setup(40, 6, Shell::Zsh);
        feed(&mut t, &mut m, &format!("{}{}{}", osc("Shell=zsh"), osc("PID=42"), prompt("% ")));
        assert_eq!(t.events, [Event::NewCmd(None)]);
        t.events.clear();
        feed(&mut t, &mut m, "ls -l");
        feed(&mut t, &mut m, &format!("\r\n{}{}file\r\n", osc(&format!("OSCLock={SID}")), osc("PreExec")));
        assert_eq!(t.events, [Event::PreExec]);
        assert!(t.extract(&m).is_none());
        t.events.clear();
        feed(&mut t, &mut m, &format!("{}{}{}", osc(&format!("OSCUnlock={SID}")), osc("ExitCode=2"), prompt("% ")));
        let [Event::NewCmd(Some(h))] = &t.events[..] else { panic!("{:?}", t.events) };
        assert_eq!((h.command.as_str(), h.exit_code, h.pid, h.shell.as_str()), ("ls -l", Some(2), Some(42), "zsh"));
        assert_eq!(h.session_id, SID);
        assert_eq!(buf(&t, &m), (String::new(), 0));
    }

    #[test]
    fn extract_zsh_prompt_and_command() {
        let (mut t, mut m) = setup(40, 5, Shell::Zsh);
        feed(&mut t, &mut m, &format!("{}git st", prompt("~/src % ")));
        assert_eq!(buf(&t, &m), ("git st".into(), 6));
        // Cursor moved left: byte offset of the cursor.
        feed(&mut t, &mut m, "\x1b[2D");
        assert_eq!(buf(&t, &m), ("git st".into(), 4));
    }

    #[test]
    fn extract_ignores_rprompt() {
        let (mut t, mut m) = setup(40, 5, Shell::Zsh);
        feed(&mut t, &mut m, &format!("{}\x1b[35G{SP}[rp]{EP}\x1b[3Gls", prompt("% ")));
        assert_eq!(buf(&t, &m), ("ls".into(), 2));
    }

    #[test]
    fn extract_ps2_multiline() {
        let (mut t, mut m) = setup(40, 5, Shell::Zsh);
        feed(&mut t, &mut m, &format!("{}echo 'a\r\n{SP}> {EP}b", prompt("% ")));
        assert_eq!(buf(&t, &m), ("echo 'a\nb".into(), 9));
        // Cursor back on the first line: the PS2 row below still counts.
        feed(&mut t, &mut m, "\x1b[A\x1b[4G");
        assert_eq!(buf(&t, &m), ("echo 'a\nb".into(), 1));
    }

    #[test]
    fn extract_wrapped_long_command() {
        let (mut t, mut m) = setup(20, 5, Shell::Bash);
        let cmd = "abcdefghijklmnopqrstuvwxyz0123456789";
        feed(&mut t, &mut m, &format!("{}{cmd}", prompt("$ ")));
        assert_eq!(buf(&t, &m), (cmd.into(), cmd.len()));
        // Exactly at the right margin (pending wrap) the cursor is at the end.
        let (mut t, mut m) = setup(20, 5, Shell::Bash);
        feed(&mut t, &mut m, &format!("{}{}", prompt("$ "), &cmd[..18]));
        assert_eq!(buf(&t, &m), (cmd[..18].into(), 18));
        // Reflow on resize keeps the buffer.
        let (mut t, mut m) = setup(20, 5, Shell::Bash);
        feed(&mut t, &mut m, &format!("{}{cmd}", prompt("$ ")));
        m.resize(TermSize::new(15, 5));
        assert_eq!(buf(&t, &m), (cmd.into(), cmd.len()));
    }

    #[test]
    fn extract_excludes_completion_listing_below() {
        let (mut t, mut m) = setup(40, 6, Shell::Zsh);
        feed(&mut t, &mut m, &format!("{}git ch\r\ncheckout  cherry-pick\r\ncherry\x1b[2A\r\x1b[8C", prompt("% ")));
        assert_eq!(buf(&t, &m), ("git ch".into(), 6));
    }

    #[test]
    fn extract_ghost_after_cursor_vs_typed_grey() {
        let (mut t, mut m) = setup(40, 5, Shell::Zsh);
        feed(&mut t, &mut m, &format!("{}{}git st\x1b[90matus\x1b[0m\x1b[4D", osc("ZshAutosuggestionColor=fg=8"), prompt("% ")));
        let e = t.extract(&m).unwrap();
        assert_eq!((e.buffer.as_str(), e.cursor, e.shell_ghost), ("git st", 6, true));

        let (mut t, mut m) = setup(40, 5, Shell::Zsh);
        feed(&mut t, &mut m, &format!("{}{}\x1b[38;5;8mgit\x1b[0m st", osc("ZshAutosuggestionColor=fg=8"), prompt("% ")));
        let e = t.extract(&m).unwrap();
        assert_eq!((e.buffer.as_str(), e.cursor, e.shell_ghost), ("git st", 6, false));

        // pwsh: PSReadLine predictions are dim/italic.
        let (mut t, mut m) = setup(40, 5, Shell::Pwsh);
        feed(&mut t, &mut m, &format!("{}ls\x1b[3m -Force\x1b[0m\x1b[7D", prompt("PS> ")));
        let e = t.extract(&m).unwrap();
        assert_eq!((e.buffer.as_str(), e.cursor, e.shell_ghost), ("ls", 2, true));
    }

    #[test]
    fn extract_keeps_blank_skipped_before_cursor() {
        // zsh redraw (Ctrl-L, reset-prompt) moves over the trailing blank with CUF instead of writing it.
        let (mut t, mut m) = setup(40, 5, Shell::Zsh);
        feed(&mut t, &mut m, &format!("{}git \r{}git\x1b[K\x1b[1C", prompt("% "), prompt("% ")));
        assert_eq!(buf(&t, &m), ("git ".into(), 4));
        // Blanks after the prompt alone are not buffer text; blanks after the cursor are still trimmed.
        feed(&mut t, &mut m, &format!("\r{}\x1b[K\x1b[2C", prompt("% ")));
        assert_eq!(buf(&t, &m), (String::new(), 0));
        feed(&mut t, &mut m, &format!("\r{}ls -l\x1b[2D", prompt("% ")));
        assert_eq!(buf(&t, &m), ("ls -l".into(), 3));
    }

    #[test]
    fn pwsh_reported_prediction_colour() {
        // A custom InlinePredictionColor (plain grey, neither dim nor italic) is excluded after the cursor.
        let (mut t, mut m) = setup(40, 5, Shell::Pwsh);
        let grey = "git ch\x1b[90meckout main\x1b[0m\x1b[11D";
        feed(&mut t, &mut m, &format!("{}{}{grey}", osc("PwshPredictionColor=[90m"), prompt("PS> ")));
        let e = t.extract(&m).unwrap();
        assert_eq!((e.buffer.as_str(), e.cursor, e.shell_ghost), ("git ch", 6, true));
        // The default colour, reported: bright white + dim + italic; a plain bright white cell is typed text.
        feed(&mut t, &mut m, &format!("\r\x1b[K{}{}ls\x1b[97;2;3m -Force\x1b[0;97m x\x1b[0m\x1b[9D", osc("PwshPredictionColor=[97;2;3m"), prompt("PS> ")));
        let e = t.extract(&m).unwrap();
        assert_eq!((e.buffer.as_str(), e.cursor), ("ls        x", 2));
        let p = |s: &str| parse_sgr(s).map(|(c, f)| (c.fg, c.bg, f));
        assert_eq!(p("[97;2;3m"), Some((vec![Some(15)], vec![], Flags::DIM | Flags::ITALIC)));
        assert_eq!(p("38;5;238;48;2;0;0;0"), Some((vec![Some(238)], vec![Some(16)], Flags::empty())));
        assert_eq!(p("[38;2;170;170;170m[3m"), Some((vec![Some(rgb_to_256(170, 170, 170))], vec![], Flags::ITALIC)));
        assert_eq!(p("[39;49m"), Some((vec![None], vec![None], Flags::empty())));
        assert_eq!((p("[0m"), p("[1;4m"), p("x")), (None, None, None));
    }

    #[test]
    fn no_history_marker_drops_the_command() {
        let (mut t, mut m) = setup(40, 6, Shell::Fish);
        let run = |t: &mut Tracker, m: &mut Model, cmd: &str, private: bool| {
            let nh = if private { osc("NoHistory") } else { String::new() };
            feed(t, m, &format!("{cmd}\r\n{}{nh}{}out\r\n{}", osc(&format!("OSCLock={SID}")), osc("PreExec"), precmd(&["ExitCode=0"], "> ")));
            std::mem::take(&mut t.events)
        };
        feed(&mut t, &mut m, &prompt("> "));
        t.events.clear();
        // Sent while locked (fish emits it after OSCLock): still honoured; the menu still resets.
        // r3-nohistory-skips-command-finish: it still ends (the engine resets its generator results).
        assert_eq!(run(&mut t, &mut m, " set -l TOKEN hunter2", true), [Event::PreExec, Event::CommandEnded]);
        assert!(!t.no_history);
        let ev = run(&mut t, &mut m, "ls", false);
        assert!(matches!(&ev[..], [Event::PreExec, Event::NewCmd(Some(h))] if h.command == "ls"), "{ev:?}");
        // A running command printing the marker cannot hide itself or the next command.
        feed(&mut t, &mut m, &format!("make\r\n{}{}", osc(&format!("OSCLock={SID}")), osc("PreExec")));
        feed(&mut t, &mut m, &format!("{}{}", osc("NoHistory"), precmd(&[], "> ")));
        assert!(matches!(&t.events[..], [Event::PreExec, Event::NewCmd(Some(h))] if h.command == "make"), "{:?}", t.events);
    }

    #[test]
    fn submitted_line_drops_shell_output_before_preexec() {
        let (mut t, mut m) = setup(40, 6, Shell::Bash);
        let finish = |t: &mut Tracker, m: &mut Model, screen: &str, submitted: &str| {
            t.events.clear();
            feed(t, m, &prompt("$ "));
            feed(t, m, screen);
            t.submitted = Some(submitted.into());
            feed(t, m, &format!("{}{}\r\n{}", osc(&format!("OSCLock={SID}")), osc("PreExec"), precmd(&[], "$ ")));
            let Some(Event::NewCmd(Some(h))) = t.events.last() else { panic!("{:?}", t.events) };
            assert_eq!(t.submitted, None);
            h.command.clone()
        };
        // bash/zsh echo the `!!` expansion (and zsh its correction prompt) before PreExec.
        assert_eq!(finish(&mut t, &mut m, "echo !!\r\necho echo first", "echo !! "), "echo !!");
        // A line the shell rewrote on Enter (fish abbreviation, PS2 loop) is taken from the screen.
        assert_eq!(finish(&mut t, &mut m, "git checkout", "gco"), "git checkout");
        assert_eq!(finish(&mut t, &mut m, &format!("echo 'a\r\n{SP}> {EP}b'"), "echo 'a\nb'"), "echo 'a\nb'");
    }

    #[test]
    fn bash_redraw_of_the_prompt_alone_keeps_the_line() {
        // bash-readline-reused-cells-lose-buffer: leaving i-search deletes its prefix (DCH) and reprints only the
        // prompt and the changed start; the shifted rest of the line stays.
        let (mut t, mut m) = setup(80, 5, Shell::Bash);
        feed(&mut t, &mut m, &format!("{}\r(reverse-i-search)`onel': git log --oneline -1 feature-", prompt("$ ")));
        feed(&mut t, &mut m, &format!("\r\x1b[24P{}git log --onel{}", prompt("$ "), "\x08".repeat(5)));
        assert_eq!(buf(&t, &m), ("git log --oneline -1 feature-".into(), 9));
        // vi mode with show-mode-in-prompt: the mode string is redrawn and the line moved over with CUF.
        let (mut t, mut m) = setup(40, 5, Shell::Bash);
        feed(&mut t, &mut m, &format!("(ins){}git ch", prompt("$ ")));
        feed(&mut t, &mut m, &format!("\r(cmd){}\x1b[5C", prompt("$ ")));
        feed(&mut t, &mut m, &format!("\r(ins){}\x1b[6Ce", prompt("$ ")));
        assert_eq!(buf(&t, &m), ("git che".into(), 7));
        // A two-line prompt: only its last line is redrawn, without StartPrompt; it is still the prompt.
        let (mut t, mut m) = setup(40, 5, Shell::Bash);
        feed(&mut t, &mut m, &format!("{SP}repo\r\n[I]> {EP}{}git ch", nc()));
        feed(&mut t, &mut m, &format!("\r[N]> {EP}{}\x1b[5C", nc()));
        let e = t.extract(&m).unwrap();
        assert_eq!((e.buffer.as_str(), e.cursor, e.at_prompt), ("git ch", 5, true));
        // A finished command's cells are not taken over by the next prompt.
        feed(&mut t, &mut m, &format!("\r\n{}{}", osc("PreExec"), prompt("$ ")));
        assert_eq!(buf(&t, &m), (String::new(), 0));
    }

    #[test]
    fn transient_prompt_before_preexec_keeps_the_submitted_line() {
        let (mut t, mut m) = setup(60, 8, Shell::Zsh);
        let run = |t: &mut Tracker, m: &mut Model, typed: &str, finish: &str| {
            feed(t, m, &format!("{}{typed}", prompt("% ")));
            t.submitted = Some(typed.into());
            feed(t, m, &format!("{finish}{}{}out\r\n{}", osc(&format!("OSCLock={SID}")), osc("PreExec"), precmd(&[], "% ")));
            let Some(Event::NewCmd(Some(h))) = std::mem::take(&mut t.events).pop() else { panic!() };
            h.command
        };
        // r3-transient-redraw-clears-submitted: zle-line-finish re-expands PS1 (NewCmd) between Enter and PreExec.
        let redraw = format!("\r{}echo !!\r\necho echo second\r\n", prompt("% "));
        assert_eq!(run(&mut t, &mut m, "echo !!", &redraw), "echo !!");
        // transient-prompt-pollutes-history: a transient prompt drawn without markers lands in the scrape.
        let transient = "\r\x1b[K\u{276f} gti status\r\nzsh: correct 'gti' to 'git' [nyae]? y\r\n";
        assert_eq!(run(&mut t, &mut m, "gti status", transient), "gti status");
        assert_eq!(run(&mut t, &mut m, "echo hi", "\r\x1b[K\u{276f} echo hi\r\n"), "echo hi");
        // Only as whole words ending a line: a rewritten line is still read from the screen.
        assert_eq!(run(&mut t, &mut m, "gco", "\r\x1b[K\u{276f} git checkout\r\n"), "\u{276f} git checkout");
        assert_eq!(run(&mut t, &mut m, "ls", "\r\x1b[K\u{276f} lsd\r\n"), "\u{276f} lsd");
    }

    #[test]
    fn nested_shell_prompt_keeps_outer_command_running() {
        let (mut t, mut m) = setup(40, 8, Shell::Fish);
        let at = |pid: u32, exit: u32| precmd(&[&format!("PID={pid}"), &format!("ExitCode={exit}")], "> ");
        let run = |t: &mut Tracker, m: &mut Model, cmd: &str, then: String| {
            feed(t, m, &format!("{cmd}\r\n{}{}{then}", osc(&format!("OSCLock={SID}")), osc("PreExec")));
            std::mem::take(&mut t.events)
                .into_iter()
                .filter_map(|e| if let Event::NewCmd(h) = e { Some(h.map(|h| (h.command, h.exit_code))) } else { None })
                .collect::<Vec<_>>()
        };
        feed(&mut t, &mut m, &at(100, 0));
        t.events.clear();
        // fish (100) → bash (200) → `false` → `exit 5` → fish prompt (exit 5).
        assert_eq!(run(&mut t, &mut m, "bash", at(200, 0)), [None]);
        assert_eq!(run(&mut t, &mut m, "false", at(200, 1)), [Some(("false".into(), Some(1)))]);
        let done = run(&mut t, &mut m, "exit 5", at(100, 5));
        assert_eq!(done, [Some(("exit 5".into(), Some(5))), Some(("bash".into(), Some(5)))]);
        assert!(t.pending.is_empty());
        // A prompt redraw of the same shell finishes nothing.
        feed(&mut t, &mut m, &format!("\r{}", prompt("> ")));
        assert_eq!(t.events, [Event::NewCmd(None)]);
    }

    #[test]
    fn pwsh_continuation_and_prompt_text_stripped() {
        // Default ContinuationPrompt `>> ` before the prompt reports it; the cursor keeps its character.
        let (mut t, mut m) = setup(40, 6, Shell::Pwsh);
        feed(&mut t, &mut m, &format!("{}if ($true) {{\r\n>> git ch", prompt("PS> ")));
        assert_eq!(buf(&t, &m), ("if ($true) {\ngit ch".into(), 19));
        feed(&mut t, &mut m, "\x1b[2D");
        assert_eq!(buf(&t, &m), ("if ($true) {\ngit ch".into(), 17));
        // Reported prompts; PSReadLine re-renders the PromptText `> ` (red) on a parse error.
        let (mut t, mut m) = setup(40, 6, Shell::Pwsh);
        let prompts = osc("PwshPrompts=... \x1f> ");
        feed(&mut t, &mut m, &format!("{prompts}{}echo (2\x1b[3G\x1b[31m> \x1b[0m\x1b[12G", prompt("PS> ")));
        assert_eq!(buf(&t, &m), ("echo (2".into(), 7));
        feed(&mut t, &mut m, "\r\n... )");
        assert_eq!(buf(&t, &m), ("echo (2\n)".into(), 9));
        // Cursor inside a stripped prompt: at its line start.
        feed(&mut t, &mut m, "\x1b[2G");
        assert_eq!(buf(&t, &m), ("echo (2\n)".into(), 8));
        feed(&mut t, &mut m, "\x1b[A\x1b[3G");
        assert_eq!(buf(&t, &m), ("echo (2".into(), 0));
    }

    #[test]
    fn pwsh_list_view_rows_not_in_buffer() {
        // PSReadLine pads the input row to the full width, so its ListView rows can look like a wrap continuation.
        let screen = format!("{}git ch{}<-/4> > cherry-pick\x1b[1;11H", prompt("PS> "), " ".repeat(10));
        let (mut t, mut m) = setup(20, 6, Shell::Pwsh);
        feed(&mut t, &mut m, &screen);
        assert_ne!(buf(&t, &m).0, "git ch");
        let (mut t, mut m) = setup(20, 6, Shell::Pwsh);
        feed(&mut t, &mut m, &format!("{}{screen}", osc("PwshListView=1")));
        assert!(t.pwsh_list_view);
        assert_eq!(buf(&t, &m), ("git ch".into(), 6));
        feed(&mut t, &mut m, &osc("PwshListView=0"));
        assert!(!t.pwsh_list_view);
        feed(&mut t, &mut m, &format!("{}{}", osc("PwshListView=1"), osc("PreExec")));
        assert!(!t.pwsh_list_view, "a command (maybe a nested shell) does not inherit it");
    }

    #[test]
    fn extract_prompt_scrolled_by_output() {
        let (mut t, mut m) = setup(10, 3, Shell::Bash);
        feed(&mut t, &mut m, "1\r\n2\r\n3\r\n");
        let cmd = "echo hello world again";
        feed(&mut t, &mut m, &format!("{}{cmd}", prompt("$ ")));
        // The prompt row has scrolled into history; tags moved with it.
        assert!(m.grid().history_size() > 0);
        assert_eq!(buf(&t, &m), (cmd.into(), cmd.len()));
        // Command output below a finished command does not leak into the next buffer.
        feed(&mut t, &mut m, &format!("\r\n{}out\r\n{}x", osc("PreExec"), prompt("$ ")));
        assert_eq!(buf(&t, &m), ("x".into(), 1));
    }

    #[test]
    fn windows_deferral_fifo() {
        let (mut t, mut m) = setup(40, 5, Shell::Pwsh);
        t.defer = true;
        feed(&mut t, &mut m, &format!("{SP}PS> {EP}{}", nc()));
        assert_eq!(t.cycle, None);
        assert_eq!(t.deferred, [Deferred::EndPrompt, Deferred::NewCmd]);
        // A batch with markers keeps deferring.
        feed(&mut t, &mut m, &osc("Dir=/x"));
        assert_eq!(t.cycle, None);
        // A marker-free batch applies the queue in order, at the current cursor.
        feed(&mut t, &mut m, "\x1b[?25h");
        assert!(t.deferred.is_empty());
        assert_eq!((t.cycle, &t.events[..]), (Some(1), &[Event::NewCmd(None)][..]));
        feed(&mut t, &mut m, "ls");
        assert_eq!(buf(&t, &m), ("ls".into(), 2));
    }

    #[test]
    fn suggestion_colors() {
        let z = SuggestionColor::parse_zsh("fg=8,bold").unwrap();
        assert_eq!(z, SuggestionColor { fg: vec![Some(8)], bg: vec![] });
        assert_eq!(SuggestionColor::parse_zsh("fg=yellow").unwrap().fg, [Some(3)]);
        assert_eq!(SuggestionColor::parse_zsh("fg=245").unwrap().fg, [Some(245)]);
        assert_eq!(SuggestionColor::parse_zsh("fg=#aaaaaa,bg=default").unwrap().bg, [None]);
        assert_eq!(SuggestionColor::parse_zsh("bold"), None);
        let f = SuggestionColor::parse_fish("555 brblack --bold").unwrap();
        assert_eq!(f.fg, [Some(rgb_to_256(0x55, 0x55, 0x55)), Some(8)]);
        assert_eq!(SuggestionColor::parse_fish("-b red 666").unwrap().fg, [Some(rgb_to_256(0x66, 0x66, 0x66))]);
        // Cell colours: named, indexed and truecolor all map to xterm-256.
        let mut cell = Cell { fg: Color::Named(alacritty_terminal::vte::ansi::NamedColor::BrightBlack), ..Cell::default() };
        assert!(z.matches(&cell));
        cell.fg = Color::Spec(alacritty_terminal::vte::ansi::Rgb { r: 0xaa, g: 0xaa, b: 0xaa });
        assert!(SuggestionColor::parse_zsh("fg=#aaaaaa").unwrap().matches(&cell));
        assert!(!z.matches(&cell));
        assert_eq!((rgb_to_256(0, 0, 0), rgb_to_256(255, 0, 0), rgb_to_256(128, 128, 128)), (16, 196, 244));
    }

    fn filter(f: &mut OutFilter, s: &[u8]) -> Vec<u8> {
        let mut out = Vec::new();
        f.feed(s, &mut out);
        out
    }

    #[test]
    fn output_filter_strips_keyboard_upgrades() {
        let mut f = OutFilter::new(24);
        for seq in ["\x1b[?9001h", "\x1b[?9001l", "\x1b[?u", "\x1b[=1;1u", "\x1b[>1u", "\x1b[<1u", "\x1b[>4;2m", "\x1b[>4m"] {
            assert_eq!(filter(&mut f, format!("a{seq}b").as_bytes()), b"ab", "{seq:?}");
        }
        for seq in ["\x1b[u", "\x1b[>c", "\x1b[?2004h", "\x1b[38;2;219;177;49m", "\x1b(B", "\x1b7"] {
            assert_eq!(filter(&mut f, seq.as_bytes()), seq.as_bytes(), "{seq:?}");
        }
        // Split across batches: held back, then stripped / emitted whole.
        assert_eq!(filter(&mut f, b"before\x1b[>4;"), b"before");
        assert_eq!(filter(&mut f, b"2mafter"), b"after");
        assert_eq!(filter(&mut f, b"x\x1b[3"), b"x");
        assert_eq!(filter(&mut f, b"1mz"), b"\x1b[31mz");
        // Inside the alternate screen the app keeps its keyboard protocol.
        assert_eq!(filter(&mut f, b"\x1b[?1049h\x1b[>1u"), b"\x1b[?1049h\x1b[>1u");
        assert_eq!(filter(&mut f, b"\x1b[<u\x1b[?1049l\x1b[>1u"), b"\x1b[<u\x1b[?1049l");
    }

    #[test]
    fn output_filter_tracks_dcs_and_apc_strings() {
        let mut f = OutFilter::new(24);
        // Split DCS (XTGETTCAP): open across the batch boundary, closed by ST; bytes pass through unchanged.
        assert_eq!(filter(&mut f, b"a\x1bP+q"), b"a\x1bP+q");
        assert!(f.in_string());
        assert_eq!(filter(&mut f, b"696e"), b"696e");
        assert!(f.in_string());
        assert_eq!(filter(&mut f, b"646e\x1b\\b"), b"646e\x1b\\b");
        assert!(!f.in_string());
        // APC split right before its ST; a trailing ESC is held as usual.
        assert_eq!(filter(&mut f, b"\x1b_Gi=1;x\x1b"), b"\x1b_Gi=1;x");
        assert!(!f.in_string());
        assert_eq!(filter(&mut f, b"\\z"), b"\x1b\\z");
        // CAN aborts a string; a bare LF inside a body is not rewritten.
        f.lf_to_ind = true;
        assert_eq!(filter(&mut f, b"\x1bPq#0\n\x18c"), b"\x1bPq#0\n\x18c");
        assert!(!f.in_string());
        // Other ESC sequences do not open a string.
        assert_eq!(filter(&mut f, b"\x1b7\x1b(B"), b"\x1b7\x1b(B");
        assert!(!f.in_string());
    }

    #[test]
    fn output_filter_passes_overlong_sequence_through() {
        // A stray `ESC [` before a megabyte of digits: bounded carry (no quadratic rescans), bytes pass through,
        // and the render guard holds until the final byte.
        let mut f = OutFilter::new(24);
        let mut out = filter(&mut f, b"x\x1b[");
        let digits = vec![b'1'; 16 * 1024];
        for _ in 0..64 {
            out.extend(filter(&mut f, &digits));
            assert!(f.carry.len() <= SEQ_MAX + digits.len());
        }
        assert!(f.in_string() && out.len() > 64 * digits.len() - SEQ_MAX - digits.len());
        out.extend(filter(&mut f, b"1mz\x1b[?9001h"));
        assert!(!f.in_string());
        assert_eq!(out, [&b"x\x1b["[..], &vec![b'1'; 64 * digits.len() + 1], b"mz"].concat());
    }

    #[test]
    fn output_filter_tracks_scroll_region_and_utf8() {
        let mut f = OutFilter::new(24);
        filter(&mut f, b"\x1b[2;20r");
        assert!(f.scroll_region());
        filter(&mut f, b"\x1b[r");
        assert!(!f.scroll_region());
        filter(&mut f, b"\x1b[1;24r");
        assert!(!f.scroll_region());
        filter(&mut f, b"\x1b[5;30r");
        assert!(f.scroll_region());
        f.resize(30);
        assert!(!f.scroll_region());
        // UTF-8 split across reads is never emitted partially.
        let e = "é€".as_bytes();
        assert_eq!(filter(&mut f, &e[..1]), b"");
        assert_eq!(filter(&mut f, &e[1..3]), "é".as_bytes());
        assert_eq!(filter(&mut f, &e[3..]), "€".as_bytes());
        // Bare LF → ESC D only when asked (Windows without DISABLE_NEWLINE_AUTO_RETURN).
        assert_eq!(filter(&mut f, b"a\nb\r\n"), b"a\nb\r\n");
        f.lf_to_ind = true;
        assert_eq!(filter(&mut f, b"a\nb\r\n\n"), b"a\x1bDb\r\n\x1bD");
    }
}
