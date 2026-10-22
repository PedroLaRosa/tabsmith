//! `tabsmith _ pty`: the PTY wrapper session (SPEC §4.3–4.5, §7.2.4, §7.3, §7.6, §8).
//! OWNER: wrapper agent.
use crate::keys::{self, Action, Binding, Key, KeyCode, Keymap, Mods};
use crate::logger;
use crate::menu::{Effect, Menu};
use crate::render::{FrameInput, Renderer, Theme};
use crate::settings::Settings;
use crate::tracker::{Event, Extracted, Tracker};
use crate::types::{Completion, EngineMsg, Insertion, Model, Msg, Phase, Request, SessionCtx, Shell};
use alacritty_terminal::event::VoidListener;
use alacritty_terminal::grid::Dimensions;
use alacritty_terminal::term::test::TermSize;
use alacritty_terminal::term::{Config, Term, TermMode};
use anyhow::{anyhow, bail, Context, Result};
use portable_pty::{CommandBuilder, MasterPty, PtySize};
use std::io::{IsTerminal, Read, Write};
use std::panic::AssertUnwindSafe;
use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

const RESIZE_DEBOUNCE: Duration = Duration::from_millis(150);
const INSERT_LOCK: Duration = Duration::from_millis(100);
const SPINNER_TICK: Duration = Duration::from_millis(80);
const FILE_POLL: Duration = Duration::from_secs(2);
const PROBE_TIMEOUT: Duration = Duration::from_millis(300);
/// After the child exits, wait this long for the rest of its output (PTY EOF) before leaving.
const EXIT_GRACE: Duration = Duration::from_millis(200);

/// Run the wrapper; returns the exit code to exit with. Never leaves the user without a shell (fallback, SPEC §4.4).
pub fn run(shell: Option<String>, cmd: Vec<String>) -> i32 {
    // Panics are logged, never printed over the user's screen; the session then falls back to a plain shell.
    std::panic::set_hook(Box::new(|info| logger::error(&format!("panic: {info}"))));
    let shell = shell
        .or_else(|| env_nonempty("TABSMITH_SHELL"))
        .or_else(|| if cfg!(windows) { Some("pwsh".into()) } else { env_nonempty("SHELL") });
    let login = std::env::var("TABSMITH_IS_LOGIN_SHELL").as_deref() == Ok("1");
    let argv = argv(shell.as_deref(), &cmd, login);
    let mut modes = None;
    let res = std::panic::catch_unwind(AssertUnwindSafe(|| Session::start(&argv, &mut modes).map(Session::run)));
    let err = match res {
        Ok(Ok(code)) => return code,
        Ok(Err(e)) => e,
        Err(_) => anyhow!("internal error (see pty.log)"),
    };
    if let Some(m) = modes {
        m.restore();
    }
    fallback(&err, &argv)
}

fn env_nonempty(k: &str) -> Option<String> {
    std::env::var(k).ok().filter(|v| !v.is_empty())
}

/// Child argv (SPEC §4.4 step 7). After `--`: a command (replaces the shell), or extra shell arguments when they
/// start with `-` (`debug shell`: `--shell bash -- --noprofile --rcfile …`).
fn argv(shell: Option<&str>, extra: &[String], login: bool) -> Vec<String> {
    if extra.first().is_some_and(|a| !a.starts_with('-')) {
        return extra.to_vec();
    }
    let Some(shell) = shell else { return Vec::new() };
    let mut v = vec![shell.to_string()];
    if login && matches!(Shell::from_name(shell), Some(Shell::Bash | Shell::Zsh | Shell::Fish)) {
        v.push("--login".into());
    }
    v.extend(extra.iter().cloned());
    v
}

/// Run the plain shell with `TABSMITH_TERM_DISABLED=1` so its pre block does not loop (SPEC §4.4 fallback).
fn fallback(err: &anyhow::Error, argv: &[String]) -> i32 {
    logger::error(&format!("pty: {err:#}"));
    eprintln!("tabsmith had an error: {err:#}");
    let argv = if argv.is_empty() { vec![if cfg!(windows) { "pwsh" } else { "/bin/sh" }.to_string()] } else { argv.to_vec() };
    let mut c = std::process::Command::new(&argv[0]);
    c.args(&argv[1..])
        .env("TABSMITH_TERM_DISABLED", "1")
        .env_remove("TABSMITH_SHELL")
        .env_remove("TABSMITH_IS_LOGIN_SHELL");
    #[cfg(unix)]
    let err = std::os::unix::process::CommandExt::exec(&mut c); // returns only on error
    #[cfg(windows)]
    let err = match c.status() {
        Ok(s) => return s.code().unwrap_or(1),
        Err(e) => e,
    };
    eprintln!("tabsmith: could not start {}: {err}", argv[0]);
    1
}

/// Bytes for an insertion, reconciled with what the shell shows now (SPEC §7.3).
/// `ib` = buffer the reply was computed from, `cur` = buffer extracted now.
pub fn insertion_bytes(ins: &Insertion, ib: &str, cur: &str) -> Vec<u8> {
    let mut b = Vec::new();
    if let Some(extra) = cur.strip_prefix(ib) {
        b.extend(std::iter::repeat_n(0x7f, extra.chars().count())); // the user typed more meanwhile
    } else if let Some(missing) = ib.strip_prefix(cur) {
        b.extend_from_slice(missing.as_bytes()); // the shell has not echoed yet
    }
    b.extend(std::iter::repeat_n(0x7f, ins.delete));
    b.extend_from_slice(ins.text.as_bytes());
    for _ in 0..ins.cursor_back {
        b.extend_from_slice(b"\x1b[D");
    }
    if ins.execute {
        b.push(b'\r');
    }
    b
}

/// Buffer expected once the shell applied an insertion (insertion-lock release, SPEC §7.3).
/// `cursor` = byte offset into `ib` the reply was computed at.
pub fn expected_buffer(ins: &Insertion, ib: &str, cursor: usize) -> String {
    let cursor = if ib.is_char_boundary(cursor) { cursor } else { ib.len() };
    let mut s = ib[..cursor].to_string();
    for _ in 0..ins.delete {
        s.pop();
    }
    s.push_str(&ins.text);
    s.push_str(&ib[cursor..]);
    s
}

/// Ghost accept keys (SPEC §8.3): `Some(true)` = whole ghost, `Some(false)` = next word.
fn ghost_key(k: &Key) -> Option<bool> {
    let m = k.mods;
    let plain = m == Mods::default();
    let ctrl = m == Mods { ctrl: true, ..Mods::default() };
    let alt = m == Mods { alt: true, ..Mods::default() };
    match k.code {
        KeyCode::Right | KeyCode::End if plain => Some(true),
        KeyCode::Char('f' | 'F' | 'e' | 'E') if ctrl => Some(true),
        KeyCode::Char('f') if alt => Some(false),
        KeyCode::Right if ctrl => Some(false),
        _ => None,
    }
}

/// Leading whitespace + the next run of non-whitespace chars.
fn next_word(g: &str) -> &str {
    let start = g.len() - g.trim_start().len();
    let end = g[start..].find(char::is_whitespace).map_or(g.len(), |i| start + i);
    &g[..end]
}

fn is_ctrl(k: &Key, c: char) -> bool {
    k.mods.ctrl && matches!(k.code, KeyCode::Char(x) if x.eq_ignore_ascii_case(&c))
}

type Rgb = (u8, u8, u8);

/// Startup probe replies (SPEC §4.4 step 5): `(CPR (row, col), OSC 11 background, other bytes typed meanwhile)`.
fn parse_probe(buf: &[u8]) -> (Option<(u16, u16)>, Option<Rgb>, Vec<u8>) {
    let (mut cpr, mut bg, mut rest) = (None, None, Vec::new());
    let mut i = 0;
    while i < buf.len() {
        let s = &buf[i..];
        if let Some((r, c, n)) = parse_cpr(s) {
            cpr = Some((r, c));
            i += n;
        } else if let Some((rgb, n)) = parse_osc11(s) {
            bg = rgb.or(bg);
            i += n;
        } else {
            rest.push(buf[i]);
            i += 1;
        }
    }
    (cpr, bg, rest)
}

/// `ESC [ row ; col R` → (row, col, length).
fn parse_cpr(s: &[u8]) -> Option<(u16, u16, usize)> {
    let body = s.strip_prefix(b"\x1b[")?;
    let end = body.iter().position(|b| !(b.is_ascii_digit() || *b == b';'))?;
    if body[end] != b'R' {
        return None;
    }
    let (r, c) = std::str::from_utf8(&body[..end]).ok()?.split_once(';')?;
    Some((r.parse().ok()?, c.parse().ok()?, end + 3))
}

/// `ESC ] 11 ; rgb:RRRR/GGGG/BBBB (BEL | ESC \)` → (colour, length).
fn parse_osc11(s: &[u8]) -> Option<(Option<Rgb>, usize)> {
    let body = s.strip_prefix(b"\x1b]11;")?;
    let end = body.iter().position(|b| *b == 0x07 || *b == 0x1b)?;
    let term_len = if body[end] == 0x07 { 1 } else if body.get(end + 1) == Some(&b'\\') { 2 } else { return None };
    let text = std::str::from_utf8(&body[..end]).ok()?;
    let mut parts = text.strip_prefix("rgb:").into_iter().flat_map(|t| t.split('/')).map(|h| {
        let v = u32::from_str_radix(h, 16).ok().filter(|_| (1..=4).contains(&h.len()))?;
        Some((v * 255 / ((1 << (4 * h.len())) - 1)) as u8)
    });
    let rgb = (|| Some((parts.next()??, parts.next()??, parts.next()??)))();
    Some((rgb, 5 + end + term_len))
}

fn session_id() -> String {
    let nanos = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_nanos() as u64).unwrap_or(0);
    format!("{:016x}{:016x}", nanos, std::process::id() as u64 ^ nanos.rotate_left(17))
}

fn mtime(p: &Option<PathBuf>) -> Option<SystemTime> {
    std::fs::metadata(p.as_ref()?).ok()?.modified().ok()
}

fn write_stdout(b: &[u8]) {
    if b.is_empty() {
        return;
    }
    let mut o = std::io::stdout().lock();
    let _ = o.write_all(b);
    let _ = o.flush();
}

struct Session {
    rx: Receiver<Msg>,
    engine: Sender<EngineMsg>,
    writer: Sender<Vec<u8>>,
    /// Dropped after the child exits on Windows to unblock the ConPTY reader.
    master: Option<Box<dyn MasterPty + Send>>,
    modes: sys::Modes,
    term: Model,
    tracker: Tracker,
    menu: Menu,
    renderer: Renderer,
    keymap: Keymap,
    settings: Settings,
    settings_path: Option<PathBuf>,
    settings_mtime: Option<SystemTime>,
    session_path: Option<PathBuf>,
    session_mtime: Option<SystemTime>,
    terminal_bg: Option<Rgb>,
    /// Shell of the spawned program (the tracker's `Shell=` wins once known).
    shell: Shell,
    /// Latest completion request id and the buffer/cursor it was sent for.
    req: u64,
    sent: Option<(String, usize)>,
    changed_at: Instant,
    /// Edit buffer at the last batch.
    cur: Option<Extracted>,
    /// Insertion lock: (deadline, expected buffer) (SPEC §7.3).
    lock: Option<(Instant, String)>,
    /// Ghost candidates as (buffer they were computed for, text): local (engine) and LLM.
    ghost: Option<(String, String)>,
    llm: Option<(String, String)>,
    llm_req: u64,
    llm_at: Option<Instant>,
    resize_at: Option<Instant>,
    repaint: bool,
    spinner: usize,
    spinner_at: Option<Instant>,
    poll_at: Instant,
    exit_code: Option<i32>,
    eof: bool,
    exit_at: Option<Instant>,
}

impl Session {
    /// SPEC §4.4 steps 1–9. `modes` receives the saved terminal modes as soon as they change.
    fn start(argv: &[String], modes: &mut Option<sys::Modes>) -> Result<Session> {
        if !std::io::stdin().is_terminal() || !std::io::stdout().is_terminal() {
            bail!("stdin/stdout is not a terminal");
        }
        let program = argv.first().context("no shell to run (set SHELL or pass --shell)")?;
        let settings = Settings::load();
        let (m, lf_to_ind) = sys::enter()?;
        *modes = Some(m);
        let (cols, rows) = crossterm::terminal::size().context("terminal size")?;
        let (cols, rows) = (cols.max(1), rows.max(1));
        let mut term = Term::new(
            Config { scrolling_history: 2000, ..Config::default() },
            &TermSize::new(cols as usize, rows as usize),
            VoidListener,
        );
        let sid = session_id();
        let shell = Shell::from_name(program).unwrap_or(if cfg!(windows) { Shell::Pwsh } else { Shell::Bash });
        let mut tracker = Tracker::new(sid.clone(), Shell::from_name(program), rows as usize);
        tracker.filter.lf_to_ind = lf_to_ind;

        // Sync the model with the real cursor (and ask for the background for the `system` theme).
        let system_theme = settings.string("autocomplete.theme") == Some("system");
        write_stdout(if system_theme { b"\x1b]11;?\x1b\\\x1b[6n" } else { b"\x1b[6n" });
        let deadline = Instant::now() + PROBE_TIMEOUT;
        let mut replies = Vec::new();
        let (cpr, terminal_bg, typeahead) = loop {
            let parsed = parse_probe(&replies);
            let now = Instant::now();
            if parsed.0.is_some() || now >= deadline {
                break parsed;
            }
            replies.extend(sys::read_timeout(deadline - now));
        };
        match cpr {
            Some((r, c)) => tracker.advance(&mut term, format!("\x1b[{r};{c}H").as_bytes()),
            None => write_stdout(b"\x1b[H\x1b[2J"),
        }

        // Everything that can fail goes before the child exists, so the fallback never orphans a shell.
        let truecolor = matches!(std::env::var("COLORTERM").as_deref(), Ok("truecolor" | "24bit"))
            || std::env::var_os("WT_SESSION").is_some();
        let renderer = Renderer::new(Theme::from_settings(&settings, terminal_bg), truecolor);
        let (menu, keymap) = (Menu::new(), Keymap::from_settings(&settings));
        let (tx, rx) = mpsc::channel();
        sys::spawn_resize(tx.clone())?;
        let engine = crate::engine::spawn(tx.clone(), settings.clone());

        let pair = portable_pty::native_pty_system().openpty(PtySize { rows, cols, pixel_width: 0, pixel_height: 0 })?;
        let mut cb = CommandBuilder::from_argv(argv.iter().map(Into::into).collect());
        let cwd = std::env::current_dir().ok().filter(|d| d.is_dir()).or_else(|| crate::settings::user_home().ok());
        if let Some(cwd) = cwd {
            cb.cwd(cwd);
        }
        let version = env!("CARGO_PKG_VERSION");
        cb.env("TABSMITH_TERM", version);
        if std::env::var_os("TMUX").is_some() {
            cb.env("TABSMITH_TERM_TMUX", version);
        }
        cb.env("TABSMITH_SESSION_ID", &sid);
        cb.env_remove("TABSMITH_SHELL");
        cb.env_remove("TABSMITH_IS_LOGIN_SHELL");
        let mut child = pair.slave.spawn_command(cb)?;
        drop(pair.slave);
        let mut reader = pair.master.try_clone_reader()?;
        let mut pty_writer = pair.master.take_writer()?;
        logger::info(&format!("pty: {argv:?} {cols}x{rows} sid {sid} cpr {cpr:?}"));

        let base = program.rsplit(['/', '\\']).next().unwrap_or(program);
        write_stdout(format!("\x1b]0;{base}\x07").as_bytes());

        // Threads (SPEC §4.3).
        let t = tx.clone();
        thread::spawn(move || {
            let mut buf = vec![0u8; 16 * 1024];
            loop {
                match reader.read(&mut buf) {
                    Ok(0) => break,
                    Ok(n) => {
                        if t.send(Msg::Pty(buf[..n].to_vec())).is_err() {
                            return;
                        }
                    }
                    Err(e) if e.kind() == std::io::ErrorKind::Interrupted => {}
                    Err(_) => break, // Linux: EIO once the slave side is gone
                }
            }
            let _ = t.send(Msg::PtyEof);
        });
        let (writer, wrx) = mpsc::channel::<Vec<u8>>();
        thread::spawn(move || {
            for b in wrx {
                if pty_writer.write_all(&b).and_then(|_| pty_writer.flush()).is_err() {
                    break;
                }
            }
        });
        let t = tx.clone();
        thread::spawn(move || {
            let code = child.wait().map(|s| s.exit_code() as i32).unwrap_or(1);
            let _ = t.send(Msg::ChildExit(code));
        });
        sys::spawn_input(tx);
        if !typeahead.is_empty() {
            let _ = writer.send(typeahead);
        }

        let settings_path = crate::settings::settings_path().ok();
        let session_path = SessionCtx::path(&sid).ok();
        Ok(Session {
            rx,
            engine,
            writer,
            master: Some(pair.master),
            modes: m,
            term,
            tracker,
            menu,
            renderer,
            keymap,
            settings_mtime: mtime(&settings_path),
            settings_path,
            settings,
            session_mtime: None,
            session_path,
            terminal_bg,
            shell,
            req: 0,
            sent: None,
            changed_at: Instant::now(),
            cur: None,
            lock: None,
            ghost: None,
            llm: None,
            llm_req: 0,
            llm_at: None,
            resize_at: None,
            repaint: false,
            spinner: 0,
            spinner_at: None,
            poll_at: Instant::now() + FILE_POLL,
            exit_code: None,
            eof: false,
            exit_at: None,
        })
    }

    /// Main loop (SPEC §4.3); returns the child's exit code.
    fn run(mut self) -> i32 {
        let mut stash = None;
        loop {
            let msg = match stash.take() {
                Some(m) => Ok(m),
                None => match self.next_deadline() {
                    Some(d) => self.rx.recv_timeout(d.saturating_duration_since(Instant::now())),
                    None => self.rx.recv().map_err(|_| RecvTimeoutError::Disconnected),
                },
            };
            match msg {
                Ok(Msg::Stdin(b)) => self.on_input(&b),
                Ok(Msg::Pty(mut b)) => {
                    // One batch = everything already queued (one model pass, one stdout write).
                    loop {
                        match self.rx.try_recv() {
                            Ok(Msg::Pty(more)) => b.extend(more),
                            Ok(other) => {
                                stash = Some(other);
                                break;
                            }
                            Err(_) => break,
                        }
                    }
                    self.on_output(&b);
                }
                Ok(Msg::PtyEof) => {
                    self.eof = true;
                    if self.exit_code.is_some() {
                        break;
                    }
                }
                Ok(Msg::Resize(c, r)) => self.on_resize(c, r),
                Ok(Msg::ChildExit(c)) => {
                    self.exit_code = Some(c);
                    if cfg!(windows) {
                        self.master = None; // ConPTY does not EOF on child exit
                    }
                    if self.eof {
                        break;
                    }
                    self.exit_at = Some(Instant::now() + EXIT_GRACE);
                }
                Ok(Msg::Term) => {
                    self.exit_code = Some(129);
                    break;
                }
                Ok(Msg::Completion(c)) => self.on_completion(*c),
                Ok(Msg::Ghost { req, text }) => {
                    if req == self.req && req == self.llm_req {
                        self.llm = text.zip(self.sent.as_ref().map(|s| s.0.clone())).map(|(t, b)| (b, t));
                        self.redraw();
                    }
                }
                Err(RecvTimeoutError::Timeout) => {}
                Err(RecvTimeoutError::Disconnected) => break,
            }
            if self.exit_at.is_some_and(|d| d <= Instant::now()) {
                break;
            }
            self.on_timers();
        }
        self.cleanup();
        self.exit_code.unwrap_or(0)
    }

    fn next_deadline(&self) -> Option<Instant> {
        [self.resize_at, self.lock.as_ref().map(|l| l.0), self.llm_at, self.spinner_at, Some(self.poll_at), self.exit_at]
            .into_iter()
            .flatten()
            .min()
    }

    fn alt(&self) -> bool {
        self.term.mode().contains(TermMode::ALT_SCREEN)
    }

    fn cwd(&self) -> PathBuf {
        self.tracker.cwd.clone().or_else(|| std::env::current_dir().ok()).unwrap_or_default()
    }

    fn shell(&self) -> Shell {
        self.tracker.shell.unwrap_or(self.shell)
    }

    /// PTY batch (SPEC §4.5): clear, bytes (markers stripped), frame — one stdout write.
    fn on_output(&mut self, batch: &[u8]) {
        if self.resize_at.is_some() {
            self.resize_at = Some(Instant::now() + RESIZE_DEBOUNCE); // the shell / ConPTY is still redrawing
        }
        let was_alt = self.alt();
        let mut out = self.renderer.take_clear();
        self.tracker.feed(&mut self.term, batch, &mut out);
        for ev in std::mem::take(&mut self.tracker.events) {
            self.on_event(ev);
        }
        if was_alt && !self.alt() {
            self.menu.error_reset();
        }
        self.refresh();
        out.extend(self.take_repaint());
        out.extend(self.frame());
        write_stdout(&out);
    }

    fn on_event(&mut self, ev: Event) {
        match ev {
            Event::NewCmd(done) => {
                if let Some(h) = done {
                    let _ = self.engine.send(EngineMsg::CommandFinished(h));
                }
                self.sent = None;
                self.poll_files();
            }
            Event::PreExec => {}
        }
        self.menu.error_reset();
        self.ghost = None;
        self.llm = None;
        self.llm_at = None;
        self.lock = None;
    }

    /// Re-read the edit buffer; request completions when it changed (SPEC §4.5; held while insertion-locked).
    fn refresh(&mut self) {
        self.cur = if self.alt() { None } else { self.tracker.extract(&self.term) };
        if let Some((until, expected)) = &self.lock {
            let applied = self.cur.as_ref().is_some_and(|e| &e.buffer == expected);
            if !applied && Instant::now() < *until {
                return;
            }
            self.lock = None;
        }
        let Some(e) = &self.cur else { return };
        if self.sent.as_ref().is_some_and(|(b, c)| *b == e.buffer && *c == e.cursor) {
            return;
        }
        let (buffer, cursor) = (e.buffer.clone(), e.cursor);
        self.req += 1;
        self.sent = Some((buffer.clone(), cursor));
        self.changed_at = Instant::now();
        self.llm_at = None;
        if buffer.trim().is_empty() {
            self.menu.error_reset(); // empty buffer (SPEC §7.1); nothing to ask
            return;
        }
        let req = Request { id: self.req, buffer, cursor, cwd: self.cwd(), shell: self.shell(), history_mode: self.menu.history_mode() };
        let _ = self.engine.send(EngineMsg::Complete(req));
    }

    fn on_completion(&mut self, c: Completion) {
        if c.req != self.req {
            return; // only the latest request is served
        }
        if c.phase == Phase::Parse || c.ghost.is_some() {
            self.ghost = c.ghost.clone().map(|g| (c.buffer.clone(), g));
        }
        // LLM ghost: only when the local reply has no history match (SPEC §8.4).
        if c.phase == Phase::Parse && c.ghost.is_none() && !c.error && c.cursor == c.buffer.len() && self.llm_enabled() {
            let debounce = self.settings.i64("inline.debounceMs", 300).max(0) as u64;
            self.llm_at = Some(self.changed_at + Duration::from_millis(debounce));
        }
        if c.loading && self.spinner_at.is_none() {
            self.spinner_at = Some(Instant::now() + SPINNER_TICK);
        }
        self.menu.on_completion(c, &self.settings);
        self.redraw();
    }

    fn llm_enabled(&self) -> bool {
        self.settings.bool("inline.enabled", true)
            && matches!(crate::inline::source(&self.settings), Ok(crate::inline::Source::Profile(_)))
    }

    /// Ghost text to draw (SPEC §8.1): survives typing that matches its start.
    fn ghost_text(&self) -> Option<String> {
        if !self.settings.bool("inline.enabled", true) || self.tracker.preexec || self.alt() {
            return None;
        }
        let e = self.cur.as_ref()?;
        if e.buffer.is_empty() || e.cursor != e.buffer.len() || e.shell_ghost {
            return None;
        }
        [&self.ghost, &self.llm].into_iter().flatten().find_map(|(b, g)| {
            let rest = g.strip_prefix(e.buffer.strip_prefix(b.as_str())?)?;
            (!rest.is_empty() && !rest.contains('\n')).then(|| rest.to_string())
        })
    }

    fn suppressed(&self) -> bool {
        !self.tracker.at_prompt() || self.resize_at.is_some() || self.repaint
    }

    fn frame(&mut self) -> Vec<u8> {
        let ghost = self.ghost_text();
        let input = FrameInput {
            settings: &self.settings,
            ghost: ghost.as_deref(),
            spinner: self.spinner,
            suppress: self.suppressed(),
            scroll_region: self.tracker.filter.scroll_region(),
        };
        self.renderer.frame(&self.term, &self.menu, &input)
    }

    /// State-only change: clear + frame in one write (SPEC §7.5.3).
    fn redraw(&mut self) {
        let mut out = self.renderer.take_clear();
        out.extend(self.frame());
        write_stdout(&out);
    }

    /// Stdin chunk (SPEC §7.2.4): intercept bound keys while the menu is shown, accept ghost text, else forward.
    fn on_input(&mut self, chunk: &[u8]) {
        let mut fwd = Vec::new();
        let (mut redraw, mut rerequest) = (false, false);
        let only_tab = self.settings.bool("autocomplete.onlyShowOnTab", false);
        for ev in keys::decode(chunk) {
            let key = ev.key;
            let prompt = !self.tracker.preexec && !self.alt();
            let intercepting = prompt && self.menu.intercepting(&self.settings);
            let global = prompt && !intercepting && self.menu.has_items();
            let binding = if prompt { self.keymap.lookup(&key) } else { None };
            let effects = match binding {
                Some(Binding::Action(a)) if intercepting => self.menu.handle(a, &self.settings),
                Some(Binding::Unknown) if intercepting => Vec::new(),
                _ if global && only_tab && key.code == KeyCode::Tab && key.mods == Mods::default() => {
                    self.menu.show_on_tab(&self.settings)
                }
                Some(Binding::Action(a @ (Action::ShowAutocomplete | Action::ToggleAutocomplete))) if global => {
                    self.menu.handle(a, &self.settings)
                }
                _ => {
                    match ghost_key(&key).zip(self.ghost_text()) {
                        Some((full, g)) => {
                            fwd.extend_from_slice(if full { g.as_str() } else { next_word(&g) }.as_bytes());
                            self.ghost = None;
                            self.llm = None;
                            redraw = true;
                        }
                        None => {
                            fwd.extend_from_slice(&chunk[ev.bytes.clone()]);
                            if is_ctrl(&key, 'c') || is_ctrl(&key, 'd') {
                                self.menu.error_reset();
                                redraw = true;
                            }
                            if key.code == KeyCode::Backspace {
                                self.menu.note_backspace();
                            }
                        }
                    }
                    continue;
                }
            };
            redraw = true;
            for e in effects {
                match e {
                    Effect::Insert { ins, record, .. } => self.insert(&ins, record, &mut fwd),
                    Effect::Write(b) => fwd.extend(b),
                    Effect::Bell => write_stdout(b"\x07"),
                    Effect::Rerequest => rerequest = true,
                    Effect::ToggleFuzzy => {
                        let _ = self.engine.send(EngineMsg::ToggleFuzzy);
                        rerequest = true;
                    }
                }
            }
        }
        if !fwd.is_empty() {
            let _ = self.writer.send(fwd);
        }
        if rerequest {
            self.sent = None;
            self.refresh();
        }
        if redraw {
            self.redraw();
        }
    }

    /// Insertion with reconciliation + lock (SPEC §7.3). Dropped while a command runs.
    fn insert(&mut self, ins: &Insertion, record: Option<(String, String)>, fwd: &mut Vec<u8>) {
        if self.tracker.preexec {
            return;
        }
        let from = self.menu.last().map(|c| (c.buffer.clone(), c.cursor));
        let Some((ib, at)) = from.or_else(|| self.cur.as_ref().map(|e| (e.buffer.clone(), e.cursor))) else { return };
        let cur = self.cur.as_ref().map_or(ib.as_str(), |e| e.buffer.as_str());
        fwd.extend(insertion_bytes(ins, &ib, cur));
        self.lock = Some((Instant::now() + INSERT_LOCK, expected_buffer(ins, &ib, at)));
        if let Some((cmd, name)) = record {
            let _ = self.engine.send(EngineMsg::RecordInsert { cmd, name });
        }
    }

    /// SPEC §7.6: resize PTY + model now, repaint after 150 ms of quiet.
    fn on_resize(&mut self, cols: u16, rows: u16) {
        if cols == 0 || rows == 0 || (cols as usize, rows as usize) == (self.term.columns(), self.term.screen_lines()) {
            return;
        }
        if let Some(m) = &self.master {
            let _ = m.resize(PtySize { rows, cols, pixel_width: 0, pixel_height: 0 });
        }
        self.term.resize(TermSize::new(cols as usize, rows as usize));
        self.tracker.filter.resize(rows as usize);
        self.renderer.drop_clear(); // the terminal reflowed the old frame unpredictably
        self.resize_at = Some(Instant::now() + RESIZE_DEBOUNCE);
    }

    /// Full repaint once the resize settled (postponed while on the alternate screen).
    fn take_repaint(&mut self) -> Vec<u8> {
        if !self.repaint || self.resize_at.is_some() || self.alt() {
            return Vec::new();
        }
        self.repaint = false;
        self.renderer.full_repaint(&self.term)
    }

    fn on_timers(&mut self) {
        let now = Instant::now();
        let due = |d: Option<Instant>| d.is_some_and(|d| d <= now);
        let mut redraw = false;
        if due(self.resize_at) {
            self.resize_at = None;
            self.repaint = true;
        }
        if self.lock.as_ref().is_some_and(|l| l.0 <= now) {
            self.refresh(); // releases the lock and catches up with the buffer
            redraw = true;
        }
        if due(self.llm_at) {
            self.llm_at = None;
            if let Some((buffer, _)) = self.sent.clone() {
                self.llm_req = self.req;
                let msg = EngineMsg::Llm { req: self.req, buffer, cwd: self.cwd(), shell: self.shell() };
                let _ = self.engine.send(msg);
            }
        }
        if due(self.spinner_at) {
            self.spinner_at = None;
            if self.menu.last().is_some_and(|c| c.loading) {
                self.spinner += 1;
                self.spinner_at = Some(now + SPINNER_TICK);
                redraw |= self.menu.loading();
            }
        }
        if now >= self.poll_at {
            self.poll_at = now + FILE_POLL;
            redraw |= self.poll_files();
        }
        let repaint = self.take_repaint();
        if !repaint.is_empty() {
            let mut out = repaint;
            out.extend(self.frame());
            write_stdout(&out);
        } else if redraw {
            self.redraw();
        }
    }

    /// Settings (live reload) and the session env file from `_ pre-cmd` (D17). True when settings changed.
    fn poll_files(&mut self) -> bool {
        let m = mtime(&self.session_path);
        if m.is_some() && m != self.session_mtime {
            self.session_mtime = m;
            if let Some(ctx) = self.session_path.as_deref().and_then(SessionCtx::load) {
                let _ = self.engine.send(EngineMsg::Session(ctx));
            }
        }
        let m = mtime(&self.settings_path);
        if m == self.settings_mtime {
            return false;
        }
        self.settings_mtime = m;
        self.settings = Settings::load();
        self.keymap = Keymap::from_settings(&self.settings);
        self.renderer.set_theme(Theme::from_settings(&self.settings, self.terminal_bg));
        let _ = self.engine.send(EngineMsg::Settings(self.settings.clone()));
        true
    }

    /// SPEC §4.4 step 10.
    fn cleanup(&mut self) {
        let mut out = self.renderer.take_clear();
        out.extend_from_slice(b"\x1b[0m\x1b[?25h");
        write_stdout(&out);
        let _ = self.engine.send(EngineMsg::Shutdown);
        self.modes.restore();
        if let Some(p) = &self.session_path {
            let _ = std::fs::remove_file(p);
        }
    }
}

#[cfg(unix)]
mod sys {
    use crate::types::Msg;
    use anyhow::Result;
    use std::io::Read;
    use std::sync::mpsc::Sender;
    use std::time::Duration;

    #[derive(Clone, Copy)]
    pub struct Modes;

    impl Modes {
        pub fn restore(&self) {
            let _ = crossterm::terminal::disable_raw_mode();
        }
    }

    /// Raw mode (cfmakeraw, TCSANOW: ISIG off so Ctrl-C/Z become bytes). Bare LF never needs rewriting here.
    pub fn enter() -> Result<(Modes, bool)> {
        crossterm::terminal::enable_raw_mode()?;
        Ok((Modes, false))
    }

    /// One read from stdin, waiting at most `t` (startup probe). `select`, since macOS `poll` rejects devices.
    pub fn read_timeout(t: Duration) -> Vec<u8> {
        let mut buf = [0u8; 1024];
        // SAFETY: plain libc calls on fd 0 with stack-owned buffers.
        let n = unsafe {
            let mut set: libc::fd_set = std::mem::zeroed();
            libc::FD_ZERO(&mut set);
            libc::FD_SET(0, &mut set);
            let us = t.as_micros();
            let mut tv = libc::timeval { tv_sec: (us / 1_000_000) as _, tv_usec: (us % 1_000_000) as _ };
            if libc::select(1, &mut set, std::ptr::null_mut(), std::ptr::null_mut(), &mut tv) <= 0 {
                return Vec::new();
            }
            libc::read(0, buf.as_mut_ptr().cast(), buf.len())
        };
        if n > 0 {
            buf[..n as usize].to_vec()
        } else {
            Vec::new()
        }
    }

    pub fn spawn_input(tx: Sender<Msg>) {
        std::thread::spawn(move || {
            let mut buf = [0u8; 4096];
            let mut stdin = std::io::stdin();
            loop {
                match stdin.read(&mut buf) {
                    Ok(0) => break,
                    Ok(n) => {
                        if tx.send(Msg::Stdin(buf[..n].to_vec())).is_err() {
                            break;
                        }
                    }
                    Err(e) if e.kind() == std::io::ErrorKind::Interrupted => {}
                    Err(_) => break,
                }
            }
        });
    }

    /// SIGWINCH → Resize, SIGTERM/SIGHUP → Term.
    pub fn spawn_resize(tx: Sender<Msg>) -> Result<()> {
        use signal_hook::consts::signal::{SIGHUP, SIGTERM, SIGWINCH};
        let mut signals = signal_hook::iterator::Signals::new([SIGWINCH, SIGTERM, SIGHUP])?;
        std::thread::spawn(move || {
            for sig in signals.forever() {
                let msg = match sig {
                    SIGWINCH => match crossterm::terminal::size() {
                        Ok((c, r)) => Msg::Resize(c, r),
                        Err(_) => continue,
                    },
                    _ => Msg::Term,
                };
                if tx.send(msg).is_err() {
                    break;
                }
            }
        });
        Ok(())
    }
}

#[cfg(windows)]
mod sys {
    use crate::types::Msg;
    use anyhow::{bail, Result};
    use std::sync::mpsc::Sender;
    use std::time::Duration;
    use windows_sys::Win32::Foundation::{HANDLE, WAIT_OBJECT_0};
    use windows_sys::Win32::System::Console::{
        GetConsoleCP, GetConsoleMode, GetConsoleOutputCP, GetStdHandle, ReadConsoleInputW, SetConsoleCP, SetConsoleMode,
        SetConsoleOutputCP, DISABLE_NEWLINE_AUTO_RETURN, ENABLE_ECHO_INPUT, ENABLE_LINE_INPUT, ENABLE_PROCESSED_INPUT,
        ENABLE_VIRTUAL_TERMINAL_INPUT, ENABLE_VIRTUAL_TERMINAL_PROCESSING, ENABLE_WINDOW_INPUT, INPUT_RECORD, KEY_EVENT,
        STD_INPUT_HANDLE, STD_OUTPUT_HANDLE, WINDOW_BUFFER_SIZE_EVENT,
    };
    use windows_sys::Win32::System::Threading::WaitForSingleObject;

    const CP_UTF8: u32 = 65001;

    /// Saved console state, restored on exit.
    #[derive(Clone, Copy)]
    pub struct Modes {
        input: u32,
        output: u32,
        in_cp: u32,
        out_cp: u32,
    }

    fn handles() -> (HANDLE, HANDLE) {
        // SAFETY: GetStdHandle has no preconditions.
        unsafe { (GetStdHandle(STD_INPUT_HANDLE), GetStdHandle(STD_OUTPUT_HANDLE)) }
    }

    impl Modes {
        pub fn restore(&self) {
            let (i, o) = handles();
            // SAFETY: plain console calls with values previously read from the same handles.
            unsafe {
                SetConsoleMode(i, self.input);
                SetConsoleMode(o, self.output);
                SetConsoleCP(self.in_cp);
                SetConsoleOutputCP(self.out_cp);
            }
        }
    }

    /// Console modes (SPEC §4.4 step 4). Returns whether bare LF must be rewritten to `ESC D`
    /// (`DISABLE_NEWLINE_AUTO_RETURN` unsupported).
    pub fn enter() -> Result<(Modes, bool)> {
        let (i, o) = handles();
        let (mut input, mut output) = (0u32, 0u32);
        // SAFETY: plain console calls; out-params point to locals.
        unsafe {
            if GetConsoleMode(i, &mut input) == 0 || GetConsoleMode(o, &mut output) == 0 {
                bail!("not a Windows console");
            }
            let m = Modes { input, output, in_cp: GetConsoleCP(), out_cp: GetConsoleOutputCP() };
            let raw = (input & !(ENABLE_ECHO_INPUT | ENABLE_LINE_INPUT | ENABLE_PROCESSED_INPUT))
                | ENABLE_VIRTUAL_TERMINAL_INPUT
                | ENABLE_WINDOW_INPUT;
            if SetConsoleMode(i, raw) == 0 {
                bail!("cannot set the console input mode");
            }
            let vt = output | ENABLE_VIRTUAL_TERMINAL_PROCESSING;
            let lf_to_ind = SetConsoleMode(o, vt | DISABLE_NEWLINE_AUTO_RETURN) == 0;
            if lf_to_ind && SetConsoleMode(o, vt) == 0 {
                m.restore();
                bail!("the console has no VT support (Windows 10 1809+ required)");
            }
            SetConsoleCP(CP_UTF8);
            SetConsoleOutputCP(CP_UTF8);
            Ok((m, lf_to_ind))
        }
    }

    /// Read input records: key-down chars → UTF-8 (surrogate pairs joined across reads), plus "a resize happened".
    fn read_console(h: HANDLE, high: &mut Option<u16>) -> Option<(Vec<u8>, bool)> {
        // SAFETY: INPUT_RECORD is plain old data; ReadConsoleInputW fills at most `len` records.
        let mut recs: [INPUT_RECORD; 64] = unsafe { std::mem::zeroed() };
        let mut n = 0u32;
        if unsafe { ReadConsoleInputW(h, recs.as_mut_ptr(), recs.len() as u32, &mut n) } == 0 {
            return None;
        }
        let mut units: Vec<u16> = high.take().into_iter().collect();
        let mut resized = false;
        for r in &recs[..n as usize] {
            match r.EventType as u32 {
                KEY_EVENT => {
                    // SAFETY: EventType says the union holds a KEY_EVENT_RECORD.
                    let k = unsafe { r.Event.KeyEvent };
                    let ch = unsafe { k.uChar.UnicodeChar };
                    if k.bKeyDown != 0 && ch != 0 {
                        units.extend(std::iter::repeat_n(ch, k.wRepeatCount.max(1) as usize));
                    }
                }
                WINDOW_BUFFER_SIZE_EVENT => resized = true,
                _ => {}
            }
        }
        if units.last().is_some_and(|u| (0xd800..0xdc00).contains(u)) {
            *high = units.pop();
        }
        Some((String::from_utf16_lossy(&units).into_bytes(), resized))
    }

    /// Input available within `t` (startup probe), read without a thread so a fallback shell keeps its input.
    pub fn read_timeout(t: Duration) -> Vec<u8> {
        let (i, _) = handles();
        // SAFETY: waiting on the console input handle.
        if unsafe { WaitForSingleObject(i, t.as_millis() as u32) } != WAIT_OBJECT_0 {
            return Vec::new();
        }
        read_console(i, &mut None).map(|(b, _)| b).unwrap_or_default()
    }

    pub fn spawn_input(tx: Sender<Msg>) {
        std::thread::spawn(move || {
            let (i, _) = handles();
            let mut high = None;
            while let Some((bytes, resized)) = read_console(i, &mut high) {
                if !bytes.is_empty() && tx.send(Msg::Stdin(bytes)).is_err() {
                    break;
                }
                if resized {
                    if let Ok((c, r)) = crossterm::terminal::size() {
                        let _ = tx.send(Msg::Resize(c, r));
                    }
                }
            }
        });
    }

    /// No SIGWINCH on Windows: also poll the size every 250 ms (legacy conhost may not send the event).
    pub fn spawn_resize(tx: Sender<Msg>) -> Result<()> {
        std::thread::spawn(move || {
            let mut last = crossterm::terminal::size().ok();
            loop {
                std::thread::sleep(Duration::from_millis(250));
                let now = crossterm::terminal::size().ok();
                if now != last {
                    last = now;
                    if let Some((c, r)) = now {
                        if tx.send(Msg::Resize(c, r)).is_err() {
                            break;
                        }
                    }
                }
            }
        });
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ins(delete: usize, text: &str, cursor_back: usize, execute: bool) -> Insertion {
        Insertion { delete, text: text.into(), cursor_back, execute }
    }

    #[test]
    fn insertion_bytes_reconcile() {
        let i = ins(0, "atus ", 0, false);
        assert_eq!(insertion_bytes(&i, "git st", "git st"), b"atus ");
        // Shell ahead of the reply (user typed `a` meanwhile): erase the extra chars first.
        assert_eq!(insertion_bytes(&i, "git st", "git sta"), b"\x7fatus ");
        assert_eq!(insertion_bytes(&i, "git st", "git stéé"), b"\x7f\x7fatus ");
        // Shell behind (echo pending): retype the missing tail first.
        assert_eq!(insertion_bytes(&i, "git st", "git s"), b"tatus ");
        // Diverged: no reconciliation.
        assert_eq!(insertion_bytes(&i, "git st", "ls"), b"atus ");
        // Delete, cursor-back and execute.
        let i = ins(3, "--out=", 1, true);
        assert_eq!(insertion_bytes(&i, "x --o", "x --o"), b"\x7f\x7f\x7f--out=\x1b[D\r");
    }

    #[test]
    fn expected_buffer_after_insertion() {
        assert_eq!(expected_buffer(&ins(0, "atus ", 0, false), "git st", 6), "git status ");
        assert_eq!(expected_buffer(&ins(6, "my\\ dir/", 0, false), "cd my\\ di", 9), "cd my\\ dir/");
        assert_eq!(expected_buffer(&ins(2, "checkout ", 0, false), "git ch | x", 6), "git checkout  | x");
        assert_eq!(expected_buffer(&ins(1, "é", 0, false), "aé", 3), "aé");
    }

    #[test]
    fn probe_replies() {
        let (cpr, bg, rest) = parse_probe(b"ab\x1b]11;rgb:ffff/8080/0000\x1b\\\x1b[12;40Rc");
        assert_eq!((cpr, bg, rest.as_slice()), (Some((12, 40)), Some((255, 128, 0)), b"abc".as_slice()));
        let (cpr, bg, rest) = parse_probe(b"\x1b]11;rgb:f/0/f\x07\x1b[1;1R");
        assert_eq!((cpr, bg, rest.len()), (Some((1, 1)), Some((255, 0, 255)), 0));
        // Keys typed meanwhile (arrows, partial replies) are kept for the shell.
        let (cpr, _, rest) = parse_probe(b"\x1b[A\x1b[12;");
        assert_eq!((cpr, rest.as_slice()), (None, b"\x1b[A\x1b[12;".as_slice()));
    }

    #[test]
    fn child_argv() {
        let s = |v: &[&str]| v.iter().map(|x| x.to_string()).collect::<Vec<_>>();
        assert_eq!(argv(Some("/bin/zsh"), &[], true), s(&["/bin/zsh", "--login"]));
        assert_eq!(argv(Some("pwsh"), &[], true), s(&["pwsh"]));
        assert_eq!(argv(Some("bash"), &s(&["--noprofile", "--rcfile", "x"]), false), s(&["bash", "--noprofile", "--rcfile", "x"]));
        assert_eq!(argv(Some("bash"), &s(&["vim", "-u"]), true), s(&["vim", "-u"]));
        assert!(argv(None, &[], false).is_empty());
    }

    #[test]
    fn ghost_keys_and_words() {
        let k = |code, mods| Key { code, mods };
        let ctrl = Mods { ctrl: true, ..Mods::default() };
        let alt = Mods { alt: true, ..Mods::default() };
        assert_eq!(ghost_key(&k(KeyCode::Right, Mods::default())), Some(true));
        assert_eq!(ghost_key(&k(KeyCode::Char('e'), ctrl)), Some(true));
        assert_eq!(ghost_key(&k(KeyCode::Char('f'), alt)), Some(false));
        assert_eq!(ghost_key(&k(KeyCode::Right, ctrl)), Some(false));
        assert_eq!(ghost_key(&k(KeyCode::Tab, Mods::default())), None);
        assert_eq!(next_word(" -la /tmp"), " -la");
        assert_eq!(next_word("ho hi"), "ho");
        assert_eq!(next_word("x"), "x");
    }
}
