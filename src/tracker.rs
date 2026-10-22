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
    /// Windows console without `DISABLE_NEWLINE_AUTO_RETURN`: bare LF → `ESC D`.
    pub lf_to_ind: bool,
}

impl OutFilter {
    pub fn new(rows: usize) -> OutFilter {
        OutFilter { carry: Vec::new(), rows, alt: false, partial_region: false, in_string: false, lf_to_ind: false }
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

    /// A DCS/APC/PM/SOS string is open on the terminal (render guard: a frame would land inside it).
    pub fn in_string(&self) -> bool {
        self.in_string
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
    pub submitted: Option<String>,
    pub no_history: bool,
    pub pwsh_list_view: bool,
    /// Drained by the wrapper after every batch.
    pub events: Vec<Event>,
    pub filter: OutFilter,
    pending: Option<HistoryEntry>,
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
            submitted: None,
            no_history: false,
            pwsh_list_view: false,
            events: Vec::new(),
            filter: OutFilter::new(rows),
            pending: None,
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
        if self.osc_lock && !matches!(key, "NewCmd" | "PreExec" | "OSCLock" | "OSCUnlock" | "User") {
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
            "OSCLock" if v == self.sid => self.osc_lock = true,
            "OSCUnlock" if v == self.sid => self.osc_lock = false,
            "Dir" => self.cwd = Some(PathBuf::from(val.unwrap_or(""))),
            "ExitCode" => {
                if let Some(p) = &mut self.pending {
                    p.exit_code = v.parse().ok();
                    p.end_time = Some(now_secs());
                }
            }
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
            _ => {}
        }
    }

    fn end_prompt(&mut self, model: &mut Model) {
        let open = self.cycle.filter(|_| !self.preexec);
        tag(model, open.map(|n| format!("tabsmith:c{n}")));
    }

    fn new_cmd(&mut self, model: &mut Model) {
        let n = self.cycle.map_or(1, |n| n + 1);
        self.cycle = Some(n);
        self.preexec = false;
        tag(model, Some(format!("tabsmith:c{n}")));
        self.events.push(Event::NewCmd(self.pending.take()));
    }

    fn pre_exec(&mut self, model: &mut Model) {
        self.flush_deferred(model); // a queued NewCmd belongs before this PreExec
        let command = self.extract(model).map(|e| e.buffer.trim().to_string()).unwrap_or_default();
        self.pending = Some(HistoryEntry {
            command,
            shell: self.shell_name.clone().unwrap_or_default(),
            pid: self.pid,
            session_id: self.sid.clone(),
            cwd: self.cwd.as_ref().map(|p| p.display().to_string()),
            start_time: now_secs(),
            ..Default::default()
        });
        self.preexec = true;
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

    /// Suggestion-coloured cell (SPEC §5.1.3; pwsh: PSReadLine predictions are dim or italic).
    fn suggestion(&self, cell: &Cell) -> bool {
        match self.shell {
            Some(Shell::Zsh) => self.zsh_color.as_ref().is_some_and(|c| c.matches(cell)),
            Some(Shell::Fish) => self.fish_color.as_ref().is_some_and(|c| c.matches(cell)),
            Some(Shell::Pwsh) => cell.flags.intersects(Flags::DIM | Flags::ITALIC),
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
        // Below the cursor only wrap continuations and PS2 rows count (not zsh completion listings).
        let mut end = cl;
        while end < grid.bottommost_line().0 && (wraps(end) || (has(&row(end + 1)[0], TAG_PROMPT) && tagged(end + 1))) {
            end += 1;
        }

        let mut e = Extracted::default();
        let mut cursor = None;
        let (mut pad, mut line_has_cmd) = (0, false);
        for r in start..=end {
            if r == start || !wraps(r - 1) {
                // ponytail: untagged cells before the first command cell of a line (prompt) are not padding.
                pad = 0;
                line_has_cmd = false;
            }
            let mut row_has_cmd = false;
            for (c, cell) in row(r).iter().enumerate() {
                if (r, c) == (cl, cc) {
                    cursor = Some(e.buffer.len()); // padding before the cursor is not flushed
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
                cursor = Some(e.buffer.len());
            }
            if r < end && !wraps(r) && (row_has_cmd || !e.buffer.is_empty()) {
                e.buffer.push('\n');
            }
        }
        e.cursor = cursor.unwrap_or(e.buffer.len());
        // Blank command cells after the cursor are drawing leftovers (Kiro trims them too).
        let keep = e.buffer.trim_end().len().max(e.cursor);
        e.buffer.truncate(keep);
        Some(e)
    }
}

/// Inject a cell tag into the model only (OSC 8 hyperlink on the cursor template; SPEC D3).
fn tag(model: &mut Model, uri: Option<String>) {
    model.set_hyperlink(uri.map(|uri| Hyperlink { id: None, uri }));
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
