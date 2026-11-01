//! `tabsmith _ pty`: the PTY wrapper session (SPEC §4.3–4.5, §7.2.4, §7.3, §7.6, §8).
//! OWNER: wrapper agent.
use crate::ghost_text::domain::value_objects::ghost_text::GhostText;
use crate::ghost_text::domain::value_objects::ghost_text_source::GhostTextSource;
use crate::dropdown::domain::value_objects::key::Key;
use crate::dropdown::domain::value_objects::key_action::KeyAction;
use crate::dropdown::domain::value_objects::key_binding::KeyBinding;
use crate::dropdown::domain::value_objects::key_code::KeyCode;
use crate::dropdown::domain::value_objects::keymap::Keymap;
use crate::dropdown::domain::value_objects::modifiers::Modifiers;
use crate::shared::infrastructure::logger;
use crate::dropdown::domain::entities::menu::Menu;
use crate::dropdown::domain::value_objects::menu_effect::MenuEffect;
use crate::dropdown::domain::repositories::theme_repository::ThemeRepository;
use crate::dropdown::domain::services::theme_resolver::resolve_theme;
use crate::dropdown::infrastructure::adapters::file_theme_repository::FileThemeRepository;
use crate::terminal_session::application::frame_input::FrameInput;
use crate::terminal_session::infrastructure::adapters::overlay_renderer::OverlayRenderer;
use crate::settings::domain::services::settings_catalog::{
    AUTOCOMPLETE_DISABLE, AUTOCOMPLETE_ONLY_SHOW_ON_TAB, INLINE_DEBOUNCE_MS, INLINE_ENABLED,
};
use crate::settings::domain::repositories::settings_repository::SettingsRepository;
use crate::settings::infrastructure::adapters::json_file_settings_repository::JsonFileSettingsRepository;
use crate::settings::infrastructure::load_or_defaults::load_or_defaults;
use crate::settings::domain::value_objects::settings::Settings;
use crate::shared::infrastructure::data_directory_from_env::{data_directory_from_env, user_home_from_env};
use crate::terminal_session::domain::value_objects::edit_buffer::EditBuffer;
use crate::terminal_session::domain::value_objects::shell_event::ShellEvent;
use crate::tracker::Tracker;
use crate::shared::domain::value_objects::shell::Shell;
use crate::terminal_session::domain::repositories::session_context_repository::SessionContextRepository;
use crate::terminal_session::domain::value_objects::session_id::SessionId;
use crate::terminal_session::infrastructure::adapters::file_session_context_repository::FileSessionContextRepository;
use crate::terminal_session::infrastructure::adapters::kitty_graphics;
use crate::terminal_session::infrastructure::adapters::stdin_key_decoder::StdinKeyDecoder;
use crate::terminal_session::application::ports::completion_engine::CompletionEngine;
use crate::completion::application::completion_request::CompletionRequest;
use crate::completion::domain::value_objects::insertion::Insertion;
use crate::completion::domain::value_objects::completion_phase::CompletionPhase;
use crate::completion::domain::value_objects::completion_result::CompletionResult;
use crate::types::{
    Model, Msg,
};
use alacritty_terminal::event::VoidListener;
use alacritty_terminal::grid::Dimensions;
use alacritty_terminal::term::test::TermSize;
use alacritty_terminal::term::{Config, Term, TermMode};
use anyhow::{anyhow, bail, Context, Result};
use portable_pty::{CommandBuilder, MasterPty, PtySize};
use std::io::{IsTerminal, Read, Write};
use std::panic::AssertUnwindSafe;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender, SyncSender};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

const RESIZE_DEBOUNCE: Duration = Duration::from_millis(150);
const INSERT_LOCK: Duration = Duration::from_millis(100);
/// A shell that has echoed nothing of an insertion keeps it in the basis this long at most.
const LOCK_CAP: Duration = Duration::from_secs(1);
/// A prompt redraw whose line arrives in a later read: its empty line is not acted on before this.
const REDRAW_GRACE: Duration = Duration::from_millis(50);
const SPINNER_TICK: Duration = Duration::from_millis(80);
const FILE_POLL: Duration = Duration::from_secs(2);
const PROBE_TIMEOUT: Duration = Duration::from_millis(300);
/// Startup probe: background colour (`system` theme, also when chosen later) and cursor position (SPEC §4.4 step 5).
const PROBE: &[u8] = b"\x1b]11;?\x1b\\\x1b[6n";
/// After a probe timeout, a late reply arriving within this window is dropped instead of typed into the shell.
const LATE_PROBE: Duration = Duration::from_secs(3);
/// PTY chunks in flight between the reader thread and the main loop (backpressure for a flooding child).
const PTY_CREDITS: usize = 4;
/// After the child exits, wait this long for the rest of its output (PTY EOF) before leaving.
const EXIT_GRACE: Duration = Duration::from_millis(200);

/// Starts the completion engine, its replies going to the session's inbox; called once, inside `Session::start`.
pub(crate) type CompletionEngineStarter = fn(Sender<Msg>, Settings) -> Rc<dyn CompletionEngine>;

/// Run the wrapper; returns the exit code to exit with. Never leaves the user without a shell (fallback, SPEC §4.4).
pub fn run(shell: Option<String>, cmd: Vec<String>, start: CompletionEngineStarter) -> i32 {
    // Panics are logged, never printed over the user's screen; the session then falls back to a plain shell.
    std::panic::set_hook(Box::new(|info| logger::error(&format!("panic: {info}"))));
    let shell = shell
        .or_else(|| env_nonempty("TABSMITH_SHELL"))
        .or_else(|| {
            if cfg!(windows) {
                Some("pwsh".into())
            } else {
                env_nonempty("SHELL")
            }
        });
    let login = std::env::var("TABSMITH_IS_LOGIN_SHELL").as_deref() == Ok("1");
    let argv = argv(shell.as_deref(), &cmd, login);
    let mut modes = None;
    let res = std::panic::catch_unwind(AssertUnwindSafe(|| {
        Session::start(&argv, &mut modes, start).map(Session::run)
    }));
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
    let Some(shell) = shell else {
        return Vec::new();
    };
    let mut v = vec![shell.to_string()];
    if login
        && matches!(
            Shell::from_name(shell),
            Some(Shell::Bash | Shell::Zsh | Shell::Fish)
        )
    {
        v.push("--login".into());
    }
    v.extend(extra.iter().cloned());
    v
}

/// Run the plain shell with `TABSMITH_TERM_DISABLED=1` so its pre block does not loop (SPEC §4.4 fallback).
fn fallback(err: &anyhow::Error, argv: &[String]) -> i32 {
    logger::error(&format!("pty: {err:#}"));
    eprintln!("tabsmith had an error: {err:#}");
    crate::specs::infrastructure::adapters::process_executor::kill_live_children(); // exec skips atexit

    let argv = if argv.is_empty() {
        vec![if cfg!(windows) { "pwsh" } else { "/bin/sh" }.to_string()]
    } else {
        argv.to_vec()
    };
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
    b.extend(std::iter::repeat_n(0x7f, ins.delete()));
    b.extend_from_slice(ins.text().as_bytes());
    for _ in 0..ins.cursor_back() {
        b.extend_from_slice(b"\x1b[D");
    }
    if ins.executes() {
        b.push(b'\r');
    }
    b
}

/// Buffer expected once the shell applied an insertion (insertion-lock release, SPEC §7.3).
/// `cursor` = byte offset into `ib` the reply was computed at.
pub fn expected_buffer(ins: &Insertion, ib: &str, cursor: usize) -> String {
    let cursor = if ib.is_char_boundary(cursor) {
        cursor
    } else {
        ib.len()
    };
    let mut s = ib[..cursor].to_string();
    for _ in 0..ins.delete() {
        s.pop();
    }
    s.push_str(ins.text());
    s.push_str(&ib[cursor..]);
    s
}

/// Ghost accept keys (SPEC §8.3): `Some(true)` = whole ghost, `Some(false)` = next word.
fn ghost_key(k: &Key) -> Option<bool> {
    let m = k.modifiers();
    let plain = m == Modifiers::default();
    let ctrl = m == Modifiers::none().with_ctrl(true);
    let alt = m == Modifiers::none().with_alt(true);
    match k.code() {
        KeyCode::Right | KeyCode::End if plain => Some(true),
        KeyCode::Char('f' | 'F' | 'e' | 'E') if ctrl => Some(true),
        KeyCode::Char('f') if alt => Some(false),
        KeyCode::Right if ctrl => Some(false),
        _ => None,
    }
}

/// Keys typed ahead that the extracted buffer `new` does not show yet. `base` = the buffer they were typed after
/// (the pending insertion's result while locked); `in_flight` = that insertion is not echoed yet.
fn unechoed_after(
    unechoed: &str,
    base: Option<&str>,
    new: Option<&str>,
    in_flight: bool,
) -> String {
    match (base, new) {
        (Some(b), Some(n)) => match n.strip_prefix(b) {
            Some(added) => unechoed.strip_prefix(added).unwrap_or("").to_string(),
            None if in_flight => unechoed.to_string(),
            None => String::new(),
        },
        _ => String::new(),
    }
}

/// PTY reader (SPEC §4.3). Every read takes a credit, returned once the main loop handled the chunk, so at most
/// `PTY_CREDITS` chunks are queued: a flooding child feels backpressure and Ctrl-C is never stuck behind its output.
fn pump(mut reader: impl Read, credits: Receiver<()>, tx: Sender<Msg>) {
    let mut buf = vec![0u8; 16 * 1024];
    while credits.recv().is_ok() {
        let n = loop {
            match reader.read(&mut buf) {
                Err(e) if e.kind() == std::io::ErrorKind::Interrupted => {}
                r => break r.unwrap_or(0), // Linux: EIO once the slave side is gone
            }
        };
        if n == 0 || tx.send(Msg::Pty(buf[..n].to_vec())).is_err() {
            break;
        }
    }
    let _ = tx.send(Msg::PtyEof);
}

fn is_ctrl(k: &Key, c: char) -> bool {
    k.modifiers().ctrl() && matches!(k.code(), KeyCode::Char(x) if x.eq_ignore_ascii_case(&c))
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
    let end = body
        .iter()
        .position(|b| !(b.is_ascii_digit() || *b == b';'))?;
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
    let term_len = if body[end] == 0x07 {
        1
    } else if body.get(end + 1) == Some(&b'\\') {
        2
    } else {
        return None;
    };
    let text = std::str::from_utf8(&body[..end]).ok()?;
    let mut parts = text
        .strip_prefix("rgb:")
        .into_iter()
        .flat_map(|t| t.split('/'))
        .map(|h| {
            let v = u32::from_str_radix(h, 16)
                .ok()
                .filter(|_| (1..=4).contains(&h.len()))?;
            Some((v * 255 / ((1 << (4 * h.len())) - 1)) as u8)
        });
    let rgb = (|| Some((parts.next()??, parts.next()??, parts.next()??)))();
    Some((rgb, 5 + end + term_len))
}

fn session_id() -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0);
    format!(
        "{:016x}{:016x}",
        nanos,
        std::process::id() as u64 ^ nanos.rotate_left(17)
    )
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
    engine: Rc<dyn CompletionEngine>,
    writer: Sender<Vec<u8>>,
    /// Returns PTY read credits to the reader thread (see `pump`).
    credits: SyncSender<()>,
    /// Dropped after the child exits on Windows to unblock the ConPTY reader.
    master: Option<Box<dyn MasterPty + Send>>,
    modes: sys::Modes,
    term: Model,
    tracker: Tracker,
    menu: Menu,
    renderer: OverlayRenderer,
    /// Where `autocomplete.theme` finds a custom theme file; read again at every theme resolution, never cached (P8-T25).
    themes: Rc<dyn ThemeRepository>,
    keymap: Keymap,
    /// Kept across stdin reads: a bracketed paste split over several reads stays one opaque unit (SPEC §7.2.1).
    decoder: StdinKeyDecoder,
    settings: Settings,
    settings_repository: Box<dyn SettingsRepository>,
    settings_mtime: Option<SystemTime>,
    session_id: Option<SessionId>,
    session_contexts: Box<dyn SessionContextRepository>,
    session_mtime: Option<SystemTime>,
    terminal_bg: Option<Rgb>,
    /// Shell of the spawned program (the tracker's `Shell=` wins once known).
    shell: Shell,
    /// Latest completion request id and the buffer/cursor it was sent for.
    req: u64,
    sent: Option<(String, usize)>,
    changed_at: Instant,
    /// Edit buffer at the last batch.
    cur: Option<EditBuffer>,
    /// Insertion lock (SPEC §7.3): (deadline, cap, expected buffer, buffer before it). Re-armed up to the cap while
    /// the shell has echoed nothing of the insertion (a slow shell).
    lock: Option<(Instant, Instant, String, String)>,
    /// Printable keys forwarded but not yet echoed (reconciliation basis, SPEC §7.3).
    unechoed: String,
    /// `req` when a key the echo prediction cannot model was forwarded; cleared by a reply for a later request.
    echo_pending: Option<u64>,
    /// Keys that would act on the menu or ghost while `echo_pending`: replayed once the reply came, or at `held_at`.
    held: Vec<u8>,
    held_at: Option<Instant>,
    /// A prompt redraw is in progress (see `REDRAW_GRACE`).
    redraw_until: Option<Instant>,
    /// Ghost candidates (`GhostText`: the buffer each was computed for and its text): local (engine) and LLM.
    ghost: Option<GhostText>,
    llm: Option<GhostText>,
    llm_req: u64,
    llm_at: Option<Instant>,
    /// The startup probe timed out: drop its late reply from stdin until then.
    probe_until: Option<Instant>,
    resize_at: Option<Instant>,
    repaint: bool,
    spinner: usize,
    spinner_at: Option<Instant>,
    poll_at: Instant,
    exit_code: Option<i32>,
    eof: bool,
    exit_at: Option<Instant>,
    /// After a panic in a handler: plain pass-through until the child exits (see `guard`).
    passthrough: bool,
    /// Test builds: stdout bytes are captured here instead of written.
    sink: Vec<u8>,
}

impl Session {
    /// SPEC §4.4 steps 1–9. `modes` receives the saved terminal modes as soon as they change.
    fn start(argv: &[String], modes: &mut Option<sys::Modes>, start: CompletionEngineStarter) -> Result<Session> {
        if !std::io::stdin().is_terminal() || !std::io::stdout().is_terminal() {
            bail!("stdin/stdout is not a terminal");
        }
        let program = argv
            .first()
            .context("no shell to run (set SHELL or pass --shell)")?;
        let settings_repository: Box<dyn SettingsRepository> =
            Box::new(JsonFileSettingsRepository::new(data_directory_from_env()));
        let settings = load_or_defaults(&*settings_repository);
        let (m, lf_to_ind) = sys::enter()?;
        *modes = Some(m);
        let (cols, rows) = crossterm::terminal::size().context("terminal size")?;
        let (cols, rows) = (cols.max(1), rows.max(1));
        let mut term = Term::new(
            Config {
                scrolling_history: 2000,
                ..Config::default()
            },
            &TermSize::new(cols as usize, rows as usize),
            VoidListener,
        );
        let sid = session_id();
        let shell = Shell::from_name(program).unwrap_or(if cfg!(windows) {
            Shell::Pwsh
        } else {
            Shell::Bash
        });
        let mut tracker = Tracker::new(sid.clone(), Shell::from_name(program), rows as usize);
        tracker.filter.lf_to_ind = lf_to_ind;

        // Sync the model with the real cursor; always ask for the background, the theme may become `system` later.
        write_stdout(PROBE);
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
        // ponytail: a reply cut by the timeout is forwarded as typeahead; only a whole late reply is dropped.
        let probe_until = cpr.is_none().then(|| Instant::now() + LATE_PROBE);
        match cpr {
            Some((r, c)) => tracker.advance(&mut term, format!("\x1b[{r};{c}H").as_bytes()),
            None => write_stdout(b"\x1b[H\x1b[2J"),
        }

        // Everything that can fail goes before the child exists, so the fallback never orphans a shell.
        let truecolor = matches!(
            std::env::var("COLORTERM").as_deref(),
            Ok("truecolor" | "24bit")
        ) || std::env::var_os("WT_SESSION").is_some();
        let themes: Rc<dyn ThemeRepository> =
            Rc::new(FileThemeRepository::new(data_directory_from_env()));
        let renderer = OverlayRenderer::new(resolve_theme(&settings, terminal_bg, &*themes), truecolor)
            .with_images(kitty_graphics::detect());
        let (menu, keymap) = (Menu::new(), Keymap::from_settings(&settings));
        let (tx, rx) = mpsc::channel();
        sys::spawn_resize(tx.clone())?;
        let engine = start(tx.clone(), settings.clone());

        let pair = portable_pty::native_pty_system().openpty(PtySize {
            rows,
            cols,
            pixel_width: 0,
            pixel_height: 0,
        })?;
        let mut cb = CommandBuilder::from_argv(argv.iter().map(Into::into).collect());
        let cwd = std::env::current_dir()
            .ok()
            .filter(|d| d.is_dir())
            .or_else(|| user_home_from_env().ok());
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
        let reader = pair.master.try_clone_reader()?;
        let mut pty_writer = pair.master.take_writer()?;
        logger::info(&format!(
            "pty: {argv:?} {cols}x{rows} sid {sid} cpr {cpr:?}"
        ));

        let base = program.rsplit(['/', '\\']).next().unwrap_or(program);
        write_stdout(format!("\x1b]0;{base}\x07").as_bytes());

        // Threads (SPEC §4.3).
        let t = tx.clone();
        let (credits, credit_rx) = mpsc::sync_channel(PTY_CREDITS);
        for _ in 0..PTY_CREDITS {
            let _ = credits.send(());
        }
        thread::spawn(move || pump(reader, credit_rx, t));
        let (writer, wrx) = mpsc::channel::<Vec<u8>>();
        thread::spawn(move || {
            for b in wrx {
                if pty_writer
                    .write_all(&b)
                    .and_then(|_| pty_writer.flush())
                    .is_err()
                {
                    break;
                }
            }
        });
        let t = tx.clone();
        thread::spawn(move || {
            let _ = t.send(Msg::ChildExit(sys::wait(&mut *child)));
        });
        sys::spawn_input(tx);
        if !typeahead.is_empty() {
            let _ = writer.send(typeahead);
        }

        let session_id = SessionId::new(sid).ok();
        let session_contexts = Box::new(FileSessionContextRepository::new(data_directory_from_env()));
        Ok(Session {
            rx,
            engine,
            writer,
            credits,
            master: Some(pair.master),
            modes: m,
            term,
            tracker,
            menu,
            renderer,
            themes,
            keymap,
            decoder: StdinKeyDecoder::default(),
            settings_mtime: settings_repository.last_modified(),
            settings_repository,
            settings,
            session_mtime: None,
            session_id,
            session_contexts,
            terminal_bg,
            shell,
            req: 0,
            sent: None,
            changed_at: Instant::now(),
            cur: None,
            lock: None,
            unechoed: String::new(),
            echo_pending: None,
            held: Vec::new(),
            held_at: None,
            redraw_until: None,
            ghost: None,
            llm: None,
            llm_req: 0,
            llm_at: None,
            probe_until,
            resize_at: None,
            repaint: false,
            spinner: 0,
            spinner_at: None,
            poll_at: Instant::now() + FILE_POLL,
            exit_code: None,
            eof: false,
            exit_at: None,
            passthrough: false,
            sink: Vec::new(),
        })
    }

    /// Main loop (SPEC §4.3); returns the child's exit code.
    fn run(mut self) -> i32 {
        let mut stash = None;
        loop {
            let msg = match stash.take() {
                Some(m) => Ok(m),
                None => match self.next_deadline() {
                    Some(d) => self
                        .rx
                        .recv_timeout(d.saturating_duration_since(Instant::now())),
                    None => self.rx.recv().map_err(|_| RecvTimeoutError::Disconnected),
                },
            };
            let go = match msg {
                Ok(m) => self.step(m, &mut stash),
                Err(RecvTimeoutError::Timeout) => true,
                Err(RecvTimeoutError::Disconnected) => false,
            };
            if !go {
                break;
            }
            if self.exit_at.is_some_and(|d| d <= Instant::now()) {
                break;
            }
            if !self.passthrough {
                self.guard(Session::on_timers);
            }
        }
        self.cleanup();
        self.exit_code.unwrap_or(0)
    }

    /// One main-loop message; false = leave the loop.
    fn step(&mut self, msg: Msg, stash: &mut Option<Msg>) -> bool {
        match msg {
            Msg::Stdin(b) if self.passthrough => {
                let _ = self.writer.send(b);
            }
            Msg::Stdin(b) => {
                // on_input writes nothing before its last step: after a panic, keys the menu would not have taken go
                // out raw (an Enter meant for an item is dropped rather than running the line).
                let raw = !self.menu.intercepting(&self.settings);
                if !self.guard(|s| s.on_input(&b)) && raw {
                    let _ = self.writer.send(b);
                }
            }
            Msg::Pty(mut b) => {
                // One batch = everything already queued (one model pass, one stdout write).
                let mut chunks = 1;
                loop {
                    match self.rx.try_recv() {
                        Ok(Msg::Pty(more)) => {
                            b.extend(more);
                            chunks += 1;
                        }
                        Ok(other) => {
                            *stash = Some(other);
                            break;
                        }
                        Err(_) => break,
                    }
                }
                // on_output writes nothing before its last step: after a panic the batch goes out raw.
                if self.passthrough || !self.guard(|s| s.on_output(&b)) {
                    self.emit(&b);
                }
                for _ in 0..chunks {
                    let _ = self.credits.try_send(());
                }
            }
            Msg::PtyEof => {
                self.eof = true;
                return self.exit_code.is_none();
            }
            Msg::Resize(cols, rows) if self.passthrough => {
                if let Some(m) = &self.master {
                    let _ = m.resize(PtySize {
                        rows,
                        cols,
                        pixel_width: 0,
                        pixel_height: 0,
                    });
                }
            }
            Msg::Resize(c, r) => {
                self.guard(|s| s.on_resize(c, r));
            }
            Msg::ChildExit(c) => {
                self.exit_code = Some(c);
                // ConPTY does not EOF on child exit. Its close may wait until the final frame fits in the pipe, which
                // the reader drains only with credits from this loop: close it on a helper thread.
                if let Some(m) = self.master.take_if(|_| cfg!(windows)) {
                    thread::spawn(move || drop(m));
                }
                if self.eof {
                    return false;
                }
                self.exit_at = Some(Instant::now() + EXIT_GRACE);
            }
            Msg::Term => {
                self.exit_code = Some(129);
                return false;
            }
            Msg::Completion(_) | Msg::Ghost { .. } if self.passthrough => {}
            Msg::Completion(c) => {
                self.guard(|s| s.on_completion(*c));
            }
            Msg::Ghost { req, text } => {
                self.guard(|s| {
                    if req == s.req && req == s.llm_req {
                        s.llm = text
                            .zip(s.sent.as_ref().map(|x| x.0.clone()))
                            .map(|(t, b)| GhostText::new(b, t));
                        s.redraw();
                    }
                });
            }
        }
        true
    }

    /// Run one handler. A panic (logged by the hook) clears the overlay and turns the session into a plain
    /// pass-through until the child exits: the running shell and its jobs are never killed for it.
    /// ponytail: a panic in `on_output`'s final frame loses the clear it already took (overlay left until redrawn).
    fn guard(&mut self, f: impl FnOnce(&mut Session)) -> bool {
        if std::panic::catch_unwind(AssertUnwindSafe(|| f(self))).is_ok() {
            return true;
        }
        logger::error("pty: internal error; passing the session through until the shell exits");
        self.passthrough = true;
        let clear = self.renderer.take_clear();
        self.emit(&clear);
        false
    }

    fn emit(&mut self, b: &[u8]) {
        if cfg!(test) {
            self.sink.extend_from_slice(b);
        } else {
            write_stdout(b);
        }
    }

    fn next_deadline(&self) -> Option<Instant> {
        if self.passthrough {
            return self.exit_at;
        }
        let lock = self.lock.as_ref().map(|l| l.0);
        [
            self.resize_at,
            lock,
            self.llm_at,
            self.spinner_at,
            Some(self.poll_at),
            self.exit_at,
            self.redraw_until,
            self.held_at,
        ]
        .into_iter()
        .flatten()
        .min()
    }

    fn alt(&self) -> bool {
        self.term.mode().contains(TermMode::ALT_SCREEN)
    }

    fn cwd(&self) -> PathBuf {
        self.tracker
            .marker
            .cwd()
            .map(|p| p.to_path_buf())
            .or_else(|| std::env::current_dir().ok())
            .unwrap_or_default()
    }

    fn shell(&self) -> Shell {
        self.tracker.marker.shell().unwrap_or(self.shell)
    }

    /// PTY batch (SPEC §4.5): clear, bytes (markers stripped), frame — one stdout write.
    fn on_output(&mut self, batch: &[u8]) {
        if self.resize_at.is_some() {
            self.resize_at = Some(Instant::now() + RESIZE_DEBOUNCE); // the shell / ConPTY is still redrawing
        }
        let was_alt = self.alt();
        let mut body = Vec::new();
        self.tracker.feed(&mut self.term, batch, &mut body);
        for ev in std::mem::take(&mut self.tracker.events) {
            self.on_event(ev);
        }
        if was_alt && !self.alt() {
            self.menu.error_reset();
        }
        self.refresh();
        // Taken late: a panic above leaves the clear cached for `guard`.
        let mut out = self.renderer.take_clear();
        out.extend(body);
        out.extend(self.take_repaint());
        out.extend(self.frame());
        self.emit(&out);
        self.release_held();
    }

    fn on_event(&mut self, ev: ShellEvent) {
        match ev {
            // A prompt redraw (SIGWINCH, `zle reset-prompt`, an empty line) re-emits NewCmd: line, menu and ghost
            // stay, and the unchanged line is not re-requested (its reply would count as a keypress). A cleared line
            // resets through the empty-buffer path in `refresh`, once a line split off into a later read had time.
            ShellEvent::NewCommand(None) => {
                self.redraw_until = Some(Instant::now() + REDRAW_GRACE);
                self.poll_files();
                return;
            }
            ShellEvent::NewCommand(Some(_)) | ShellEvent::CommandEnded => {
                match ev {
                    ShellEvent::NewCommand(Some(h)) => self.engine.command_finished(h),
                    _ => self.engine.command_ended(), // NoHistory: the end-of-command reset, nothing recorded
                }
                self.menu.error_reset();
                self.sent = None;
                self.poll_files();
            }
            ShellEvent::PreExec => self.menu.error_reset(),
        }
        self.unechoed.clear();
        self.echo_pending = None;
        self.ghost = None;
        self.llm = None;
        self.llm_at = None;
        self.lock = None;
    }

    /// Re-read the edit buffer; request completions when it changed (SPEC §4.5; held while insertion-locked).
    fn refresh(&mut self) {
        // Off the command line the cursor is in a widget's inline TUI (fzf, fish's pager; see Tracker::extract). Live
        // UI only: pre_exec still scrapes after a transient prompt, which is redrawn without markers.
        let new = if self.alt() {
            None
        } else {
            self.tracker.extract(&self.term).filter(|e| e.at_prompt())
        };
        if !self.unechoed.is_empty() {
            let base = self
                .lock
                .as_ref()
                .map(|l| l.2.as_str())
                .or(self.cur.as_ref().map(|e| e.buffer()));
            let n = new.as_ref().map(|e| e.buffer());
            self.unechoed = unechoed_after(&self.unechoed, base, n, self.lock.is_some());
        }
        self.cur = new;
        if let Some((until, cap, expected, before)) = &mut self.lock {
            let now = Instant::now();
            let buf = self.cur.as_ref().map(|e| e.buffer());
            let applied = buf == Some(expected.as_str());
            if !applied && now < *until {
                return;
            }
            // Nothing of it echoed yet (a slow shell): it stays in the basis, or it would be written twice.
            if !applied && now < *cap && buf == Some(before.as_str()) {
                *until = now + INSERT_LOCK;
                return;
            }
            self.lock = None;
        }
        let Some(e) = &self.cur else { return };
        match self.redraw_until {
            Some(t) if e.buffer().trim().is_empty() && Instant::now() < t => return,
            _ => self.redraw_until = None,
        }
        if self
            .sent
            .as_ref()
            .is_some_and(|(b, c)| *b == e.buffer() && *c == e.cursor())
        {
            return;
        }
        let (buffer, cursor) = (e.buffer().to_string(), e.cursor());
        self.req += 1;
        self.sent = Some((buffer.clone(), cursor));
        self.changed_at = Instant::now();
        self.llm_at = None;
        if buffer.trim().is_empty() {
            self.menu.error_reset(); // empty buffer (SPEC §7.1); nothing to ask
            self.echo_pending = None; // nothing stale left for held keys to act on
            return;
        }
        let req = CompletionRequest {
            id: self.req,
            buffer,
            cursor,
            cwd: self.cwd(),
            shell: self.shell(),
            history_mode: self.menu.history_mode(),
        };
        self.engine.complete(req);
    }

    fn on_completion(&mut self, c: CompletionResult) {
        if c.request_id() != self.req {
            return; // only the latest request is served
        }
        if self.echo_pending.is_some_and(|r| c.request_id() > r) {
            self.echo_pending = None;
        }
        if c.phase() == CompletionPhase::Parse || c.ghost().is_some() {
            self.ghost = c.ghost().map(String::from).map(|g| GhostText::new(c.buffer().to_string(), g));
        }
        // LLM ghost: only when the local reply has no history match (SPEC §8.4). Error replies (no spec for the
        // command) too: the inline source depends on the buffer only. A mid-word cursor fails the length check.
        if c.phase() == CompletionPhase::Parse
            && c.ghost().is_none()
            && c.cursor() == c.buffer().len()
            && self.llm_enabled()
        {
            let debounce = self.settings.i64(INLINE_DEBOUNCE_MS).max(0) as u64;
            self.llm_at = Some(self.changed_at + Duration::from_millis(debounce));
        }
        if c.is_loading() && self.spinner_at.is_none() {
            self.spinner_at = Some(Instant::now() + SPINNER_TICK);
        }
        self.menu.on_completion(c, &self.settings, Instant::now());
        self.redraw();
        self.release_held();
    }

    fn llm_enabled(&self) -> bool {
        self.settings.bool(INLINE_ENABLED)
            && matches!(
                crate::ghost_text::domain::services::ghost_text_source_resolver::source(&self.settings),
                Ok(GhostTextSource::Profile(_))
            )
    }

    /// Ghost text (SPEC §8.1): survives typing that matches its start. `typed` = after the keys typed ahead of
    /// their echo (what accepting it must add), else after the buffer on screen (what is drawn).
    fn ghost_text(&self, typed: bool) -> Option<String> {
        if !self.settings.bool(INLINE_ENABLED) || self.tracker.marker.preexec() || self.alt() {
            return None;
        }
        let e = self.cur.as_ref()?;
        if e.buffer().is_empty() || e.cursor() != e.buffer().len() || e.shell_ghost() {
            return None;
        }
        let buffer = if typed {
            self.basis()?
        } else {
            e.buffer().to_string()
        };
        [&self.ghost, &self.llm]
            .into_iter()
            .flatten()
            .find_map(|g| g.rest_after(&buffer))
    }

    /// The buffer once the shell echoed everything already sent: the pending insertion's result (else the extracted
    /// buffer) plus the printable keys typed ahead of their echo (SPEC §7.3 reconciliation).
    fn basis(&self) -> Option<String> {
        let b = self
            .lock
            .as_ref()
            .map(|l| l.2.as_str())
            .or(self.cur.as_ref().map(|e| e.buffer()))?;
        Some(format!("{b}{}", self.unechoed))
    }

    /// Track a forwarded key: a printable char at the end of the line comes back as echo; anything else
    /// (backspace, arrows, control keys, paste) cannot be modelled and drops the prediction.
    fn note_forwarded(&mut self, key: &Key) {
        let at_end = self
            .cur
            .as_ref()
            .is_some_and(|e| e.cursor() == e.buffer().len());
        match key.code() {
            KeyCode::Char(c) if key.modifiers() == Modifiers::default() && at_end => {
                self.unechoed.push(c)
            }
            _ => {
                self.unechoed.clear();
                self.echo_pending = Some(self.req);
            }
        }
    }

    /// Replay held keys once the reply for the buffer after the unmodelled key came (or the hold timed out).
    fn release_held(&mut self) {
        let timed_out = self.held_at.is_some_and(|t| t <= Instant::now());
        if self.held.is_empty() || (self.echo_pending.is_some() && !timed_out) {
            return;
        }
        self.echo_pending = None;
        self.held_at = None;
        let held = std::mem::take(&mut self.held);
        self.on_keys(&held);
    }

    fn suppressed(&self) -> bool {
        !self.tracker.marker.at_prompt()
            // The cursor is off the command line (an inline TUI's rows: fzf, fish's pager): a menu for the line
            // stays hidden and keeps its hands off the keys.
            || self.cur.is_none()
            || self.resize_at.is_some()
            || self.repaint
            || self.tracker.filter.in_string()
            || self.redraw_until.is_some()
            // PSReadLine's ListView predictions own the rows below a non-empty line.
            || (self.tracker.marker.pwsh_list_view() && self.cur.as_ref().is_some_and(|e| !e.buffer().is_empty()))
    }

    /// No render guard (SPEC §7.5.1) holds, so a menu or ghost can be on screen and keys may act on it.
    /// ponytail: mirrors overlay_renderer.rs `compose`'s guards; its no-room-for-the-menu case (3-row terminal) is not covered.
    fn drawable(&self) -> bool {
        let g = self.term.grid();
        !self.suppressed()
            && !self.tracker.filter.scroll_region()
            && !self
                .term
                .mode()
                .intersects(TermMode::ALT_SCREEN | TermMode::ORIGIN | TermMode::INSERT)
            && !g.cursor.input_needs_wrap
            && g.columns() >= 10
            && g.screen_lines() >= 3
    }

    fn frame(&mut self) -> Vec<u8> {
        let ghost = self.ghost_text(false);
        let input = FrameInput {
            settings: &self.settings,
            ghost: ghost.as_deref(),
            spinner: self.spinner,
            suppress: self.suppressed(),
            scroll_region: self.tracker.filter.scroll_region(),
            now: Instant::now(),
        };
        self.renderer.frame(&self.term, &self.menu, &input)
    }

    /// State-only change: the renderer clears a changed frame in the same write and skips an identical one
    /// (SPEC §7.5.3).
    fn redraw(&mut self) {
        let out = self.frame();
        self.emit(&out);
    }

    /// Stdin chunk (SPEC §7.2.4): intercept bound keys while the menu is shown, accept ghost text, else forward.
    fn on_input(&mut self, chunk: &[u8]) {
        // A probe reply arriving after the startup timeout is dropped, not typed into the shell.
        let late;
        let chunk = match self.probe_until {
            Some(t) if Instant::now() < t => {
                let (cpr, bg, rest) = parse_probe(chunk);
                if bg.is_some() {
                    self.terminal_bg = bg;
                    self.renderer
                        .set_theme(resolve_theme(&self.settings, bg, &*self.themes));
                }
                if cpr.is_some() {
                    self.probe_until = None;
                }
                late = rest;
                &late[..]
            }
            _ => chunk,
        };
        self.on_keys(chunk);
    }

    fn on_keys(&mut self, chunk: &[u8]) {
        if !self.held.is_empty() {
            self.held.extend_from_slice(chunk); // keys keep their order behind held ones
            return;
        }
        let mut fwd = Vec::new();
        let (mut redraw, mut rerequest) = (false, false);
        let only_tab = self.settings.bool(AUTOCOMPLETE_ONLY_SHOW_ON_TAB);
        let disabled = self.settings.bool(AUTOCOMPLETE_DISABLE);
        let mut typed = 0usize;
        for ev in self.decoder.decode(chunk) {
            #[cfg(test)]
            tests::maybe_panic();
            let key = ev.key();
            // Two or more printable keys in one read are a paste (pwsh has no bracketed paste) or typeahead the menu
            // was not computed for: the rest of the read reaches the shell as is.
            // Off the command line (an inline TUI's rows) keys belong to the TUI, as on the alternate screen.
            let prompt = !self.tracker.marker.preexec() && !self.alt() && self.cur.is_some() && typed < 2;
            // A logically shown menu acts on keys only while the render guards let it be seen.
            let shown = prompt && self.menu.intercepting(&self.settings);
            let intercepting = shown && self.drawable();
            let global = prompt && !shown && !disabled && self.menu.has_items();
            let binding = if prompt {
                self.keymap.lookup(&key)
            } else {
                None
            };
            let tab_show = global
                && only_tab
                && key.code() == KeyCode::Tab
                && key.modifiers() == Modifiers::default();
            let ghost = ghost_key(&key)
                .filter(|_| prompt && self.drawable())
                .zip(self.ghost_text(true));
            let acts = ghost.is_some()
                || tab_show
                || (intercepting && matches!(binding, Some(KeyBinding::Action(_) | KeyBinding::Unknown)));
            if acts && self.echo_pending.is_some() {
                // The menu / ghost is for a buffer the shell may no longer have: wait for the echo and its reply.
                // ponytail: one hold buffer; separate reads replayed together count as one for the paste rule.
                self.held.extend_from_slice(&chunk[ev.bytes().start..]);
                self.held_at = Some(Instant::now() + INSERT_LOCK);
                self.decoder = StdinKeyDecoder::default(); // held bytes start outside a paste and are decoded again
                break;
            }
            let effects = match binding {
                Some(KeyBinding::Action(a)) if intercepting => self.menu.handle(a, &self.settings),
                Some(KeyBinding::Unknown) if intercepting => Vec::new(),
                _ if tab_show => self.menu.show_on_tab(&self.settings),
                Some(KeyBinding::Action(
                    a @ (KeyAction::ShowAutocomplete | KeyAction::ToggleAutocomplete),
                )) if global => self.menu.handle(a, &self.settings),
                _ => {
                    match ghost {
                        Some((full, g)) => {
                            let g = if full { g.as_str() } else { GhostText::next_word(&g) };
                            fwd.extend_from_slice(g.as_bytes());
                            self.unechoed.push_str(g);
                            self.ghost = None;
                            self.llm = None;
                            redraw = true;
                        }
                        None => {
                            if key.code() == KeyCode::Enter && self.tracker.marker.at_prompt() {
                                self.tracker.marker.set_submitted(self.basis()); // history: the line, not what follows it
                            }
                            fwd.extend_from_slice(&chunk[ev.bytes()]);
                            typed += usize::from(
                                matches!(key.code(), KeyCode::Char(_)) && key.modifiers() == Modifiers::default(),
                            );
                            self.note_forwarded(&key);
                            // Enter too: its newline echo would re-anchor the stale menu below the line until PreExec.
                            if is_ctrl(&key, 'c')
                                || is_ctrl(&key, 'd')
                                || key.code() == KeyCode::Enter
                            {
                                self.menu.error_reset();
                                redraw = true;
                            }
                        }
                    }
                    continue;
                }
            };
            redraw = true;
            for e in effects {
                match e {
                    MenuEffect::Insert { ins, record, .. } => self.insert(&ins, record, &mut fwd),
                    MenuEffect::Write(b) => {
                        if b == b"\r" && self.tracker.marker.at_prompt() {
                            self.tracker.marker.set_submitted(self.basis()); // `execute`
                        }
                        fwd.extend(b);
                        self.unechoed.clear();
                    }
                    MenuEffect::Bell => self.emit(b"\x07"),
                    MenuEffect::Rerequest => rerequest = true,
                    MenuEffect::ToggleFuzzy => {
                        self.engine.toggle_fuzzy_search();
                        rerequest = true;
                    }
                }
            }
        }
        if rerequest {
            self.sent = None;
            self.refresh();
        }
        if redraw {
            self.redraw();
        }
        if !fwd.is_empty() {
            let _ = self.writer.send(fwd); // last: after a panic above, `step` forwards the read raw
        }
    }

    /// Insertion with reconciliation + lock (SPEC §7.3). Dropped while a command runs.
    fn insert(&mut self, ins: &Insertion, record: Option<(String, String)>, fwd: &mut Vec<u8>) {
        if self.tracker.marker.preexec() {
            return;
        }
        let from = self.menu.last().map(|c| (c.buffer().to_string(), c.cursor()));
        let Some((ib, at)) =
            from.or_else(|| self.cur.as_ref().map(|e| (e.buffer().to_string(), e.cursor())))
        else {
            return;
        };
        // Against what the shell will show, not the last batch: keys sent ahead of their echo are reconciled too.
        let cur = self.basis().unwrap_or_else(|| ib.clone());
        fwd.extend(insertion_bytes(ins, &ib, &cur));
        self.unechoed.clear();
        let expected = expected_buffer(ins, &ib, at);
        if ins.executes() {
            // As for Enter: preferred over a scrape with a transient prompt in front of the line; pre_exec still
            // reads a line the shell rewrote from the screen.
            self.tracker.marker.set_submitted(Some(expected.clone()));
        }
        let before = self
            .cur
            .as_ref()
            .map_or_else(|| ib.clone(), |e| e.buffer().to_string());
        let now = Instant::now();
        self.lock = Some((now + INSERT_LOCK, now + LOCK_CAP, expected, before));
        if let Some((cmd, name)) = record {
            self.engine.record_insertion(cmd, name);
        }
    }

    /// SPEC §7.6: resize PTY + model now, repaint after 150 ms of quiet.
    fn on_resize(&mut self, cols: u16, rows: u16) {
        if cols == 0
            || rows == 0
            || (cols as usize, rows as usize) == (self.term.columns(), self.term.screen_lines())
        {
            return;
        }
        if let Some(m) = &self.master {
            let _ = m.resize(PtySize {
                rows,
                cols,
                pixel_width: 0,
                pixel_height: 0,
            });
        }
        self.term
            .resize(TermSize::new(cols as usize, rows as usize));
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
        if due(self.redraw_until) {
            self.redraw_until = None;
            self.refresh(); // the line really is empty
            redraw = true;
        }
        self.release_held();
        if self.lock.as_ref().is_some_and(|l| l.0 <= now) {
            self.refresh(); // releases the lock and catches up with the buffer
            redraw = true;
        }
        if due(self.llm_at) {
            self.llm_at = None;
            if let Some((buffer, _)) = self.sent.clone() {
                self.llm_req = self.req;
                self.engine.request_llm_ghost_text(self.req, buffer, self.cwd(), self.shell());
            }
        }
        if due(self.spinner_at) {
            self.spinner_at = None;
            if self.menu.last().is_some_and(|c| c.is_loading()) {
                self.spinner += 1;
                self.spinner_at = Some(now + SPINNER_TICK);
                redraw |= self.menu.loading(Instant::now());
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
            self.emit(&out);
        } else if redraw {
            self.redraw();
        }
    }

    /// Settings (live reload) and the session env file from `_ pre-cmd` (D17). True when settings changed.
    fn poll_files(&mut self) -> bool {
        let m = self
            .session_id
            .as_ref()
            .and_then(|id| self.session_contexts.last_modified(id));
        if m.is_some() && m != self.session_mtime {
            self.session_mtime = m;
            if let Some(ctx) = self
                .session_id
                .as_ref()
                .and_then(|id| self.session_contexts.find(id))
            {
                self.engine
                    .apply_session_context(ctx.env().clone(), ctx.alias().map(str::to_owned));
            }
        }
        let m = self.settings_repository.last_modified();
        if m == self.settings_mtime {
            return false;
        }
        self.settings_mtime = m;
        self.settings = load_or_defaults(&*self.settings_repository);
        self.keymap = Keymap::from_settings(&self.settings);
        self.renderer
            .set_theme(resolve_theme(
                &self.settings,
                self.terminal_bg,
                &*self.themes,
            ));
        self.engine.apply_settings(self.settings.clone());
        true
    }

    /// SPEC §4.4 step 10.
    fn cleanup(&mut self) {
        let mut out = self.renderer.take_clear();
        out.extend_from_slice(b"\x1b[0m\x1b[?25h");
        self.emit(&out);
        self.engine.shut_down();
        crate::specs::infrastructure::adapters::process_executor::kill_live_children(); // Windows has no atexit hook for running generators
        self.modes.restore();
        if let Some(id) = &self.session_id {
            self.session_contexts.remove(id);
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

    #[derive(Clone, Copy, Default)]
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
            let mut tv = libc::timeval {
                tv_sec: (us / 1_000_000) as _,
                tv_usec: (us % 1_000_000) as _,
            };
            if libc::select(
                1,
                &mut set,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                &mut tv,
            ) <= 0
            {
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

    /// The child's exit code as a shell reports it: 128 + signal when killed (portable-pty maps that to 1).
    pub fn wait(child: &mut dyn portable_pty::Child) -> i32 {
        match child.process_id() {
            Some(pid) => wait_pid(pid as libc::pid_t),
            None => child.wait().map_or(1, |s| s.exit_code() as i32),
        }
    }

    pub fn wait_pid(pid: libc::pid_t) -> i32 {
        let mut st = 0;
        // SAFETY: waitpid on our own child with a stack-owned status.
        while unsafe { libc::waitpid(pid, &mut st, 0) } != pid {
            if std::io::Error::last_os_error().kind() != std::io::ErrorKind::Interrupted {
                return 1;
            }
        }
        if libc::WIFSIGNALED(st) {
            128 + libc::WTERMSIG(st)
        } else {
            libc::WEXITSTATUS(st)
        }
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
        GetConsoleCP, GetConsoleMode, GetConsoleOutputCP, GetStdHandle, ReadConsoleInputW,
        SetConsoleCP, SetConsoleMode, SetConsoleOutputCP, DISABLE_NEWLINE_AUTO_RETURN,
        ENABLE_ECHO_INPUT, ENABLE_LINE_INPUT, ENABLE_PROCESSED_INPUT,
        ENABLE_VIRTUAL_TERMINAL_INPUT, ENABLE_VIRTUAL_TERMINAL_PROCESSING, ENABLE_WINDOW_INPUT,
        INPUT_RECORD, KEY_EVENT, STD_INPUT_HANDLE, STD_OUTPUT_HANDLE, WINDOW_BUFFER_SIZE_EVENT,
    };
    use windows_sys::Win32::System::Threading::WaitForSingleObject;

    const CP_UTF8: u32 = 65001;

    /// Saved console state, restored on exit.
    #[derive(Clone, Copy, Default)]
    pub struct Modes {
        input: u32,
        output: u32,
        in_cp: u32,
        out_cp: u32,
    }

    fn handles() -> (HANDLE, HANDLE) {
        // SAFETY: GetStdHandle has no preconditions.
        unsafe {
            (
                GetStdHandle(STD_INPUT_HANDLE),
                GetStdHandle(STD_OUTPUT_HANDLE),
            )
        }
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
            let m = Modes {
                input,
                output,
                in_cp: GetConsoleCP(),
                out_cp: GetConsoleOutputCP(),
            };
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
        read_console(i, &mut None)
            .map(|(b, _)| b)
            .unwrap_or_default()
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

    pub fn wait(child: &mut dyn portable_pty::Child) -> i32 {
        child.wait().map_or(1, |s| s.exit_code() as i32)
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
    use crate::completion::domain::value_objects::insertion::InsertionParts;

    fn ins(delete: usize, text: &str, cursor_back: usize, execute: bool) -> Insertion {
        Insertion::new(InsertionParts {
            delete,
            text: text.into(),
            cursor_back,
            execute,
        })
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
        assert_eq!(
            insertion_bytes(&i, "x --o", "x --o"),
            b"\x7f\x7f\x7f--out=\x1b[D\r"
        );
    }

    #[test]
    fn expected_buffer_after_insertion() {
        assert_eq!(
            expected_buffer(&ins(0, "atus ", 0, false), "git st", 6),
            "git status "
        );
        assert_eq!(
            expected_buffer(&ins(6, "my\\ dir/", 0, false), "cd my\\ di", 9),
            "cd my\\ dir/"
        );
        assert_eq!(
            expected_buffer(&ins(2, "checkout ", 0, false), "git ch | x", 6),
            "git checkout  | x"
        );
        assert_eq!(expected_buffer(&ins(1, "é", 0, false), "aé", 3), "aé");
    }

    #[test]
    fn probe_replies() {
        let (cpr, bg, rest) = parse_probe(b"ab\x1b]11;rgb:ffff/8080/0000\x1b\\\x1b[12;40Rc");
        assert_eq!(
            (cpr, bg, rest.as_slice()),
            (Some((12, 40)), Some((255, 128, 0)), b"abc".as_slice())
        );
        let (cpr, bg, rest) = parse_probe(b"\x1b]11;rgb:f/0/f\x07\x1b[1;1R");
        assert_eq!(
            (cpr, bg, rest.len()),
            (Some((1, 1)), Some((255, 0, 255)), 0)
        );
        // Keys typed meanwhile (arrows, partial replies) are kept for the shell.
        let (cpr, _, rest) = parse_probe(b"\x1b[A\x1b[12;");
        assert_eq!((cpr, rest.as_slice()), (None, b"\x1b[A\x1b[12;".as_slice()));
        // The background is always asked for (a later switch to `system` needs it), before the CPR that ends it.
        assert!(PROBE.starts_with(b"\x1b]11;?") && PROBE.ends_with(b"\x1b[6n"));
    }

    #[test]
    fn child_argv() {
        let s = |v: &[&str]| v.iter().map(|x| x.to_string()).collect::<Vec<_>>();
        assert_eq!(
            argv(Some("/bin/zsh"), &[], true),
            s(&["/bin/zsh", "--login"])
        );
        assert_eq!(argv(Some("pwsh"), &[], true), s(&["pwsh"]));
        assert_eq!(
            argv(Some("bash"), &s(&["--noprofile", "--rcfile", "x"]), false),
            s(&["bash", "--noprofile", "--rcfile", "x"])
        );
        assert_eq!(
            argv(Some("bash"), &s(&["vim", "-u"]), true),
            s(&["vim", "-u"])
        );
        assert!(argv(None, &[], false).is_empty());
    }

    #[test]
    fn ghost_keys_and_words_keys() {
        let k = |code, mods| Key::new(code, mods);
        let ctrl = Modifiers::none().with_ctrl(true);
        let alt = Modifiers::none().with_alt(true);
        assert_eq!(ghost_key(&k(KeyCode::Right, Modifiers::default())), Some(true));
        assert_eq!(ghost_key(&k(KeyCode::Char('e'), ctrl)), Some(true));
        assert_eq!(ghost_key(&k(KeyCode::Char('f'), alt)), Some(false));
        assert_eq!(ghost_key(&k(KeyCode::Right, ctrl)), Some(false));
        assert_eq!(ghost_key(&k(KeyCode::Tab, Modifiers::default())), None);
    }

    #[test]
    fn unechoed_keys_are_consumed_by_their_echo() {
        assert_eq!(unechoed_after("lo", Some("hel"), Some("hel"), false), "lo");
        assert_eq!(unechoed_after("lo", Some("hel"), Some("hell"), false), "o");
        assert_eq!(unechoed_after("lo", Some("hel"), Some("hello"), false), "");
        assert_eq!(
            unechoed_after("lo", Some("hel"), Some("he"), false),
            "",
            "the shell did something else"
        );
        // Locked: typed after a pending insertion (base = its expected buffer).
        assert_eq!(
            unechoed_after("y", Some("cat abcd"), Some("cat ab"), true),
            "y"
        );
        assert_eq!(
            unechoed_after("y", Some("cat abcd"), Some("cat abcdy"), true),
            ""
        );
        assert_eq!(unechoed_after("y", None, Some("x"), false), "");
    }

    #[test]
    fn pty_reader_waits_for_credits() {
        let (credits, credit_rx) = mpsc::sync_channel(PTY_CREDITS);
        let (tx, rx) = mpsc::channel();
        credits.send(()).unwrap();
        credits.send(()).unwrap();
        std::thread::spawn(move || pump(std::io::repeat(b'y'), credit_rx, tx));
        let pty =
            |r: Result<Msg, RecvTimeoutError>| matches!(r, Ok(Msg::Pty(b)) if b.len() == 16 * 1024);
        assert!(pty(rx.recv_timeout(Duration::from_secs(5))));
        assert!(pty(rx.recv_timeout(Duration::from_secs(5))));
        assert!(
            rx.recv_timeout(Duration::from_millis(100)).is_err(),
            "an endless child is not read ahead"
        );
        credits.send(()).unwrap();
        assert!(pty(rx.recv_timeout(Duration::from_secs(5))));
        drop(credits);
        assert!(matches!(
            rx.recv_timeout(Duration::from_secs(5)),
            Ok(Msg::PtyEof)
        ));
    }

    #[cfg(unix)]
    #[test]
    fn signal_death_exit_code() {
        let sh = |s: &str| {
            std::process::Command::new("/bin/sh")
                .args(["-c", s])
                .spawn()
                .unwrap()
                .id() as libc::pid_t
        };
        assert_eq!(sys::wait_pid(sh("kill -9 $$")), 137);
        assert_eq!(sys::wait_pid(sh("exit 3")), 3);
    }

    // --- Session driven without a terminal: PTY output via `on_output`, keys via `on_input`. -------------------

    use crate::completion::domain::test_fixtures::{comp, item, settings};
    use crate::dropdown::domain::repositories::theme_repository::InMemoryThemeRepository;
    use crate::dropdown::domain::value_objects::theme::Theme;
    use crate::settings::domain::repositories::settings_repository::InMemorySettingsRepository;
    use crate::shared::domain::value_objects::data_directory::DataDirectory;
    use crate::terminal_session::domain::repositories::session_context_repository::InMemorySessionContextRepository;
    use crate::terminal_session::application::ports::completion_engine::{CompletionEngineCall, RecordingCompletionEngine};
    use crate::completion::domain::value_objects::suggestion::Suggestion;
    use crate::specs::domain::value_objects::suggestion_kind::SuggestionKind;
    use alacritty_terminal::event::VoidListener;

    const SID: &str = "0123456789abcdef0123456789abcdef";

    thread_local!(static PANIC_IN_INPUT: std::cell::Cell<bool> = const { std::cell::Cell::new(false) });

    /// Fault injection: a panic while keys are handled (decoder, menu, insertion).
    pub(super) fn maybe_panic() {
        if PANIC_IN_INPUT.with(|p| p.get()) {
            panic!("injected");
        }
    }

    fn session(json: &str) -> (Session, Receiver<Vec<u8>>) {
        let settings = settings(json);
        let (writer, wrx) = mpsc::channel();
        let tracker = Tracker::with_defer(SID.into(), Some(Shell::Bash), 24, false);
        let s = Session {
            rx: mpsc::channel().1,
            engine: Rc::new(RecordingCompletionEngine::new(mpsc::channel().0)),
            writer,
            credits: mpsc::sync_channel(PTY_CREDITS).0,
            master: None,
            modes: sys::Modes::default(),
            term: Term::new(Config::default(), &TermSize::new(80, 24), VoidListener),
            tracker,
            menu: Menu::new(),
            renderer: OverlayRenderer::new(Theme::dark(), false),
            themes: Rc::new(InMemoryThemeRepository::default()),
            keymap: Keymap::from_settings(&settings),
            decoder: StdinKeyDecoder::default(),
            settings,
            settings_repository: Box::new(InMemorySettingsRepository::default()),
            settings_mtime: None,
            session_id: None,
            session_contexts: Box::new(InMemorySessionContextRepository::default()),
            session_mtime: None,
            terminal_bg: None,
            shell: Shell::Bash,
            req: 0,
            sent: None,
            changed_at: Instant::now(),
            cur: None,
            lock: None,
            unechoed: String::new(),
            echo_pending: None,
            held: Vec::new(),
            held_at: None,
            redraw_until: None,
            ghost: None,
            llm: None,
            llm_req: 0,
            llm_at: None,
            probe_until: None,
            resize_at: None,
            repaint: false,
            spinner: 0,
            spinner_at: None,
            poll_at: Instant::now() + FILE_POLL,
            exit_code: None,
            eof: false,
            exit_at: None,
            passthrough: false,
            sink: Vec::new(),
        };
        (s, wrx)
    }

    /// A bash prompt `$ ` with the integration markers, then `text` echoed.
    fn prompt(s: &mut Session, text: &str) {
        let p = format!(
            "\x1b]697;StartPrompt\x07$ \x1b]697;EndPrompt\x07\x1b]697;NewCmd={SID}\x07{text}"
        );
        s.on_output(p.as_bytes());
    }

    /// Engine reply for the latest request (twice: the first one after a reset only arms the menu).
    fn reply(s: &mut Session, items: Vec<Suggestion>, ghost: Option<&str>) {
        let (buffer, _) = s.sent.clone().expect("a request was sent");
        let mut c = comp(&buffer, 1, items);
        c = c.with_request_id(s.req).with_ghost(ghost.map(String::from));
        s.on_completion(c.clone());
        s.on_completion(c);
    }

    fn subs(q: &str) -> Vec<Suggestion> {
        ["cherry-pick", "checkout"]
            .iter()
            .map(|n| item(n, SuggestionKind::Subcommand, q))
            .collect()
    }

    fn sent(pty: &Receiver<Vec<u8>>) -> Vec<u8> {
        pty.try_iter().flatten().collect()
    }

    /// The shell line after `line` receives what pty wrote (DEL erases, printable bytes append).
    fn shell_line(line: &str, bytes: &[u8]) -> String {
        let mut l = line.to_string();
        for &b in bytes {
            match b {
                0x7f => drop(l.pop()),
                0x20..=0x7e => l.push(b as char),
                _ => {}
            }
        }
        l
    }

    #[test]
    fn split_paste_is_never_intercepted() {
        let (mut s, pty) = session("{}");
        prompt(&mut s, "git ch");
        reply(&mut s, subs("ch"), None);
        assert!(s.menu.intercepting(&s.settings));
        s.on_input(b"\x1b[200~ONE");
        s.on_input(b"\rTWO\tx\x1b[A\x1b[201~");
        assert_eq!(sent(&pty), b"\x1b[200~ONE\rTWO\tx\x1b[A\x1b[201~");
    }

    #[test]
    fn menu_hidden_by_render_guards_does_not_intercept() {
        let (mut s, pty) = session("{}");
        prompt(&mut s, "git ch");
        reply(&mut s, subs("ch"), Some("erry-pick"));
        s.on_input(b"\t");
        assert!(!sent(&pty).contains(&b'\t'), "drawn: Tab is intercepted");
        // A crashed TUI left a scroll region: nothing is drawn, so Enter and End reach the shell.
        s.on_output(b"\x1b7\x1b[2;20r\x1b8");
        assert!(s.menu.intercepting(&s.settings) && !s.drawable());
        s.on_input(b"\r\x1b[F");
        assert_eq!(sent(&pty), b"\r\x1b[F");
    }

    #[test]
    fn disabled_menu_lets_tab_through() {
        let (mut s, pty) =
            session(r#"{"autocomplete.disable": true, "autocomplete.onlyShowOnTab": true}"#);
        prompt(&mut s, "cat ");
        let mut c = comp(
            "cat ",
            1,
            vec![
                item("a.txt", SuggestionKind::File, ""),
                item("b.txt", SuggestionKind::File, ""),
            ],
        );
        c = c.with_request_id(s.req);
        s.on_completion(c.clone());
        c = c.with_arg_key(2); // a new argument: hidden until Tab, with items
        s.on_completion(c);
        assert!(s.menu.has_items());
        s.on_input(b"\t\t");
        assert_eq!(sent(&pty), b"\t\t");
    }

    #[test]
    fn insertions_build_on_keys_not_yet_echoed() {
        // Tab + Enter (and Tab + Tab) in one read: the second insertion builds on the first one's result.
        let files = || {
            vec![
                item("abcd1.txt", SuggestionKind::File, "ab"),
                item("abcd2.txt", SuggestionKind::File, "ab"),
            ]
        };
        for (keys, want) in [(&b"\t\r"[..], "cat abcd1.txt"), (b"\t\t", "cat abcd")] {
            let (mut s, pty) = session("{}");
            prompt(&mut s, "cat ab");
            reply(&mut s, files(), None);
            s.on_input(keys);
            assert_eq!(shell_line("cat ab", &sent(&pty)), want, "{keys:?}");
        }
        // `e` + Enter in one read: the typed-ahead `e` is erased before the selection is inserted.
        let (mut s, pty) = session("{}");
        prompt(&mut s, "git ch");
        reply(&mut s, subs("ch"), None);
        s.on_input(b"e\r");
        assert_eq!(shell_line("git ch", &sent(&pty)), "git cherry-pick");
        // Same, with the echo arriving in between (consumed, not counted twice).
        let (mut s, pty) = session("{}");
        prompt(&mut s, "git ch");
        reply(&mut s, subs("ch"), None);
        s.on_input(b"e");
        s.on_output(b"e");
        assert!(s.unechoed.is_empty());
        s.on_input(b"\r");
        assert_eq!(shell_line("git ch", &sent(&pty)), "git cherry-pick");
        // `l` + Right in one read: only the rest of the ghost is accepted.
        let (mut s, pty) = session("{}");
        prompt(&mut s, "echo hel");
        reply(&mut s, vec![], Some("lo"));
        s.on_input(b"l\x1b[C");
        assert_eq!(shell_line("echo hel", &sent(&pty)), "echo hello");
    }

    #[test]
    fn llm_ghost_requested_for_commands_without_spec() {
        let (mut s, _) = session(
            r#"{"inline.source": "mock", "inline.profiles.mock": {"provider": "openai", "model": "m", "base_url": "http://127.0.0.1:9/v1"}}"#,
        );
        prompt(&mut s, "zzzcmd foo");
        s.on_completion(CompletionResult::error(s.req, "zzzcmd foo".into(), 10));
        assert!(s.llm_at.is_some());
        // Mid-word cursor: still none.
        s.llm_at = None;
        s.on_completion(CompletionResult::error(s.req, "zzzcmd foo".into(), 3));
        assert!(s.llm_at.is_none());
    }

    #[test]
    fn should_show_the_llm_ghost_text_of_the_current_request_behind_the_local_one() {
        let (mut s, _) = session("{}");
        prompt(&mut s, "git sta");
        s.llm_req = s.req;
        let llm_reply = |req, text: Option<&str>| Msg::Ghost { req, text: text.map(String::from) };

        s.step(llm_reply(s.req, Some("tus")), &mut None);
        let from_the_current_request = s.ghost_text(false);
        s.step(llm_reply(s.req + 1, Some("x")), &mut None);
        let after_another_request = s.ghost_text(false);
        s.step(llm_reply(s.req, None), &mut None);
        let after_an_empty_reply = s.ghost_text(false);
        s.step(llm_reply(s.req, Some("tus")), &mut None);
        reply(&mut s, Vec::new(), Some("sh"));
        let with_a_local_ghost_text = s.ghost_text(false);

        assert_eq!(
            (from_the_current_request, after_another_request, after_an_empty_reply, with_a_local_ghost_text),
            (Some("tus".to_string()), Some("tus".to_string()), None, Some("sh".to_string()))
        );
    }

    #[test]
    fn late_probe_reply_is_not_typed() {
        let (mut s, pty) = session("{}");
        s.probe_until = Some(Instant::now() + LATE_PROBE);
        prompt(&mut s, "");
        s.on_input(b"\x1b]11;rgb:ffff/ffff/ffff\x1b\\\x1b[5;1Rx");
        assert_eq!(sent(&pty), b"x");
        assert_eq!(
            (s.terminal_bg, s.probe_until),
            (Some((255, 255, 255)), None)
        );
    }

    #[test]
    fn identical_redraw_writes_nothing() {
        let (mut s, _) = session("{}");
        prompt(&mut s, "git ch");
        reply(&mut s, subs("ch"), None);
        assert!(
            s.sink.windows(8).any(|w| w == b"\x1b[?2026h"),
            "a frame was drawn"
        );
        s.sink.clear();
        s.redraw();
        assert!(s.sink.is_empty(), "{:?}", String::from_utf8_lossy(&s.sink));
    }

    #[test]
    fn prompt_redraw_keeps_the_menu() {
        let (mut s, _) = session("{}");
        prompt(&mut s, "git ch");
        reply(&mut s, subs("ch"), None);
        // SIGWINCH: the shell redraws PS1 (with NewCmd=<sid>) and the line, in one write.
        let redraw = format!("\r\x1b[K\x1b]697;StartPrompt\x07$ \x1b]697;EndPrompt\x07\x1b]697;NewCmd={SID}\x07git ch");
        s.on_output(redraw.as_bytes());
        assert!(s.menu.intercepting(&s.settings));
        // A command that ran does reset it.
        s.on_output(b"\r\n\x1b]697;PreExec\x07");
        prompt(&mut s, "");
        assert!(!s.menu.has_items());
    }

    #[test]
    fn no_frame_inside_an_open_dcs() {
        let (mut s, pty) = session("{}");
        prompt(&mut s, "git ch");
        reply(&mut s, subs("ch"), None);
        s.sink.clear();
        s.on_output(b"\x1bP+q");
        assert!(
            s.sink.ends_with(b"\x1bP+q"),
            "{:?}",
            String::from_utf8_lossy(&s.sink)
        );
        s.on_input(b"\x1b[D");
        assert_eq!(sent(&pty), b"\x1b[D");
        s.on_output(b"696e\x1b\\");
        assert!(s.sink.ends_with(b"\x1b[?2026l"), "frame back after ST");
    }

    #[test]
    fn panic_turns_the_session_into_a_pass_through() {
        let (mut s, pty) = session("{}");
        prompt(&mut s, "git ch");
        reply(&mut s, subs("ch"), None);
        s.sink.clear();
        assert!(!s.guard(|_| panic!("injected")));
        assert!(
            s.passthrough && s.sink.starts_with(b"\x1b[?2026h"),
            "overlay cleared"
        );
        s.sink.clear();
        let mut stash = None;
        assert!(s.step(Msg::Pty(b"out\x1b]697;NewCmd\x07".to_vec()), &mut stash));
        assert_eq!(s.sink, b"out\x1b]697;NewCmd\x07");
        assert!(s.step(Msg::Stdin(b"\r".to_vec()), &mut stash));
        assert_eq!(sent(&pty), b"\r", "keys reach the shell unintercepted");
        assert!(s.step(Msg::ChildExit(3), &mut stash));
        assert!(!s.step(Msg::PtyEof, &mut stash));
        assert_eq!((s.exit_code, s.next_deadline().is_some()), (Some(3), true));
    }

    #[test]
    fn panic_while_handling_keys_still_forwards_them() {
        let (mut s, pty) = session("{}");
        prompt(&mut s, "ls");
        PANIC_IN_INPUT.with(|p| p.set(true));
        assert!(s.step(Msg::Stdin(b"BOOM\r".to_vec()), &mut None));
        PANIC_IN_INPUT.with(|p| p.set(false));
        assert!(s.passthrough);
        assert_eq!(sent(&pty), b"BOOM\r", "the read is not lost");
        // With the menu intercepting it is dropped: an Enter meant for an item never runs the line.
        let (mut s, pty) = session("{}");
        prompt(&mut s, "git ch");
        reply(&mut s, subs("ch"), None);
        PANIC_IN_INPUT.with(|p| p.set(true));
        s.step(Msg::Stdin(b"\r".to_vec()), &mut None);
        PANIC_IN_INPUT.with(|p| p.set(false));
        assert!(s.passthrough && sent(&pty).is_empty());
    }

    /// The shell redraws its prompt (SIGWINCH, `zle reset-prompt`): PS1 with NewCmd=<sid>, then `text`.
    fn redraw(s: &mut Session, text: &str) {
        let r = format!("\r\x1b[K\x1b]697;StartPrompt\x07$ \x1b]697;EndPrompt\x07\x1b]697;NewCmd={SID}\x07{text}");
        s.on_output(r.as_bytes());
    }

    #[test]
    fn prompt_redraw_does_not_reshow_a_hidden_menu() {
        // Up at the top hides the menu until the next key; a redraw is not a key.
        let (mut s, pty) = session("{}");
        prompt(&mut s, "git ch");
        reply(&mut s, subs("ch"), None);
        s.on_input(b"\x1b[A");
        assert!(!s.menu.intercepting(&s.settings));
        let req = s.req;
        redraw(&mut s, "git ch");
        assert_eq!(
            s.req, req,
            "the unchanged line is not re-requested (its reply would show the menu)"
        );
        s.on_input(b"\r");
        assert_eq!(sent(&pty), b"\r");
        // A recalled line (a large change) stays hidden over a resize redraw: Enter runs it.
        let (mut s, pty) = session("{}");
        prompt(&mut s, "git ch");
        reply(&mut s, subs("ch"), None);
        s.on_output(b"\x08\x08\x1b[Kstatus --short ");
        let mut c = comp(
            "git status --short ",
            2,
            vec![item("my file.txt", SuggestionKind::File, "")],
        );
        c = c.with_request_id(s.req);
        s.on_completion(c);
        assert!(!s.menu.intercepting(&s.settings));
        let req = s.req;
        redraw(&mut s, "git status --short ");
        assert_eq!(s.req, req);
        s.on_input(b"\r");
        assert_eq!(sent(&pty), b"\r");
    }

    #[test]
    fn split_prompt_redraw_keeps_the_menu() {
        let (mut s, _) = session("{}");
        prompt(&mut s, "git ch");
        reply(&mut s, subs("ch"), None);
        // The redraw's line arrives in a second read: the empty line in between does not reset the menu.
        redraw(&mut s, "");
        s.on_output(b"git ch");
        assert!(s.menu.intercepting(&s.settings) && s.drawable());
        // A line that really is empty resets once the grace passed.
        redraw(&mut s, "");
        assert!(
            s.menu.has_items() && !s.drawable(),
            "held, not drawn, during the grace"
        );
        s.redraw_until = Some(Instant::now());
        s.on_timers();
        assert!(!s.menu.has_items());
    }

    #[test]
    fn unbracketed_paste_is_not_intercepted() {
        // pwsh never enables bracketed paste: a pasted line arrives as keys in one read.
        let (mut s, pty) = session("{}");
        prompt(&mut s, "git ");
        reply(&mut s, subs(""), None);
        assert!(s.menu.intercepting(&s.settings));
        s.on_input(b"checkout main\r");
        assert_eq!(sent(&pty), b"checkout main\r");
    }

    #[test]
    fn keys_after_an_unmodelled_key_wait_for_its_echo() {
        // Backspace then Enter before the echo: Enter acts on the menu for the line the shell then has.
        let (mut s, pty) = session("{}");
        prompt(&mut s, "git che");
        reply(&mut s, subs("che"), None);
        s.on_input(b"\x7f");
        s.on_input(b"\r");
        assert_eq!(sent(&pty), b"\x7f", "Enter is held");
        s.on_output(b"\x08\x1b[K");
        reply(&mut s, subs("ch"), None);
        assert_eq!(shell_line("git ch", &sent(&pty)), "git cherry-pick");
        // Ghost: Backspace, `e`, Right in one read gives `git ce`, not the ghost computed for `git ch`.
        let (mut s, pty) = session("{}");
        prompt(&mut s, "git ch");
        reply(&mut s, vec![], Some("eckout feature-login"));
        s.on_input(b"\x7fe\x1b[C");
        assert_eq!(sent(&pty), b"\x7fe");
        s.on_output(b"\x08\x1b[Ke");
        reply(&mut s, vec![], None);
        assert_eq!(sent(&pty), b"\x1b[C");
        // No echo at all (nothing changed): the held key goes out after the hold.
        let (mut s, pty) = session("{}");
        prompt(&mut s, "git ch");
        reply(&mut s, subs("ch"), None);
        s.on_input(b"\x1bOC\t");
        assert_eq!(sent(&pty), b"\x1bOC");
        s.held_at = Some(Instant::now());
        s.on_timers();
        assert_eq!(sent(&pty), b"e", "Tab inserted the common prefix");
    }

    #[test]
    fn slow_echo_keeps_the_insertion_in_the_basis() {
        // Tab inserts `kout`; the shell has echoed nothing when the 100 ms lock runs out, then Right accepts the ghost.
        let (mut s, pty) = session("{}");
        prompt(&mut s, "git chec");
        let items = ["checkout", "checkout-index"]
            .iter()
            .map(|n| item(n, SuggestionKind::Subcommand, "chec"))
            .collect();
        reply(&mut s, items, Some("kout feature-login"));
        s.on_input(b"\t");
        s.lock.as_mut().unwrap().0 = Instant::now();
        s.on_timers();
        assert!(s.lock.is_some(), "re-armed: nothing echoed");
        s.on_input(b"\x1b[C");
        assert_eq!(
            shell_line("git chec", &sent(&pty)),
            "git checkout feature-login"
        );
        // Never past the cap.
        let l = s.lock.as_mut().unwrap();
        (l.0, l.1) = (Instant::now(), Instant::now());
        s.on_timers();
        assert!(s.lock.is_none());
    }

    #[test]
    fn enter_records_the_submitted_line() {
        let (mut s, _) = session("{}");
        prompt(&mut s, "echo !");
        s.on_input(b"!\r");
        assert_eq!(s.tracker.marker.submitted(), Some("echo !!"));
        // transient-prompt-execute-insertion-pollutes-history: an insertion that executes (the ↪ row) records the line
        // it builds, not the transient prompt (p10k, oh-my-posh) redrawn without markers in front of it.
        let (mut s, _) = session("{}");
        let (tx, rx) = mpsc::channel();
        s.engine = Rc::new(RecordingCompletionEngine::new(tx));
        prompt(&mut s, "git ch");
        reply(&mut s, subs("ch"), None);
        s.tracker.marker.set_submitted(Some("stale".into()));
        let fx = s.menu.handle(KeyAction::InsertSelectedAndExecute, &s.settings);
        let Some(MenuEffect::Insert { ins, .. }) = fx.into_iter().next() else {
            panic!("no insertion")
        };
        s.insert(&ins, None, &mut Vec::new());
        assert_eq!(s.tracker.marker.submitted(), Some("git cherry-pick"));
        let run = format!("\r\x1b[K\u{276f} git cherry-pick\r\n\x1b]697;OSCLock={SID}\x07\x1b]697;PreExec\x07x\r\n");
        s.on_output(format!("{run}\x1b]697;OSCUnlock={SID}\x07").as_bytes());
        prompt(&mut s, "");
        let done: Vec<_> = rx
            .try_iter()
            .filter_map(|m| {
                if let CompletionEngineCall::CommandFinished(h) = m {
                    Some(h.command().to_string())
                } else {
                    None
                }
            })
            .collect();
        assert_eq!(done, ["git cherry-pick"]);
    }

    #[test]
    fn enter_clears_the_menu_before_the_line_runs() {
        // enter-redraws-stale-menu-below-line: the newline echo must not re-anchor the menu one row below the line.
        let (mut s, pty) = session("{}");
        prompt(&mut s, "echo one");
        reply(&mut s, vec![], None);
        assert!(
            s.menu.shown(&s.settings) && !s.menu.intercepting(&s.settings),
            "the argument hint row"
        );
        s.on_input(b"\r");
        assert_eq!(sent(&pty), b"\r");
        assert!(!s.menu.shown(&s.settings));
        s.sink.clear();
        s.on_output(b"\r\n");
        assert!(
            !String::from_utf8_lossy(&s.sink).contains("path"),
            "{:?}",
            String::from_utf8_lossy(&s.sink)
        );
    }

    #[test]
    fn no_history_command_still_ends() {
        // r3-nohistory-skips-command-finish: the engine's end-of-command reset runs; nothing is recorded.
        let (mut s, _) = session("{}");
        let (tx, rx) = mpsc::channel();
        s.engine = Rc::new(RecordingCompletionEngine::new(tx));
        prompt(&mut s, " git branch -D fe");
        s.on_output(b"\r\n\x1b]697;NoHistory\x07\x1b]697;PreExec\x07Deleted branch\r\n");
        prompt(&mut s, "");
        let msgs: Vec<_> = rx
            .try_iter()
            .filter(|m| !matches!(m, CompletionEngineCall::Complete(_)))
            .collect();
        assert!(
            matches!(&msgs[..], [CompletionEngineCall::CommandEnded]),
            "{}",
            msgs.len()
        );
    }

    #[test]
    fn inline_tui_below_the_prompt_is_not_the_line() {
        // inline-tui-scraped-as-command-line: fzf --height (zsh Ctrl-T) draws its query row below the prompt.
        let (mut s, _) = session("{}");
        prompt(&mut s, "");
        let req = s.req;
        s.on_output(b"\r\n  git ch");
        assert_eq!((s.req, s.cur.clone()), (req, None));
        // fzf exits; the widget redraws the prompt and the line.
        s.on_output(format!("\r\x1b[J\x1b[A\x1b]697;StartPrompt\x07$ \x1b]697;EndPrompt\x07\x1b]697;NewCmd={SID}\x07git ch").as_bytes());
        assert_eq!(s.sent.as_ref().map(|(b, _)| b.as_str()), Some("git ch"));
        // R4-7: Ctrl-T after typed text, with the line's menu open: hidden over fzf, and fzf gets the keys.
        let (mut s, pty) = session("{}");
        prompt(&mut s, "git ch");
        reply(&mut s, subs("ch"), Some("eckout"));
        s.sink.clear();
        s.on_output(b"\r\n  1/2\r\n> rea");
        assert!(s.menu.intercepting(&s.settings) && !s.drawable());
        assert!(
            !String::from_utf8_lossy(&s.sink).contains("checkout"),
            "{:?}",
            String::from_utf8_lossy(&s.sink)
        );
        s.on_input(b"\x1b[B\r\x1b[C");
        assert_eq!(sent(&pty), b"\x1b[B\r\x1b[C");
        // Nor does a hidden menu's show-on-Tab take fzf's Tab.
        let (mut s, pty) = session(r#"{"autocomplete.onlyShowOnTab": true}"#);
        prompt(&mut s, "git ch");
        reply(&mut s, subs("ch"), None);
        s.on_output(b"\r\n  1/2\r\n> rea");
        s.on_input(b"\t");
        assert_eq!(sent(&pty), b"\t");
    }

    #[test]
    fn pwsh_list_view_owns_the_rows_below() {
        let (mut s, pty) = session("{}");
        prompt(&mut s, "git ch");
        reply(&mut s, subs("ch"), None);
        s.tracker.marker.set_pwsh_list_view(true);
        assert!(!s.drawable());
        s.sink.clear();
        s.redraw();
        assert!(
            !String::from_utf8_lossy(&s.sink).contains("checkout"),
            "menu not drawn"
        );
        s.on_input(b"\r");
        assert_eq!(sent(&pty), b"\r");
        let (mut s, _) = session("{}");
        prompt(&mut s, "");
        s.redraw_until = None;
        s.tracker.marker.set_pwsh_list_view(true);
        assert!(s.drawable(), "an empty line has no list");
    }

    // --- The session context `_ pre-cmd` reports, reloaded by `poll_files` (D17). ------------------------------

    use crate::shared::infrastructure::testing::temp_dir::TempDir;
    use std::collections::HashMap;

    /// A session reading its context from `<temp>/run/abc.json`, and what it sends the engine.
    fn session_reading_context_in(temp_dir: &TempDir) -> (Session, Receiver<CompletionEngineCall>) {
        let (mut s, _) = session("{}");
        let (engine, sent_to_engine) = mpsc::channel();
        s.engine = Rc::new(RecordingCompletionEngine::new(engine));
        s.session_id = SessionId::new("abc".to_string()).ok();
        s.session_contexts = Box::new(FileSessionContextRepository::new(DataDirectory::new(
            Some(temp_dir.path().to_path_buf()),
            None,
        )));
        (s, sent_to_engine)
    }

    fn write_context_file(temp_dir: &TempDir, json: &str, modified: SystemTime) {
        let path = temp_dir.path().join("run/abc.json");
        std::fs::create_dir_all(temp_dir.path().join("run")).unwrap();
        std::fs::write(&path, json).unwrap();
        std::fs::File::options()
            .write(true)
            .open(&path)
            .unwrap()
            .set_modified(modified)
            .unwrap();
    }

    fn contexts_sent(sent_to_engine: &Receiver<CompletionEngineCall>) -> Vec<(HashMap<String, String>, Option<String>)> {
        sent_to_engine
            .try_iter()
            .map(|m| match m {
                CompletionEngineCall::ApplySessionContext { env, alias } => (env, alias),
                _ => panic!("only session contexts are sent"),
            })
            .collect()
    }

    fn at(secs: u64) -> SystemTime {
        UNIX_EPOCH + Duration::from_secs(secs)
    }

    #[test]
    fn should_send_the_engine_the_env_and_alias_of_a_saved_session_context() {
        let temp_dir = TempDir::new("pty-context-sent");
        let (mut s, sent_to_engine) = session_reading_context_in(&temp_dir);
        write_context_file(&temp_dir, r#"{"env":{"A":"1"},"alias":"ll='ls -l'","ts":7}"#, at(1_000));

        s.poll_files();

        assert_eq!(
            contexts_sent(&sent_to_engine),
            [(
                HashMap::from([("A".to_string(), "1".to_string())]),
                Some("ll='ls -l'".to_string())
            )]
        );
    }

    #[test]
    fn should_not_send_the_session_context_again_while_its_file_keeps_its_modification_time() {
        let temp_dir = TempDir::new("pty-context-unchanged");
        let (mut s, sent_to_engine) = session_reading_context_in(&temp_dir);
        write_context_file(&temp_dir, r#"{"env":{"A":"1"}}"#, at(1_000));
        s.poll_files();
        contexts_sent(&sent_to_engine);

        s.poll_files();

        assert_eq!(contexts_sent(&sent_to_engine), Vec::new());
    }

    /// A removed file is no change: its context is not reloaded until the file comes back with a new time.
    #[test]
    fn should_send_nothing_and_keep_the_modification_time_once_the_context_file_is_deleted() {
        let temp_dir = TempDir::new("pty-context-deleted");
        let (mut s, sent_to_engine) = session_reading_context_in(&temp_dir);
        write_context_file(&temp_dir, r#"{"env":{"A":"1"}}"#, at(1_000));
        s.poll_files();
        contexts_sent(&sent_to_engine);
        std::fs::remove_file(temp_dir.path().join("run/abc.json")).unwrap();

        s.poll_files();

        assert_eq!(
            (contexts_sent(&sent_to_engine), s.session_mtime),
            (Vec::new(), Some(at(1_000)))
        );
    }

    /// The time is kept before the file is read, so a file that cannot be read is not read again until it changes.
    #[test]
    fn should_send_nothing_for_a_context_file_that_is_not_json_but_keep_its_new_modification_time() {
        let temp_dir = TempDir::new("pty-context-bad-json");
        let (mut s, sent_to_engine) = session_reading_context_in(&temp_dir);
        write_context_file(&temp_dir, r#"{"env":{"A":"1"}}"#, at(1_000));
        s.poll_files();
        contexts_sent(&sent_to_engine);
        write_context_file(&temp_dir, "not json", at(2_000));

        s.poll_files();

        assert_eq!(
            (contexts_sent(&sent_to_engine), s.session_mtime),
            (Vec::new(), Some(at(2_000)))
        );
    }

    #[test]
    fn should_send_the_session_context_again_once_its_file_is_rewritten_with_a_new_modification_time() {
        let temp_dir = TempDir::new("pty-context-rewritten");
        let (mut s, sent_to_engine) = session_reading_context_in(&temp_dir);
        write_context_file(&temp_dir, r#"{"env":{"A":"1"}}"#, at(1_000));
        s.poll_files();
        contexts_sent(&sent_to_engine);
        write_context_file(&temp_dir, r#"{"env":{"B":"2"},"alias":""}"#, at(2_000));

        s.poll_files();

        assert_eq!(
            contexts_sent(&sent_to_engine),
            [(
                HashMap::from([("B".to_string(), "2".to_string())]),
                Some(String::new())
            )]
        );
    }

    #[cfg(windows)]
    #[test]
    fn conpty_is_closed_off_the_main_loop() {
        struct Master(mpsc::Sender<thread::ThreadId>);
        impl MasterPty for Master {
            fn resize(&self, _: PtySize) -> anyhow::Result<()> {
                Ok(())
            }
            fn get_size(&self) -> anyhow::Result<PtySize> {
                Ok(PtySize::default())
            }
            fn try_clone_reader(&self) -> anyhow::Result<Box<dyn Read + Send>> {
                bail!("unused")
            }
            fn take_writer(&self) -> anyhow::Result<Box<dyn Write + Send>> {
                bail!("unused")
            }
        }
        // ClosePseudoConsole runs here and may wait for the reader, which waits for credits from the main loop.
        impl Drop for Master {
            fn drop(&mut self) {
                let _ = self.0.send(thread::current().id());
            }
        }
        let (mut s, _) = session("{}");
        let (tx, rx) = mpsc::channel();
        s.master = Some(Box::new(Master(tx)));
        assert!(s.step(Msg::ChildExit(0), &mut None));
        let on = rx.recv_timeout(Duration::from_secs(5)).expect("closed");
        assert_ne!(on, thread::current().id());
    }

    // --- Phase 9 characterisation: what the Session writes to stdout, to the PTY and to the engine, step by step. --
    // A scenario drives the session only through `Rig`, and forces a deadline only through a `poke_*` helper, so a
    // later move of the session edits the Rig and the helpers and leaves the scenario bodies and fixtures alone.

    use crate::shared::infrastructure::testing::fixture_log::assert_matches_fixture;

    fn describe(call: &CompletionEngineCall) -> String {
        match call {
            CompletionEngineCall::Complete(r) => format!(
                "Complete(id={} buffer={:?} cursor={} shell={:?} history_mode={})",
                r.id, r.buffer, r.cursor, r.shell, r.history_mode
            ),
            CompletionEngineCall::CommandFinished(h) => format!(
                "CommandFinished(command={:?} exit_code={:?} pid={:?})",
                h.command(),
                h.exit_code(),
                h.pid()
            ),
            CompletionEngineCall::CommandEnded => "CommandEnded".into(),
            CompletionEngineCall::RecordInsertion { command, name } => format!("RecordInsertion({command:?}, {name:?})"),
            CompletionEngineCall::RequestLlmGhostText { request, buffer, shell, .. } => {
                format!("RequestLlmGhostText(request={request} buffer={buffer:?} shell={shell:?})")
            }
            CompletionEngineCall::ApplySettings(s) => format!("ApplySettings({} entries)", s.entries().count()),
            CompletionEngineCall::ApplySessionContext { env, alias } => {
                let mut env: Vec<_> = env.iter().collect();
                env.sort();
                format!("ApplySessionContext({env:?}, {alias:?})")
            }
            CompletionEngineCall::ToggleFuzzySearch => "ToggleFuzzySearch".into(),
            CompletionEngineCall::ShutDown => "ShutDown".into(),
        }
    }

    /// A session plus a log of what each step wrote to stdout, to the PTY and to the engine.
    /// `inbox`, `credit_returns` and `stash` are the live ends of the session's message channel, for the scenarios
    /// that drive `Session::step`.
    struct Rig {
        s: Session,
        pty: Receiver<Vec<u8>>,
        engine: Receiver<CompletionEngineCall>,
        log: String,
        mark: usize,
        inbox: Sender<Msg>,
        credit_returns: Receiver<()>,
        stash: Option<Msg>,
    }

    impl Rig {
        fn new(settings_json: &str) -> Rig {
            let (mut s, pty) = session(settings_json);
            let (calls, engine) = mpsc::channel();
            s.engine = Rc::new(RecordingCompletionEngine::new(calls));
            let (inbox, rx) = mpsc::channel();
            let (credits, credit_returns) = mpsc::sync_channel(PTY_CREDITS);
            s.rx = rx;
            s.credits = credits;
            Rig { s, pty, engine, log: String::new(), mark: 0, inbox, credit_returns, stash: None }
        }

        fn read_settings_from(&mut self, temp_dir: &TempDir) {
            self.s.settings_repository =
                Box::new(JsonFileSettingsRepository::new(DataDirectory::new(Some(temp_dir.path().to_path_buf()), None)));
        }

        fn sent(&self) -> Vec<u8> {
            sent(&self.pty)
        }

        fn note(&mut self, step: &str) {
            let out = self.s.sink[self.mark..].to_vec();
            self.mark = self.s.sink.len();
            let engine: Vec<String> = self.engine.try_iter().map(|c| describe(&c)).collect();
            self.log += &format!(
                "{step}\n  stdout={}\n  pty={}\n  engine={engine:?}\n",
                out.escape_ascii(),
                self.sent().escape_ascii()
            );
        }

        fn record(&mut self, line: &str) {
            self.log += &format!("  {line}\n");
        }

        fn finish(self, fixture: &str) {
            assert_matches_fixture(&format!("terminal_session/characterisation/{fixture}.log"), &self.log);
        }

        fn prompt(&mut self, text: &str) {
            prompt(&mut self.s, text);
        }

        fn reply(&mut self, items: Vec<Suggestion>, ghost: Option<&str>) {
            reply(&mut self.s, items, ghost);
        }

        fn reply_error(&mut self, buffer: &str, cursor: usize) {
            let request = self.s.req;
            self.s.on_completion(CompletionResult::error(request, buffer.into(), cursor));
        }

        fn reply_loading(&mut self, buffer: &str, items: Vec<Suggestion>) {
            let loading = comp(buffer, 1, items).with_request_id(self.s.req).with_loading(true);
            self.s.on_completion(loading);
        }

        fn request_id(&self) -> u64 {
            self.s.req
        }

        fn input(&mut self, bytes: &[u8]) {
            self.s.on_input(bytes);
        }

        fn output(&mut self, bytes: &[u8]) {
            self.s.on_output(bytes);
        }

        fn complete(&mut self, result: CompletionResult) {
            self.s.on_completion(result);
        }

        fn tick(&mut self) {
            self.s.on_timers();
        }

        fn redraw(&mut self) {
            self.s.redraw();
        }

        fn resize(&mut self, columns: u16, rows: u16) {
            self.s.on_resize(columns, rows);
        }

        fn end(&mut self) {
            self.s.cleanup();
        }

        fn poke_lock_due(&mut self) {
            self.s.lock.as_mut().unwrap().0 = Instant::now();
        }

        fn poke_resize_due(&mut self) {
            self.s.resize_at = Some(Instant::now());
        }

        fn poke_llm_due(&mut self) {
            self.s.llm_at = Some(Instant::now());
        }

        fn poke_poll_due(&mut self) {
            self.s.poll_at = Instant::now();
        }

        fn poke_hold_due(&mut self) {
            self.s.held_at = Some(Instant::now());
        }

        fn poke_spinner_due(&mut self) {
            self.s.spinner_at = Some(Instant::now());
        }

        fn poke_probe_window(&mut self) {
            self.s.probe_until = Some(Instant::now() + LATE_PROBE);
        }

        fn step_pty(&mut self, bytes: &[u8]) -> bool {
            self.s.step(Msg::Pty(bytes.to_vec()), &mut self.stash)
        }

        fn step_stdin(&mut self, bytes: &[u8]) -> bool {
            self.s.step(Msg::Stdin(bytes.to_vec()), &mut self.stash)
        }

        fn step_resize(&mut self, columns: u16, rows: u16) -> bool {
            self.s.step(Msg::Resize(columns, rows), &mut self.stash)
        }

        fn step_completion(&mut self, result: CompletionResult) -> bool {
            self.s.step(Msg::Completion(Box::new(result)), &mut self.stash)
        }

        fn step_ghost(&mut self, request: u64, text: Option<String>) -> bool {
            self.s.step(Msg::Ghost { req: request, text }, &mut self.stash)
        }

        fn step_child_exit(&mut self, code: i32) -> bool {
            self.s.step(Msg::ChildExit(code), &mut self.stash)
        }

        fn step_pty_eof(&mut self) -> bool {
            self.s.step(Msg::PtyEof, &mut self.stash)
        }

        fn step_stashed(&mut self) -> bool {
            let message = self.stash.take().expect("a stashed message");
            self.s.step(message, &mut self.stash)
        }

        fn step_queued(&mut self) -> bool {
            let message = self.s.rx.try_recv().expect("a queued message");
            self.s.step(message, &mut self.stash)
        }

        fn queue_pty(&mut self, bytes: &[u8]) {
            self.inbox.send(Msg::Pty(bytes.to_vec())).unwrap();
        }

        fn queue_stdin(&mut self, bytes: &[u8]) {
            self.inbox.send(Msg::Stdin(bytes.to_vec())).unwrap();
        }

        fn credits_returned(&self) -> usize {
            self.credit_returns.try_iter().count()
        }

        fn stash_is_stdin(&self) -> bool {
            matches!(self.stash, Some(Msg::Stdin(_)))
        }

        fn panic_in_the_guard(&mut self) {
            assert!(!self.s.guard(|_| panic!("injected")));
        }

        fn is_passthrough(&self) -> bool {
            self.s.passthrough
        }

        fn exit_code(&self) -> Option<i32> {
            self.s.exit_code
        }

        fn has_deadline(&self) -> bool {
            self.s.next_deadline().is_some()
        }
    }

    #[test]
    fn should_draw_and_insert_when_typing_with_a_menu_exactly_as_at_the_phase_9_start() {
        let mut rig = Rig::new("{}");
        rig.prompt("git ch");
        rig.note("prompt: git ch");
        rig.reply(subs("ch"), None);
        rig.note("two replies");
        rig.input(b"\x1b[B");
        rig.note("Down");
        rig.input(b"\t");
        rig.note("Tab");
        rig.output(b"erry-pick");
        rig.note("the echo");
        rig.input(b"\x1b");
        rig.note("Esc");
        rig.finish("typing");
    }

    #[test]
    fn should_ring_the_bell_before_the_frame_and_write_the_line_last_exactly_as_at_the_phase_9_start() {
        let mut rig = Rig::new(r#"{"autocomplete.keybindings.tab": "insertCommonPrefix"}"#);
        rig.prompt("git ");
        let items = ["add", "branch"].iter().map(|n| item(n, SuggestionKind::Subcommand, "")).collect();
        rig.reply(items, None);
        rig.note("menu for 'git '");
        rig.input(b"\tx");
        rig.note("Tab (Bell) and a typed x in one read");
        rig.finish("bell");
    }

    #[test]
    fn should_release_held_keys_after_the_frame_exactly_as_at_the_phase_9_start() {
        let mut rig = Rig::new("{}");
        rig.prompt("git che");
        rig.reply(subs("che"), None);
        rig.note("menu");
        rig.input(b"\x7f");
        rig.input(b"\r");
        rig.note("Backspace, Enter (held)");
        rig.output(b"\x08\x1b[K");
        rig.note("echo of the Backspace");
        rig.reply(subs("ch"), None);
        rig.note("reply for 'git ch': release_held after the frame");
        rig.finish("held");
    }

    #[test]
    fn should_report_a_command_cycle_to_the_engine_exactly_as_at_the_phase_9_start() {
        let mut rig = Rig::new("{}");
        rig.prompt("echo hi");
        rig.note("prompt");
        rig.input(b"\r");
        rig.note("Enter");
        rig.output(b"\r\n\x1b]697;PreExec\x07hi\r\n");
        rig.note("PreExec and output");
        rig.output(
            format!("\x1b]697;ExitCode=0\x07\x1b]697;StartPrompt\x07$ \x1b]697;EndPrompt\x07\x1b]697;NewCmd={SID}\x07")
                .as_bytes(),
        );
        rig.note("next prompt");
        rig.finish("lifecycle");
    }

    #[test]
    fn should_clear_the_overlay_and_shut_down_the_engine_at_the_end_exactly_as_at_the_phase_9_start() {
        let mut rig = Rig::new("{}");
        rig.prompt("git ch");
        rig.reply(subs("ch"), None);
        rig.note("menu");
        rig.end();
        rig.note("cleanup");
        rig.finish("cleanup");
    }

    #[test]
    fn should_accept_the_ghost_whole_and_by_word_exactly_as_at_the_phase_9_start() {
        let mut rig = Rig::new("{}");
        rig.prompt("echo hel");
        rig.reply(vec![], Some("lo wor ld"));
        rig.note("ghost lo wor ld");
        rig.input(b"\x1bf");
        rig.note("Alt-f: next word");
        rig.output(b"lo");
        rig.note("echo of the word");
        rig.input(b"\x1b[C");
        rig.note("Right: the rest");
        rig.finish("ghost");
    }

    #[test]
    fn should_keep_an_insertion_in_the_basis_until_it_is_echoed_exactly_as_at_the_phase_9_start() {
        let mut rig = Rig::new("{}");
        rig.prompt("git chec");
        let items = ["checkout", "checkout-index"].iter().map(|n| item(n, SuggestionKind::Subcommand, "chec")).collect();
        rig.reply(items, Some("kout feature-login"));
        rig.note("menu and ghost");
        rig.input(b"\t");
        rig.note("Tab inserts, lock armed");
        rig.poke_lock_due();
        rig.tick();
        rig.note("lock deadline: re-armed, nothing echoed");
        rig.input(b"\x1b[C");
        rig.note("Right: ghost on top of the insertion");
        rig.output(b"kout feature-login");
        rig.note("everything echoed");
        rig.input(b"e");
        rig.input(b"\r");
        rig.note("typed-ahead e, then Enter");
        rig.finish("lock");
    }

    #[test]
    fn should_repaint_once_the_resize_settled_exactly_as_at_the_phase_9_start() {
        let mut rig = Rig::new("{}");
        rig.prompt("git ch");
        rig.reply(subs("ch"), None);
        rig.note("menu");
        rig.resize(60, 18);
        rig.note("resize 60x18");
        rig.output(b"x");
        rig.note("output while resizing re-arms the quiet time");
        rig.poke_resize_due();
        rig.tick();
        rig.note("resize settled: repaint and frame");
        rig.finish("resize");
    }

    #[test]
    fn should_debounce_and_show_the_llm_ghost_text_exactly_as_at_the_phase_9_start() {
        let mut rig = Rig::new(
            r#"{"inline.source": "mock", "inline.profiles.mock": {"provider": "openai", "model": "m", "base_url": "http://127.0.0.1:9/v1"}}"#,
        );
        rig.prompt("zzzcmd foo");
        rig.note("prompt");
        let request = rig.request_id();
        rig.reply_error("zzzcmd foo", 10);
        rig.note("error reply arms the debounce");
        rig.poke_llm_due();
        rig.tick();
        rig.note("debounce due: LLM request");
        rig.step_ghost(request, Some(" bar".into()));
        rig.note("llm reply: ghost drawn");
        rig.step_ghost(request + 5, Some("stale".into()));
        rig.note("stale reply ignored");
        rig.finish("llm");
    }

    #[test]
    fn should_reload_the_settings_and_the_theme_from_the_file_exactly_as_at_the_phase_9_start() {
        let temp_dir = TempDir::new("reload-the-settings");
        let mut rig = Rig::new("{}");
        rig.read_settings_from(&temp_dir);
        rig.prompt("git ch");
        rig.reply(subs("ch"), None);
        rig.note("menu");
        std::fs::write(temp_dir.path().join("settings.json"), r#"{"autocomplete.theme":"light","autocomplete.height":2}"#).unwrap();
        rig.poke_poll_due();
        rig.tick();
        rig.note("settings file appeared: reload, theme, redraw");
        std::fs::remove_file(temp_dir.path().join("settings.json")).unwrap();
        rig.poke_poll_due();
        rig.tick();
        rig.note("settings file deleted: reload to the defaults (the time went Some -> None), redraw in the dark theme");
        rig.finish("reload");
    }

    #[test]
    fn should_theme_the_late_probe_reply_and_not_type_it_exactly_as_at_the_phase_9_start() {
        let mut rig = Rig::new(r#"{"autocomplete.theme": "system"}"#);
        rig.poke_probe_window();
        rig.prompt("git ch");
        rig.reply(subs("ch"), None);
        rig.note("menu on the dark filler theme");
        rig.input(b"\x1b]11;rgb:ffff/ffff/ffff\x1b\\\x1b[5;1R");
        rig.note("late reply");
        rig.redraw();
        rig.note("next frame");
        rig.input(b"\x1b[B");
        rig.note("Down: frame in the light theme");
        rig.finish("probe");
    }

    #[test]
    fn should_pass_everything_through_after_a_panic_exactly_as_at_the_phase_9_start() {
        let mut rig = Rig::new("{}");
        rig.prompt("git ch");
        rig.reply(subs("ch"), None);
        rig.note("menu");
        rig.panic_in_the_guard();
        rig.note("guard failed: the clear goes out");
        assert!(rig.step_pty(b"out\x1b]697;NewCmd\x07"));
        rig.note("raw batch in pass-through, markers kept");
        assert!(rig.step_stdin(b"\r"));
        rig.note("raw key");
        assert!(rig.step_resize(60, 20));
        rig.note("resize in pass-through");
        assert!(rig.step_completion(comp("git ch", 1, vec![])));
        rig.note("completion dropped");
        assert!(rig.step_child_exit(3));
        assert!(!rig.step_pty_eof());
        assert!(rig.is_passthrough());
        rig.record(&format!("exit_code={:?} deadline_set={}", rig.exit_code(), rig.has_deadline()));
        rig.finish("passthrough");
    }

    #[test]
    fn should_serve_the_spinner_after_two_hundred_milliseconds_exactly_as_at_the_phase_9_start() {
        let mut rig = Rig::new("{}");
        rig.prompt("git ch");
        rig.reply_loading("git ch", subs("ch"));
        rig.note("first loading reply arms the spinner timer");
        rig.reply_loading("git ch", subs("ch"));
        std::thread::sleep(Duration::from_millis(250));
        rig.poke_spinner_due();
        rig.tick();
        rig.note("after 250 ms: spinner tick and redraw");
        rig.poke_spinner_due();
        rig.tick();
        rig.note("next tick");
        rig.finish("spinner");
    }

    #[test]
    fn should_merge_queued_chunks_into_one_batch_exactly_as_at_the_phase_9_start() {
        let mut rig = Rig::new("{}");
        rig.queue_pty(b"\x1b]697;StartPrompt\x07$ ");
        rig.queue_pty(format!("\x1b]697;EndPrompt\x07\x1b]697;NewCmd={SID}\x07gi").as_bytes());
        rig.queue_stdin(b"t");
        rig.queue_pty(b" ch");
        assert!(rig.step_pty(b"\r\n"));
        rig.note("first step: 3 chunks merged, Stdin stashed");
        assert!(rig.stash_is_stdin());
        let returned = rig.credits_returned();
        rig.record(&format!("credits returned={returned}"));
        assert!(rig.step_stashed());
        rig.note("the stashed Stdin");
        assert!(rig.step_queued());
        rig.note("the last Pty");
        let returned = rig.credits_returned();
        rig.record(&format!("credits returned={returned}"));
        rig.finish("batching");
    }

    // --- Phase 9 loop guards: how `Session::step` and `run` batch, hand back credits and decide to leave. ---------
    // The tests drive the loop only through `LoopRig`, which builds the driver messages itself, so the move of the
    // driver rewrites the rig and leaves the test bodies alone.

    #[derive(Debug, PartialEq)]
    struct ExitState {
        code: Option<i32>,
        output_ended: bool,
        grace_armed: bool,
    }

    /// Switches the key-handling fault injection off again, also when the test fails.
    struct KeyHandlingPanic;

    impl Drop for KeyHandlingPanic {
        fn drop(&mut self) {
            PANIC_IN_INPUT.with(|p| p.set(false));
        }
    }

    /// An engine that panics when it is asked for completions and ignores everything else.
    struct PanickingCompletionEngine;

    impl CompletionEngine for PanickingCompletionEngine {
        fn complete(&self, _request: CompletionRequest) {
            panic!("the engine was asked");
        }
        fn command_finished(&self, _entry: crate::history::domain::entities::history_entry::HistoryEntry) {}
        fn command_ended(&self) {}
        fn record_insertion(&self, _command: String, _name: String) {}
        fn request_llm_ghost_text(&self, _request: u64, _buffer: String, _cwd: PathBuf, _shell: Shell) {}
        fn apply_settings(&self, _settings: Settings) {}
        fn apply_session_context(&self, _env: std::collections::HashMap<String, String>, _alias: Option<String>) {}
        fn toggle_fuzzy_search(&self) {}
        fn shut_down(&self) {}
    }

    /// The session of a loop test with a live inbox, plus what the loop did to the outside world.
    struct LoopRig {
        rig: Rig,
        stdout_mark: usize,
        stepped_before: Instant,
        stepped_after: Instant,
        key_handling_panic: Option<KeyHandlingPanic>,
        pty_slave: Option<Box<dyn portable_pty::SlavePty + Send>>,
    }

    fn loop_session() -> LoopRig {
        loop_session_reading("{}")
    }

    fn loop_session_with_an_llm() -> LoopRig {
        loop_session_reading(
            r#"{"inline.source": "mock", "inline.profiles.mock": {"provider": "openai", "model": "m", "base_url": "http://127.0.0.1:9/v1"}}"#,
        )
    }

    fn loop_session_reading(settings_json: &str) -> LoopRig {
        LoopRig {
            rig: Rig::new(settings_json),
            stdout_mark: 0,
            stepped_before: Instant::now(),
            stepped_after: Instant::now(),
            key_handling_panic: None,
            pty_slave: None,
        }
    }

    impl LoopRig {
        fn queue_pty(&mut self, bytes: &[u8]) {
            self.rig.queue_pty(bytes);
        }

        fn queue_stdin(&mut self, bytes: &[u8]) {
            self.rig.queue_stdin(bytes);
        }

        fn queue_resize(&mut self, columns: u16, rows: u16) {
            self.rig.inbox.send(Msg::Resize(columns, rows)).unwrap();
        }

        fn queue_completion(&mut self, result: CompletionResult) {
            self.rig.inbox.send(Msg::Completion(Box::new(result))).unwrap();
        }

        fn queue_ghost(&mut self, request: u64, text: Option<String>) {
            self.rig.inbox.send(Msg::Ghost { req: request, text }).unwrap();
        }

        fn queue_child_exit(&mut self, code: i32) {
            self.rig.inbox.send(Msg::ChildExit(code)).unwrap();
        }

        fn queue_pty_eof(&mut self) {
            self.rig.inbox.send(Msg::PtyEof).unwrap();
        }

        fn queue_term(&mut self) {
            self.rig.inbox.send(Msg::Term).unwrap();
        }

        fn disconnect(&mut self) {
            self.rig.inbox = mpsc::channel().0;
        }

        /// Takes the next queued message and handles it; false = leave the loop.
        fn step_next(&mut self) -> bool {
            self.stepped_before = Instant::now();
            let keeps_running = self.rig.step_queued();
            self.stepped_after = Instant::now();
            keeps_running
        }

        fn credits_returned(&self) -> usize {
            self.rig.credits_returned()
        }

        fn stash_is_stdin(&self) -> bool {
            self.rig.stash_is_stdin()
        }

        fn request_id(&self) -> u64 {
            self.rig.request_id()
        }

        fn exit_state(&self) -> ExitState {
            ExitState {
                code: self.rig.exit_code(),
                output_ended: self.rig.s.eof,
                grace_armed: self.rig.s.exit_at.is_some(),
            }
        }

        /// The exit deadline lies exactly `delay` after the last step began.
        fn exit_deadline_is_set_after(&self, delay: Duration) -> bool {
            self.rig
                .s
                .exit_at
                .is_some_and(|at| at >= self.stepped_before + delay && at <= self.stepped_after + delay)
        }

        /// What reached stdout since the last call.
        fn stdout(&mut self) -> Vec<u8> {
            let written = self.rig.s.sink[self.stdout_mark..].to_vec();
            self.stdout_mark = self.rig.s.sink.len();
            written
        }

        fn pty_writes(&self) -> Vec<u8> {
            self.rig.sent()
        }

        fn is_passthrough(&self) -> bool {
            self.rig.is_passthrough()
        }

        /// A menu for `git ch` is on screen.
        fn draw_menu(&mut self) {
            self.rig.prompt("git ch");
            self.rig.reply(subs("ch"), None);
            self.stdout();
        }

        /// Keys typed behind a pending echo wait in the session until the output shows it.
        fn hold_keys_behind_a_pending_echo(&mut self) {
            self.rig.prompt("git che");
            self.rig.reply(subs("che"), None);
            self.rig.input(b"\x7f");
            self.rig.input(b"\r");
            self.stdout();
            self.pty_writes();
        }

        /// A caught panic turns the session into a pass-through; the clear the guard writes is dropped.
        fn pass_through(&mut self) {
            self.rig.panic_in_the_guard();
            self.stdout();
        }

        fn panic_when_the_engine_is_asked(&mut self) {
            self.rig.s.engine = Rc::new(PanickingCompletionEngine);
        }

        fn panic_in_key_handling(&mut self) {
            PANIC_IN_INPUT.with(|p| p.set(true));
            self.key_handling_panic = Some(KeyHandlingPanic);
        }

        /// A real pseudo-terminal of 80x24 stands in for the child's.
        fn open_a_pty(&mut self) {
            let pair = portable_pty::native_pty_system()
                .openpty(PtySize { rows: 24, cols: 80, pixel_width: 0, pixel_height: 0 })
                .unwrap();
            self.rig.s.master = Some(pair.master);
            self.pty_slave = Some(pair.slave);
        }

        fn pty_size(&self) -> PtySize {
            self.rig.s.master.as_ref().unwrap().get_size().unwrap()
        }

        fn model_size(&self) -> (usize, usize) {
            (self.rig.s.term.columns(), self.rig.s.term.screen_lines())
        }

        /// A request is waiting for the debounce, which is due.
        fn arm_the_due_llm_debounce(&mut self) {
            self.rig.prompt("zzzcmd foo");
            self.rig.reply_error("zzzcmd foo", 10);
            self.rig.engine.try_iter().for_each(drop);
            self.rig.poke_llm_due();
        }

        /// Runs the loop to its end; returns the exit code and the engine calls it made.
        fn run(self) -> (i32, Vec<String>) {
            let rig = self.rig;
            let code = rig.s.run();
            let calls = rig.engine.try_iter().map(|c| describe(&c)).collect();
            (code, calls)
        }
    }

    /// What the renderer writes to take the three-row menu of `draw_menu` off the screen, the cursor ending in `column`.
    fn clear_of_the_menu(column: usize) -> Vec<u8> {
        let blank_row = " ".repeat(40);
        format!(
            "\x1b[?2026h\x1b[?25l\x1b[2;3H\x1b[0m{blank_row}\x1b[3;3H\x1b[0m{blank_row}\x1b[4;3H\x1b[0m{blank_row}\x1b[1;{column}H\x1b[0m\x1b[?25h\x1b[?2026l"
        )
        .into_bytes()
    }

    #[test]
    fn should_write_every_queued_pty_chunk_to_stdout_as_one_batch_when_passing_through() {
        let mut rig = loop_session();
        rig.pass_through();
        rig.queue_pty(b"one \x1b]697;NewCmd\x07");
        rig.queue_pty(b"two ");
        rig.queue_pty(b"three");

        let keeps_running = rig.step_next();

        assert!(keeps_running);
        assert_eq!(rig.stdout(), b"one \x1b]697;NewCmd\x07two three");
    }

    #[test]
    fn should_stash_the_first_message_that_is_not_pty_output_behind_the_batch() {
        let mut rig = loop_session();
        rig.pass_through();
        rig.queue_pty(b"one ");
        rig.queue_stdin(b"k");
        rig.queue_pty(b"two");

        rig.step_next();

        assert!(rig.stash_is_stdin());
        assert_eq!(rig.stdout(), b"one ");
    }

    #[test]
    fn should_return_one_credit_per_pty_chunk_after_handling_the_batch() {
        let mut rig = loop_session();
        rig.queue_pty(b"a");
        rig.queue_pty(b"b");
        rig.queue_pty(b"c");

        rig.step_next();

        assert_eq!(rig.credits_returned(), 3);
    }

    #[test]
    fn should_keep_running_at_pty_eof_until_the_child_has_exited() {
        let mut rig = loop_session();
        rig.queue_pty_eof();

        let keeps_running = rig.step_next();

        assert!(keeps_running);
        assert_eq!(rig.exit_state(), ExitState { code: None, output_ended: true, grace_armed: false });
    }

    #[test]
    fn should_leave_the_loop_at_pty_eof_once_the_child_has_exited() {
        let mut rig = loop_session();
        rig.queue_child_exit(7);
        rig.queue_pty_eof();
        rig.step_next();

        let keeps_running = rig.step_next();

        assert!(!keeps_running);
        assert_eq!(rig.exit_state(), ExitState { code: Some(7), output_ended: true, grace_armed: true });
    }

    #[test]
    fn should_leave_the_loop_when_the_child_exits_after_the_output_ended() {
        let mut rig = loop_session();
        rig.queue_pty_eof();
        rig.queue_child_exit(5);
        rig.step_next();

        let keeps_running = rig.step_next();

        assert!(!keeps_running);
        assert_eq!(rig.exit_state(), ExitState { code: Some(5), output_ended: true, grace_armed: false });
    }

    #[test]
    fn should_give_the_output_two_hundred_milliseconds_after_the_child_exits() {
        let mut rig = loop_session();
        rig.queue_child_exit(0);

        let keeps_running = rig.step_next();

        assert!(keeps_running);
        assert!(rig.exit_deadline_is_set_after(Duration::from_millis(200)));
    }

    #[test]
    fn should_exit_with_129_when_terminated() {
        let mut rig = loop_session();
        rig.queue_term();

        let keeps_running = rig.step_next();

        assert!(!keeps_running);
        assert_eq!(rig.exit_state(), ExitState { code: Some(129), output_ended: false, grace_armed: false });
    }

    #[test]
    fn should_ignore_engine_replies_while_passing_through() {
        let mut rig = loop_session();
        rig.draw_menu();
        rig.pass_through();
        let request = rig.request_id();
        rig.queue_completion(comp("git ch", 1, subs("ch")).with_request_id(request));
        rig.queue_ghost(request, Some("eckout".into()));

        rig.step_next();
        rig.step_next();

        assert_eq!((rig.stdout(), rig.pty_writes()), (vec![], vec![]));
    }

    #[test]
    fn should_resize_only_the_pty_while_passing_through() {
        let mut rig = loop_session();
        rig.open_a_pty();
        rig.pass_through();
        rig.queue_resize(100, 30);

        rig.step_next();

        assert_eq!(rig.pty_size(), PtySize { rows: 30, cols: 100, pixel_width: 0, pixel_height: 0 });
        assert_eq!(rig.model_size(), (80, 24));
    }

    #[test]
    fn should_run_the_timers_after_every_message_outside_passthrough() {
        let mut rig = loop_session_with_an_llm();
        rig.arm_the_due_llm_debounce();
        rig.queue_stdin(b"\x1b[C");
        rig.disconnect();

        let (exit_code, engine_calls) = rig.run();

        assert_eq!(exit_code, 0);
        assert_eq!(engine_calls.len(), 2, "{engine_calls:?}");
        assert!(engine_calls[0].starts_with("RequestLlmGhostText(request="), "{engine_calls:?}");
    }

    #[test]
    fn should_emit_the_cached_clear_then_the_raw_batch_and_return_the_credits_when_the_refresh_panics() {
        let mut rig = loop_session();
        rig.draw_menu();
        rig.panic_when_the_engine_is_asked();
        rig.queue_pty(b"e");
        rig.queue_pty(b"c\x1b]697;NewCmd\x07");

        rig.step_next();

        assert!(rig.is_passthrough());
        assert_eq!(rig.stdout(), [clear_of_the_menu(9), b"ec\x1b]697;NewCmd\x07".to_vec()].concat());
        assert_eq!(rig.credits_returned(), 2);
    }

    #[test]
    fn should_write_the_batch_again_raw_when_release_held_panics_after_the_frame() {
        let mut rig = loop_session();
        rig.hold_keys_behind_a_pending_echo();
        rig.panic_in_key_handling();
        rig.queue_pty(b"\r\n\x1b]697;PreExec\x07");

        rig.step_next();

        assert!(rig.is_passthrough());
        let processed = [clear_of_the_menu(10), b"\r\n".to_vec()].concat();
        let raw_again = b"\r\n\x1b]697;PreExec\x07".to_vec();
        assert_eq!(rig.stdout(), [processed, raw_again].concat());
    }

    #[test]
    fn should_generate_a_session_id_of_32_lowercase_hex_digits() {
        let id = session_id();

        assert_eq!(id.len(), 32);
        assert!(id.chars().all(|c| matches!(c, '0'..='9' | 'a'..='f')), "{id}");
        assert!(SessionId::new(id.clone()).is_ok(), "{id}");
    }
}
