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
use crate::shared::infrastructure::system_clock::SystemClock;
use crate::dropdown::domain::entities::menu::Menu;
use crate::dropdown::domain::value_objects::menu_effect::MenuEffect;
use crate::dropdown::domain::repositories::theme_repository::ThemeRepository;
use crate::dropdown::domain::services::theme_resolver::resolve_theme;
use crate::dropdown::infrastructure::adapters::file_theme_repository::FileThemeRepository;
use crate::terminal_session::application::frame_input::FrameInput;
use crate::terminal_session::application::ports::terminal_screen::TerminalScreen;
use crate::terminal_session::application::shell_output_feeder::ShellOutputFeeder;
use crate::terminal_session::domain::entities::echo_hold::EchoHold;
use crate::terminal_session::domain::entities::insertion_lock::InsertionLock;
use crate::terminal_session::domain::entities::marker_splitter::MarkerSplitter;
use crate::terminal_session::domain::entities::marker_tracker::MarkerTracker;
use crate::terminal_session::domain::services::insertion_reconciler::{expected_buffer, insertion_bytes};
use crate::terminal_session::domain::services::key_router::{ghost_key, is_ctrl};
use crate::terminal_session::domain::services::render_guard::allows_drawing;
use crate::terminal_session::domain::services::session_deadlines::{
    next_deadline, EXIT_GRACE, FILE_POLL, REDRAW_GRACE, RESIZE_DEBOUNCE, SPINNER_TICK,
};
use crate::terminal_session::domain::value_objects::lock_settlement::LockSettlement;
use crate::terminal_session::domain::value_objects::screen_state::ScreenState;
use crate::terminal_session::infrastructure::adapters::alacritty_terminal_screen::AlacrittyTerminalScreen;
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
use crate::shared::domain::value_objects::shell::Shell;
use crate::terminal_session::domain::repositories::session_context_repository::SessionContextRepository;
use crate::terminal_session::domain::value_objects::session_id::SessionId;
use crate::terminal_session::infrastructure::adapters::file_session_context_repository::FileSessionContextRepository;
use crate::terminal_session::infrastructure::adapters::kitty_graphics;
use crate::terminal_session::domain::value_objects::wrapped_shell_command::WrappedShellCommand;
use crate::terminal_session::domain::entities::key_decoder::KeyDecoder;
use crate::terminal_session::application::ports::completion_engine::CompletionEngine;
use crate::completion::application::completion_request::CompletionRequest;
use crate::completion::domain::value_objects::insertion::Insertion;
use crate::completion::domain::value_objects::completion_phase::CompletionPhase;
use crate::completion::domain::value_objects::completion_result::CompletionResult;
use crate::terminal_session::infrastructure::pty::completion_engine_starter::CompletionEngineStarter;
use crate::terminal_session::infrastructure::pty::pty_reader::spawn_pty_reader;
use crate::terminal_session::infrastructure::pty::terminal_probe::{parse_probe, PROBE, PROBE_TIMEOUT};
use crate::dropdown::domain::value_objects::rgb::Rgb;
use crate::terminal_session::infrastructure::pty::session_message::SessionMessage;
use anyhow::{anyhow, bail, Context, Result};
use portable_pty::{CommandBuilder, MasterPty, PtySize};
use std::io::{IsTerminal, Write};
use std::panic::AssertUnwindSafe;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender, SyncSender};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
#[cfg(unix)]
use crate::terminal_session::infrastructure::pty::platform_unix as sys;
#[cfg(windows)]
use crate::terminal_session::infrastructure::pty::platform_windows as sys;

/// After a probe timeout, a late reply arriving within this window is dropped instead of typed into the shell.
const LATE_PROBE: Duration = Duration::from_secs(3);

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
    let command = WrappedShellCommand::new(shell.as_deref(), &cmd, login);
    let mut modes = None;
    let res = std::panic::catch_unwind(AssertUnwindSafe(|| {
        Session::start(&command, &mut modes, start).map(Session::run)
    }));
    let err = match res {
        Ok(Ok(code)) => return code,
        Ok(Err(e)) => e,
        Err(_) => anyhow!("internal error (see pty.log)"),
    };
    if let Some(m) = modes {
        m.restore();
    }
    fallback(&err, command.argv())
}

fn env_nonempty(k: &str) -> Option<String> {
    std::env::var(k).ok().filter(|v| !v.is_empty())
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
    rx: Receiver<SessionMessage>,
    engine: Rc<dyn CompletionEngine>,
    writer: Sender<Vec<u8>>,
    /// Returns PTY read credits to the reader thread (see `pump`).
    credits: SyncSender<()>,
    /// Dropped after the child exits on Windows to unblock the ConPTY reader.
    master: Option<Box<dyn MasterPty + Send>>,
    modes: sys::Modes,
    /// Splits the shell's output into bytes, window names and OSC 697 markers; keeps the cut sequences between reads.
    splitter: MarkerSplitter,
    /// Shell-integration state of the session (SPEC 5.1).
    marker: MarkerTracker,
    /// The model, the filter and the renderer of the session, behind the screen port.
    screen: Rc<dyn TerminalScreen>,
    /// Drives the splitter, the marker tracker and the screen over one PTY batch.
    feeder: ShellOutputFeeder,
    menu: Menu,
    /// Where `autocomplete.theme` finds a custom theme file; read again at every theme resolution, never cached (P8-T25).
    themes: Rc<dyn ThemeRepository>,
    keymap: Keymap,
    /// Kept across stdin reads: a bracketed paste split over several reads stays one opaque unit (SPEC §7.2.1).
    decoder: KeyDecoder,
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
    /// Insertion lock (SPEC §7.3), re-armed up to its cap while the shell has echoed nothing of the insertion.
    lock: Option<InsertionLock>,
    /// What was sent ahead of the shell's echo and the keys held back (SPEC §7.3).
    echo: EchoHold,
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
    fn start(command: &WrappedShellCommand, modes: &mut Option<sys::Modes>, start: CompletionEngineStarter) -> Result<Session> {
        if !std::io::stdin().is_terminal() || !std::io::stdout().is_terminal() {
            bail!("stdin/stdout is not a terminal");
        }
        let program = command
            .program()
            .context("no shell to run (set SHELL or pass --shell)")?;
        let settings_repository: Box<dyn SettingsRepository> =
            Box::new(JsonFileSettingsRepository::new(data_directory_from_env()));
        let settings = load_or_defaults(&*settings_repository);
        let (m, lf_to_ind) = sys::enter()?;
        *modes = Some(m);
        let (cols, rows) = crossterm::terminal::size().context("terminal size")?;
        let (cols, rows) = (cols.max(1), rows.max(1));
        let sid = session_id();
        let id = SessionId::new(sid.clone()).expect("generated ids are valid");
        let shell = Shell::from_name(program).unwrap_or(if cfg!(windows) {
            Shell::Pwsh
        } else {
            Shell::Bash
        });

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
        if cpr.is_none() {
            write_stdout(b"\x1b[H\x1b[2J");
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
        let marker = MarkerTracker::new(
            id.clone(),
            Shell::from_name(program),
            cfg!(windows),
        );
        let screen = AlacrittyTerminalScreen::new(cols as usize, rows as usize, renderer, lf_to_ind);
        // Sync the model with the real cursor, through the same parser as the shell's output.
        if let Some((r, c)) = cpr {
            screen.advance_model(format!("\x1b[{r};{c}H").as_bytes());
        }
        let screen: Rc<dyn TerminalScreen> = Rc::new(screen);
        let feeder = ShellOutputFeeder::new(screen.clone(), Arc::new(SystemClock));
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
        let command = command
            .clone()
            .in_session(env!("CARGO_PKG_VERSION"), std::env::var_os("TMUX").is_some(), &id);
        let mut cb = CommandBuilder::from_argv(command.argv().iter().map(Into::into).collect());
        let cwd = std::env::current_dir()
            .ok()
            .filter(|d| d.is_dir())
            .or_else(|| user_home_from_env().ok());
        if let Some(cwd) = cwd {
            cb.cwd(cwd);
        }
        for (name, value) in command.env_to_set() {
            cb.env(name, value);
        }
        for name in command.env_to_remove() {
            cb.env_remove(name);
        }
        let mut child = pair.slave.spawn_command(cb)?;
        drop(pair.slave);
        let reader = pair.master.try_clone_reader()?;
        let mut pty_writer = pair.master.take_writer()?;
        logger::info(&format!(
            "pty: {:?} {cols}x{rows} sid {sid} cpr {cpr:?}",
            command.argv()
        ));

        write_stdout(&command.window_title());

        // Threads (SPEC §4.3).
        let credits = spawn_pty_reader(reader, tx.clone());
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
            let _ = t.send(SessionMessage::ChildExit(sys::wait(&mut *child)));
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
            splitter: MarkerSplitter::default(),
            marker,
            screen,
            feeder,
            menu,
            themes,
            keymap,
            decoder: KeyDecoder::default(),
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
            echo: EchoHold::default(),
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
    fn step(&mut self, msg: SessionMessage, stash: &mut Option<SessionMessage>) -> bool {
        match msg {
            SessionMessage::Stdin(b) if self.passthrough => {
                let _ = self.writer.send(b);
            }
            SessionMessage::Stdin(b) => {
                // on_input writes nothing before its last step: after a panic, keys the menu would not have taken go
                // out raw (an Enter meant for an item is dropped rather than running the line).
                let raw = !self.menu.intercepting(&self.settings);
                if !self.guard(|s| s.on_input(&b)) && raw {
                    let _ = self.writer.send(b);
                }
            }
            SessionMessage::Pty(mut b) => {
                // One batch = everything already queued (one model pass, one stdout write).
                let mut chunks = 1;
                loop {
                    match self.rx.try_recv() {
                        Ok(SessionMessage::Pty(more)) => {
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
            SessionMessage::PtyEof => {
                self.eof = true;
                return self.exit_code.is_none();
            }
            SessionMessage::Resize(cols, rows) if self.passthrough => {
                if let Some(m) = &self.master {
                    let _ = m.resize(PtySize {
                        rows,
                        cols,
                        pixel_width: 0,
                        pixel_height: 0,
                    });
                }
            }
            SessionMessage::Resize(c, r) => {
                self.guard(|s| s.on_resize(c, r));
            }
            SessionMessage::ChildExit(c) => {
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
            SessionMessage::Term => {
                self.exit_code = Some(129);
                return false;
            }
            SessionMessage::Completion(_) | SessionMessage::Ghost { .. } if self.passthrough => {}
            SessionMessage::Completion(c) => {
                self.guard(|s| s.on_completion(*c));
            }
            SessionMessage::Ghost { req, text } => {
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
        let clear = self.screen.take_clear();
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
        let lock = self.lock.as_ref().map(|l| l.deadline());
        next_deadline(
            self.passthrough,
            self.exit_at,
            [
                self.resize_at,
                lock,
                self.llm_at,
                self.spinner_at,
                Some(self.poll_at),
                self.redraw_until,
                self.echo.deadline(),
            ],
        )
    }

    fn alt(&self) -> bool {
        self.screen.state().alt_screen()
    }

    fn cwd(&self) -> PathBuf {
        self.marker
            .cwd()
            .map(|p| p.to_path_buf())
            .or_else(|| std::env::current_dir().ok())
            .unwrap_or_default()
    }

    fn shell(&self) -> Shell {
        self.marker.shell().unwrap_or(self.shell)
    }

    /// PTY batch (SPEC §4.5): clear, bytes (markers stripped), frame — one stdout write.
    fn on_output(&mut self, batch: &[u8]) {
        if self.resize_at.is_some() {
            self.resize_at = Some(Instant::now() + RESIZE_DEBOUNCE); // the shell / ConPTY is still redrawing
        }
        let was_alt = self.alt();
        let mut body = Vec::new();
        let events = self
            .feeder
            .feed(&mut self.splitter, &mut self.marker, batch, &mut body);
        for ev in events {
            self.on_event(ev);
        }
        if was_alt && !self.alt() {
            self.menu.error_reset();
        }
        self.refresh();
        // Taken late: a panic above leaves the clear cached for `guard`.
        let mut out = self.screen.take_clear();
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
        self.echo.reset();
        self.ghost = None;
        self.llm = None;
        self.llm_at = None;
        self.lock = None;
    }

    /// Re-read the edit buffer; request completions when it changed (SPEC §4.5; held while insertion-locked).
    fn refresh(&mut self) {
        // Off the command line the cursor is in a widget's inline TUI (fzf, fish's pager; see TerminalScreen::edit_buffer). Live
        // UI only: pre_exec still scrapes after a transient prompt, which is redrawn without markers.
        let new = if self.alt() {
            None
        } else {
            self.marker
                .edit_buffer_context()
                .and_then(|context| self.screen.edit_buffer(&context))
                .filter(|e| e.at_prompt())
        };
        let base = self
            .lock
            .as_ref()
            .map(|l| l.expected())
            .or(self.cur.as_ref().map(|e| e.buffer()));
        let n = new.as_ref().map(|e| e.buffer());
        self.echo.consume_echo(base, n, self.lock.is_some());
        self.cur = new;
        if let Some(lock) = &mut self.lock {
            let shown = self.cur.as_ref().map(|e| e.buffer());
            match lock.settle(shown, Instant::now()) {
                LockSettlement::StillHeld => return,
                LockSettlement::Released => self.lock = None,
            }
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
            self.echo.forget_pending(); // nothing stale left for held keys to act on
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
        self.echo.answered_by(c.request_id());
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
        if !self.settings.bool(INLINE_ENABLED) || self.marker.preexec() || self.alt() {
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
            .map(|l| l.expected())
            .or(self.cur.as_ref().map(|e| e.buffer()))?;
        Some(format!("{b}{}", self.echo.unechoed()))
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
                self.echo.typed_ahead(c)
            }
            _ => self.echo.forwarded_unmodelled(self.req),
        }
    }

    /// Replay held keys once the reply for the buffer after the unmodelled key came (or the hold timed out).
    fn release_held(&mut self) {
        let Some(held) = self.echo.release(Instant::now()) else {
            return;
        };
        self.on_keys(&held);
    }

    fn suppressed_in(&self, screen: &ScreenState) -> bool {
        !self.marker.at_prompt()
            // The cursor is off the command line (an inline TUI's rows: fzf, fish's pager): a menu for the line
            // stays hidden and keeps its hands off the keys.
            || self.cur.is_none()
            || self.resize_at.is_some()
            || self.repaint
            || screen.in_string()
            || self.redraw_until.is_some()
            // PSReadLine's ListView predictions own the rows below a non-empty line.
            || (self.marker.pwsh_list_view() && self.cur.as_ref().is_some_and(|e| !e.buffer().is_empty()))
    }

    /// No render guard (SPEC §7.5.1) holds, so a menu or ghost can be on screen and keys may act on it.
    /// ponytail: mirrors overlay_renderer.rs `compose`'s guards; its no-room-for-the-menu case (3-row terminal) is not covered.
    fn drawable(&self) -> bool {
        let screen = self.screen.state();
        !self.suppressed_in(&screen) && allows_drawing(&screen)
    }

    fn frame(&mut self) -> Vec<u8> {
        let ghost = self.ghost_text(false);
        let screen = self.screen.state();
        let input = FrameInput {
            settings: &self.settings,
            ghost: ghost.as_deref(),
            spinner: self.spinner,
            suppress: self.suppressed_in(&screen),
            scroll_region: screen.scroll_region(),
            now: Instant::now(),
        };
        self.screen.draw_overlay(&self.menu, &input)
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
                    self.screen
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
        if self.echo.is_holding() {
            self.echo.hold_behind(chunk); // keys keep their order behind held ones
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
            let prompt = !self.marker.preexec() && !self.alt() && self.cur.is_some() && typed < 2;
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
            if acts && self.echo.is_waiting_for_echo() {
                // The menu / ghost is for a buffer the shell may no longer have: wait for the echo and its reply.
                // ponytail: one hold buffer; separate reads replayed together count as one for the paste rule.
                self.echo.hold(&chunk[ev.bytes().start..], Instant::now());
                self.decoder.reset(); // held bytes start outside a paste and are decoded again
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
                            self.echo.accepted_ghost(g);
                            self.ghost = None;
                            self.llm = None;
                            redraw = true;
                        }
                        None => {
                            if key.code() == KeyCode::Enter && self.marker.at_prompt() {
                                self.marker.set_submitted(self.basis()); // history: the line, not what follows it
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
                        if b == b"\r" && self.marker.at_prompt() {
                            self.marker.set_submitted(self.basis()); // `execute`
                        }
                        fwd.extend(b);
                        self.echo.forget_unechoed();
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
        if self.marker.preexec() {
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
        self.echo.forget_unechoed();
        let expected = expected_buffer(ins, &ib, at);
        if ins.executes() {
            // As for Enter: preferred over a scrape with a transient prompt in front of the line; pre_exec still
            // reads a line the shell rewrote from the screen.
            self.marker.set_submitted(Some(expected.clone()));
        }
        let before = self
            .cur
            .as_ref()
            .map_or_else(|| ib.clone(), |e| e.buffer().to_string());
        let now = Instant::now();
        self.lock = Some(InsertionLock::new(now, expected, before));
        if let Some((cmd, name)) = record {
            self.engine.record_insertion(cmd, name);
        }
    }

    /// SPEC §7.6: resize PTY + model now, repaint after 150 ms of quiet.
    fn on_resize(&mut self, cols: u16, rows: u16) {
        if cols == 0
            || rows == 0
            || (cols as usize, rows as usize) == self.screen.state().size()
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
        self.screen.resize(cols as usize, rows as usize);
        self.resize_at = Some(Instant::now() + RESIZE_DEBOUNCE);
    }

    /// Full repaint once the resize settled (postponed while on the alternate screen).
    fn take_repaint(&mut self) -> Vec<u8> {
        if !self.repaint || self.resize_at.is_some() || self.alt() {
            return Vec::new();
        }
        self.repaint = false;
        self.screen.full_repaint()
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
        if self.lock.as_ref().is_some_and(|l| l.is_due(now)) {
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
        self.screen
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
        let mut out = self.screen.take_clear();
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

#[cfg(test)]
mod tests {
    use super::*;

    // --- Session driven without a terminal: PTY output via `on_output`, keys via `on_input`. -------------------

    use crate::terminal_session::infrastructure::pty::pty_reader::PTY_CREDITS;
    use crate::completion::domain::test_fixtures::{comp, item, settings};
    use crate::dropdown::domain::repositories::theme_repository::InMemoryThemeRepository;
    use crate::settings::domain::repositories::settings_repository::InMemorySettingsRepository;
    use crate::shared::domain::value_objects::data_directory::DataDirectory;
    use crate::terminal_session::domain::repositories::session_context_repository::InMemorySessionContextRepository;
    use crate::terminal_session::application::ports::completion_engine::{CompletionEngineCall, RecordingCompletionEngine};
    use crate::completion::domain::value_objects::suggestion::Suggestion;
    use crate::specs::domain::value_objects::suggestion_kind::SuggestionKind;

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
        let marker = MarkerTracker::new(
            SessionId::new(SID.to_string()).unwrap(),
            Some(Shell::Bash),
            false,
        );
        let screen: Rc<dyn TerminalScreen> = Rc::new(AlacrittyTerminalScreen::for_tests(80, 24));
        let s = Session {
            rx: mpsc::channel().1,
            engine: Rc::new(RecordingCompletionEngine::new(mpsc::channel().0)),
            writer,
            credits: mpsc::sync_channel(PTY_CREDITS).0,
            master: None,
            modes: sys::Modes::default(),
            splitter: MarkerSplitter::default(),
            marker,
            feeder: ShellOutputFeeder::new(screen.clone(), Arc::new(SystemClock)),
            screen,
            menu: Menu::new(),
            themes: Rc::new(InMemoryThemeRepository::default()),
            keymap: Keymap::from_settings(&settings),
            decoder: KeyDecoder::default(),
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
            echo: EchoHold::default(),
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
        assert!(s.echo.unechoed().is_empty());
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
        let llm_reply = |req, text: Option<&str>| SessionMessage::Ghost { req, text: text.map(String::from) };

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
        assert!(s.step(SessionMessage::Pty(b"out\x1b]697;NewCmd\x07".to_vec()), &mut stash));
        assert_eq!(s.sink, b"out\x1b]697;NewCmd\x07");
        assert!(s.step(SessionMessage::Stdin(b"\r".to_vec()), &mut stash));
        assert_eq!(sent(&pty), b"\r", "keys reach the shell unintercepted");
        assert!(s.step(SessionMessage::ChildExit(3), &mut stash));
        assert!(!s.step(SessionMessage::PtyEof, &mut stash));
        assert_eq!((s.exit_code, s.next_deadline().is_some()), (Some(3), true));
    }

    #[test]
    fn panic_while_handling_keys_still_forwards_them() {
        let (mut s, pty) = session("{}");
        prompt(&mut s, "ls");
        PANIC_IN_INPUT.with(|p| p.set(true));
        assert!(s.step(SessionMessage::Stdin(b"BOOM\r".to_vec()), &mut None));
        PANIC_IN_INPUT.with(|p| p.set(false));
        assert!(s.passthrough);
        assert_eq!(sent(&pty), b"BOOM\r", "the read is not lost");
        // With the menu intercepting it is dropped: an Enter meant for an item never runs the line.
        let (mut s, pty) = session("{}");
        prompt(&mut s, "git ch");
        reply(&mut s, subs("ch"), None);
        PANIC_IN_INPUT.with(|p| p.set(true));
        s.step(SessionMessage::Stdin(b"\r".to_vec()), &mut None);
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
        s.echo.expire_hold_at(Instant::now());
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
        s.lock.as_mut().unwrap().expire_deadline_at(Instant::now());
        s.on_timers();
        assert!(s.lock.is_some(), "re-armed: nothing echoed");
        s.on_input(b"\x1b[C");
        assert_eq!(
            shell_line("git chec", &sent(&pty)),
            "git checkout feature-login"
        );
        // Never past the cap.
        let l = s.lock.as_mut().unwrap();
        l.expire_at(Instant::now(), Instant::now());
        s.on_timers();
        assert!(s.lock.is_none());
    }

    #[test]
    fn enter_records_the_submitted_line() {
        let (mut s, _) = session("{}");
        prompt(&mut s, "echo !");
        s.on_input(b"!\r");
        assert_eq!(s.marker.submitted(), Some("echo !!"));
        // transient-prompt-execute-insertion-pollutes-history: an insertion that executes (the ↪ row) records the line
        // it builds, not the transient prompt (p10k, oh-my-posh) redrawn without markers in front of it.
        let (mut s, _) = session("{}");
        let (tx, rx) = mpsc::channel();
        s.engine = Rc::new(RecordingCompletionEngine::new(tx));
        prompt(&mut s, "git ch");
        reply(&mut s, subs("ch"), None);
        s.marker.set_submitted(Some("stale".into()));
        let fx = s.menu.handle(KeyAction::InsertSelectedAndExecute, &s.settings);
        let Some(MenuEffect::Insert { ins, .. }) = fx.into_iter().next() else {
            panic!("no insertion")
        };
        s.insert(&ins, None, &mut Vec::new());
        assert_eq!(s.marker.submitted(), Some("git cherry-pick"));
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
        s.marker.set_pwsh_list_view(true);
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
        s.marker.set_pwsh_list_view(true);
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
            fn try_clone_reader(&self) -> anyhow::Result<Box<dyn std::io::Read + Send>> {
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
        assert!(s.step(SessionMessage::ChildExit(0), &mut None));
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
        inbox: Sender<SessionMessage>,
        credit_returns: Receiver<()>,
        stash: Option<SessionMessage>,
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
            self.s.lock.as_mut().unwrap().expire_deadline_at(Instant::now());
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
            self.s.echo.expire_hold_at(Instant::now());
        }

        fn poke_spinner_due(&mut self) {
            self.s.spinner_at = Some(Instant::now());
        }

        fn poke_probe_window(&mut self) {
            self.s.probe_until = Some(Instant::now() + LATE_PROBE);
        }

        fn step_pty(&mut self, bytes: &[u8]) -> bool {
            self.s.step(SessionMessage::Pty(bytes.to_vec()), &mut self.stash)
        }

        fn step_stdin(&mut self, bytes: &[u8]) -> bool {
            self.s.step(SessionMessage::Stdin(bytes.to_vec()), &mut self.stash)
        }

        fn step_resize(&mut self, columns: u16, rows: u16) -> bool {
            self.s.step(SessionMessage::Resize(columns, rows), &mut self.stash)
        }

        fn step_completion(&mut self, result: CompletionResult) -> bool {
            self.s.step(SessionMessage::Completion(Box::new(result)), &mut self.stash)
        }

        fn step_ghost(&mut self, request: u64, text: Option<String>) -> bool {
            self.s.step(SessionMessage::Ghost { req: request, text }, &mut self.stash)
        }

        fn step_child_exit(&mut self, code: i32) -> bool {
            self.s.step(SessionMessage::ChildExit(code), &mut self.stash)
        }

        fn step_pty_eof(&mut self) -> bool {
            self.s.step(SessionMessage::PtyEof, &mut self.stash)
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
            self.inbox.send(SessionMessage::Pty(bytes.to_vec())).unwrap();
        }

        fn queue_stdin(&mut self, bytes: &[u8]) {
            self.inbox.send(SessionMessage::Stdin(bytes.to_vec())).unwrap();
        }

        fn credits_returned(&self) -> usize {
            self.credit_returns.try_iter().count()
        }

        fn stash_is_stdin(&self) -> bool {
            matches!(self.stash, Some(SessionMessage::Stdin(_)))
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
            self.rig.inbox.send(SessionMessage::Resize(columns, rows)).unwrap();
        }

        fn queue_completion(&mut self, result: CompletionResult) {
            self.rig.inbox.send(SessionMessage::Completion(Box::new(result))).unwrap();
        }

        fn queue_ghost(&mut self, request: u64, text: Option<String>) {
            self.rig.inbox.send(SessionMessage::Ghost { req: request, text }).unwrap();
        }

        fn queue_child_exit(&mut self, code: i32) {
            self.rig.inbox.send(SessionMessage::ChildExit(code)).unwrap();
        }

        fn queue_pty_eof(&mut self) {
            self.rig.inbox.send(SessionMessage::PtyEof).unwrap();
        }

        fn queue_term(&mut self) {
            self.rig.inbox.send(SessionMessage::Term).unwrap();
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
            self.rig.s.screen.state().size()
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
