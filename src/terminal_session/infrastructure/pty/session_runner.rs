//! `tabsmith _ pty`: the PTY wrapper session (SPEC §4.3–4.5, §7.2.4, §7.3, §7.6, §8).
//! OWNER: wrapper agent.
use crate::shared::infrastructure::logger;
use crate::shared::application::ports::clock::Clock;
use crate::shared::infrastructure::system_clock::SystemClock;
use crate::dropdown::domain::services::theme_resolver::resolve_theme;
use crate::terminal_session::application::ports::terminal_modes::TerminalModes;
use crate::terminal_session::application::ports::terminal_screen::TerminalScreen;
use crate::terminal_session::application::ports::terminal_writer::TerminalWriter;
use crate::terminal_session::application::ports::wrapped_shell::WrappedShell;
use crate::terminal_session::application::session_ports_parts::SessionPortsParts;
use crate::terminal_session::application::session_use_cases::SessionUseCases;
use crate::terminal_session::application::shell_end::ShellEnd;
use crate::terminal_session::domain::entities::marker_tracker::MarkerTracker;
use crate::terminal_session::domain::entities::terminal_session::{TerminalSession, TerminalSessionParts};
use crate::terminal_session::infrastructure::adapters::alacritty_terminal_screen::AlacrittyTerminalScreen;
use crate::terminal_session::infrastructure::adapters::console_terminal_modes::ConsoleTerminalModes;
use crate::terminal_session::infrastructure::adapters::overlay_renderer::OverlayRenderer;
use crate::terminal_session::infrastructure::adapters::pty_wrapped_shell::PtyWrappedShell;
use crate::terminal_session::infrastructure::adapters::stdout_terminal_writer::StdoutTerminalWriter;
use crate::shared::infrastructure::data_directory_from_env::user_home_from_env;
use crate::shared::domain::value_objects::shell::Shell;
use crate::terminal_session::domain::value_objects::session_id::SessionId;
use crate::terminal_session::domain::value_objects::terminal_background::TerminalBackground;
use crate::terminal_session::infrastructure::adapters::kitty_graphics;
use crate::terminal_session::domain::value_objects::wrapped_shell_command::WrappedShellCommand;
use crate::terminal_session::infrastructure::terminal_session_module::TerminalSessionModule;
use crate::terminal_session::infrastructure::pty::pty_reader::spawn_pty_reader;
use crate::terminal_session::infrastructure::pty::terminal_probe::{parse_probe, probe_terminal, PROBE_TIMEOUT};
use crate::terminal_session::infrastructure::pty::session_message::SessionMessage;
use anyhow::{bail, Context, Result};
use portable_pty::{CommandBuilder, PtySize};
use std::io::{IsTerminal, Write};
use std::panic::AssertUnwindSafe;
use std::rc::Rc;
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, SyncSender};
use std::sync::Arc;
use std::thread;
use std::time::{Instant, SystemTime, UNIX_EPOCH};
#[cfg(unix)]
use crate::terminal_session::infrastructure::pty::platform_unix as sys;
#[cfg(windows)]
use crate::terminal_session::infrastructure::pty::platform_windows as sys;

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

/// What `SessionRunner::new` takes: the fields of the runner, declared in the order the runner drops them.
pub(crate) struct SessionRunnerParts {
    pub(crate) inbox: Receiver<SessionMessage>,
    /// Returns PTY read credits to the reader thread (see `pump`). Declared before `pty`: it is dropped first, so a
    /// ConPTY close never waits for a reader that waits for a credit.
    pub(crate) credits: SyncSender<()>,
    /// The shell the session wraps: its input and its size.
    pub(crate) pty: Rc<PtyWrappedShell>,
    /// What the wrapper knows of the shell it runs; the ports and plumbing around it stay here.
    pub(crate) session: TerminalSession,
    /// The model, the filter and the renderer of the session, behind the screen port.
    pub(crate) screen: Rc<dyn TerminalScreen>,
    /// The user's terminal.
    pub(crate) output: Rc<dyn TerminalWriter>,
    /// What the wrapper does with the ports it was built over.
    pub(crate) use_cases: SessionUseCases,
    pub(crate) clock: Arc<dyn Clock + Send + Sync>,
}

/// The driver of one session: it reads its inbox, hands each message to the use case that handles it and leaves when the
/// shell is gone. The fields keep the drop order of the legacy struct (the inbox, the credits, the shell, then the rest).
pub(crate) struct SessionRunner {
    inbox: Receiver<SessionMessage>,
    /// Returns PTY read credits to the reader thread (see `pump`). Declared before `pty`: it is dropped first, so a
    /// ConPTY close never waits for a reader that waits for a credit.
    credits: SyncSender<()>,
    /// The shell the session wraps: its input and its size.
    pty: Rc<PtyWrappedShell>,
    /// What the wrapper knows of the shell it runs; the ports and plumbing around it stay here.
    session: TerminalSession,
    /// The model, the filter and the renderer of the session, behind the screen port.
    screen: Rc<dyn TerminalScreen>,
    /// The user's terminal.
    output: Rc<dyn TerminalWriter>,
    /// What the wrapper does with the ports above.
    use_cases: SessionUseCases,
    clock: Arc<dyn Clock + Send + Sync>,
}

impl SessionRunner {
    fn new(parts: SessionRunnerParts) -> Self {
        SessionRunner {
            inbox: parts.inbox,
            credits: parts.credits,
            pty: parts.pty,
            session: parts.session,
            screen: parts.screen,
            output: parts.output,
            use_cases: parts.use_cases,
            clock: parts.clock,
        }
    }

    /// SPEC §4.4 steps 1–9. `modes` receives the saved terminal modes as soon as they change.
    pub(crate) fn start(command: &WrappedShellCommand, modes: &mut Option<Rc<dyn TerminalModes>>, module: &TerminalSessionModule) -> Result<SessionRunner> {
        if !std::io::stdin().is_terminal() || !std::io::stdout().is_terminal() {
            bail!("stdin/stdout is not a terminal");
        }
        let program = command
            .program()
            .context("no shell to run (set SHELL or pass --shell)")?;
        let output: Rc<dyn TerminalWriter> = Rc::new(StdoutTerminalWriter);
        let live_settings = module.live_settings();
        let settings = live_settings.load();
        let (saved_modes, lf_to_ind) = sys::enter()?;
        let terminal_modes: Rc<dyn TerminalModes> = Rc::new(ConsoleTerminalModes::new(saved_modes));
        *modes = Some(terminal_modes.clone());
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
        let (cpr, terminal_bg, typeahead) = probe_terminal(&*output, PROBE_TIMEOUT, sys::read_timeout);
        // ponytail: a reply cut by the timeout is forwarded as typeahead; only a whole late reply is dropped.
        let probe_ended_at = cpr.is_none().then(Instant::now);
        if cpr.is_none() {
            output.write(b"\x1b[H\x1b[2J");
        }

        // Everything that can fail goes before the child exists, so the fallback never orphans a shell.
        let truecolor = matches!(
            std::env::var("COLORTERM").as_deref(),
            Ok("truecolor" | "24bit")
        ) || std::env::var_os("WT_SESSION").is_some();
        let themes = module.themes();
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
        let clock: Arc<dyn Clock + Send + Sync> = Arc::new(SystemClock);
        let (tx, rx) = mpsc::channel();
        sys::spawn_resize(tx.clone())?;
        let engine = module.start_completion_engine()(tx.clone(), settings.clone());

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

        output.write(&command.window_title());

        // Threads (SPEC §4.3).
        let credits = spawn_pty_reader(reader, tx.clone());
        let (input, wrx) = mpsc::channel::<Vec<u8>>();
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
        let pty = Rc::new(PtyWrappedShell::new(input, pair.master));
        if !typeahead.is_empty() {
            pty.write_input(typeahead);
        }

        let session_contexts = module.session_contexts();
        let mut session = TerminalSession::new(TerminalSessionParts {
            id,
            launched_shell: shell,
            settings,
            tracker: marker,
            terminal_background: terminal_bg.map(TerminalBackground::new),
            settings_modified: live_settings.last_modified(),
            now: Instant::now(),
        });
        if let Some(probe_ended_at) = probe_ended_at {
            session.expect_late_probe_reply(probe_ended_at);
        }
        let working_directory = module.working_directory();
        let generator_processes = module.generator_processes();
        let use_cases = SessionUseCases::new(SessionPortsParts {
            engine: engine.clone(),
            screen: screen.clone(),
            writer: output.clone(),
            shell: pty.clone(),
            reaper: generator_processes.clone(),
            modes: terminal_modes.clone(),
            live_settings: live_settings.clone(),
            session_contexts: session_contexts.clone(),
            themes: themes.clone(),
            working_directory: working_directory.clone(),
            clock: clock.clone(),
        });
        Ok(SessionRunner::new(SessionRunnerParts {
            inbox: rx,
            credits,
            pty,
            session,
            screen,
            output,
            use_cases,
            clock,
        }))
    }

    /// Main loop (SPEC §4.3); returns the child's exit code.
    pub(crate) fn run(mut self) -> i32 {
        let mut stash = None;
        loop {
            let msg = match stash.take() {
                Some(m) => Ok(m),
                None => match self.next_deadline() {
                    Some(d) => self
                        .inbox
                        .recv_timeout(d.saturating_duration_since(self.clock.now_instant())),
                    None => self.inbox.recv().map_err(|_| RecvTimeoutError::Disconnected),
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
            if self.session.exit_grace_over(self.clock.now_instant()) {
                break;
            }
            if !self.session.is_passthrough() {
                self.guard(|s| s.use_cases.handle_timer_tick.execute(&mut s.session));
            }
        }
        self.use_cases.end_session.execute(&self.session);
        self.session.exit_code()
    }

    /// One main-loop message; false = leave the loop.
    fn step(&mut self, msg: SessionMessage, stash: &mut Option<SessionMessage>) -> bool {
        match msg {
            SessionMessage::Stdin(b) if self.session.is_passthrough() => {
                self.pty.write_input(b);
            }
            SessionMessage::Stdin(b) => {
                // on_input writes nothing before its last step: after a panic, keys the menu would not have taken go
                // out raw (an Enter meant for an item is dropped rather than running the line).
                let raw = !self.session.menu_intercepts();
                if !self.guard(|s| s.on_input(&b)) && raw {
                    self.pty.write_input(b);
                }
            }
            SessionMessage::Pty(mut b) => {
                // One batch = everything already queued (one model pass, one stdout write).
                let mut chunks = 1;
                loop {
                    match self.inbox.try_recv() {
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
                // Processing the output writes nothing before its last step: after a panic the batch goes out raw.
                if self.session.is_passthrough()
                    || !self.guard(|s| s.use_cases.process_shell_output.execute(&mut s.session, &b))
                {
                    self.emit(&b);
                }
                for _ in 0..chunks {
                    let _ = self.credits.try_send(());
                }
            }
            SessionMessage::PtyEof => {
                return self.use_cases.handle_shell_exit.execute(&mut self.session, ShellEnd::OutputClosed);
            }
            SessionMessage::Resize(cols, rows) if self.session.is_passthrough() => {
                self.pty.resize(cols, rows);
            }
            SessionMessage::Resize(c, r) => {
                self.guard(|s| s.use_cases.resize_terminal.execute(&mut s.session, c, r));
            }
            SessionMessage::ChildExit(c) => {
                self.pty.release_pseudo_console();
                if !self.use_cases.handle_shell_exit.execute(&mut self.session, ShellEnd::ChildExited(c)) {
                    return false;
                }
            }
            SessionMessage::Term => {
                return self.use_cases.handle_shell_exit.execute(&mut self.session, ShellEnd::Terminated);
            }
            SessionMessage::Completion(_) | SessionMessage::Ghost { .. } if self.session.is_passthrough() => {}
            SessionMessage::Completion(c) => {
                self.guard(|s| s.use_cases.apply_completion_result.execute(&mut s.session, *c));
            }
            SessionMessage::Ghost { req, text } => {
                self.guard(|s| s.use_cases.apply_llm_ghost_text.execute(&mut s.session, req, text));
            }
        }
        true
    }

    /// Run one handler. A panic (logged by the hook) clears the overlay and turns the session into a plain
    /// pass-through until the child exits: the running shell and its jobs are never killed for it.
    /// ponytail: a panic in the output processing's final frame loses the clear it already took (overlay left until redrawn).
    fn guard(&mut self, f: impl FnOnce(&mut SessionRunner)) -> bool {
        if std::panic::catch_unwind(AssertUnwindSafe(|| f(self))).is_ok() {
            return true;
        }
        logger::error("pty: internal error; passing the session through until the shell exits");
        self.session.enter_passthrough();
        let clear = self.screen.take_clear();
        self.emit(&clear);
        false
    }

    fn emit(&self, b: &[u8]) {
        self.output.write(b);
    }

    fn next_deadline(&self) -> Option<Instant> {
        self.session.next_deadline()
    }

    /// Stdin chunk (SPEC §7.2.4): intercept bound keys while the menu is shown, accept ghost text, else forward.
    fn on_input(&mut self, chunk: &[u8]) {
        // A probe reply arriving after the startup timeout is dropped, not typed into the shell.
        let late;
        let chunk = if self.session.late_probe_open(self.clock.now_instant()) {
            let (cpr, bg, rest) = parse_probe(chunk);
            if let Some(bg) = bg {
                self.use_cases
                    .apply_terminal_background
                    .execute(&mut self.session, TerminalBackground::new(bg));
            }
            if cpr.is_some() {
                self.session.probe_cursor_reported();
            }
            late = rest;
            &late[..]
        } else {
            chunk
        };
        self.use_cases.handle_keystrokes.execute(&mut self.session, chunk);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc::Sender;
    use std::time::Duration;
    use crate::dropdown::domain::repositories::theme_repository::ThemeRepository;
    use crate::terminal_session::application::ports::working_directory::WorkingDirectory;
    use crate::terminal_session::application::ports::completion_engine::CompletionEngine;
    use crate::completion::domain::value_objects::completion_result::CompletionResult;
    use crate::terminal_session::application::ports::generator_process_reaper::GeneratorProcessReaper;
    use crate::terminal_session::domain::repositories::session_context_repository::SessionContextRepository;
    use crate::terminal_session::application::ports::live_settings::LiveSettings;

    // --- Session driven without a terminal: PTY output via `on_output`, keys via `on_input`. -------------------

    use crate::terminal_session::infrastructure::pty::pty_reader::PTY_CREDITS;
    use crate::completion::domain::test_fixtures::{comp, item, settings};
    use crate::dropdown::domain::repositories::theme_repository::InMemoryThemeRepository;
    use crate::terminal_session::application::ports::live_settings::InMemoryLiveSettings;
    use crate::terminal_session::application::ports::terminal_modes::RecordingTerminalModes;
    use crate::terminal_session::application::ports::working_directory::FixedWorkingDirectory;
    use crate::terminal_session::infrastructure::adapters::lenient_live_settings::LenientLiveSettings;
    use crate::shared::domain::value_objects::data_directory::DataDirectory;
    use crate::shared::infrastructure::testing::temp_dir::TempDir;
    use crate::terminal_session::domain::repositories::session_context_repository::InMemorySessionContextRepository;
    use crate::settings::domain::value_objects::settings::Settings;
    use crate::terminal_session::application::ports::completion_engine::{
        CompletionEngineCall, PanickingCompletionEngine, RecordingCompletionEngine,
    };
    use crate::terminal_session::application::ports::generator_process_reaper::RecordingGeneratorProcessReaper;
    use crate::terminal_session::application::ports::terminal_writer::CapturingTerminalWriter;
    use crate::terminal_session::application::test_fixtures::set_panic_in_key_handling;
    use crate::terminal_session::infrastructure::adapters::pty_wrapped_shell::RecordingMasterPty;
    use crate::settings::infrastructure::adapters::json_file_settings_repository::JsonFileSettingsRepository;
    use crate::completion::domain::value_objects::suggestion::Suggestion;
    use crate::specs::domain::value_objects::suggestion_kind::SuggestionKind;

    const SID: &str = "0123456789abcdef0123456789abcdef";

    /// What a session sent out: the bytes it wrote to the shell (read from the writer thread's channel) and to the
    /// user's terminal.
    struct Taps {
        shell_input: Receiver<Vec<u8>>,
        stdout: Rc<CapturingTerminalWriter>,
    }

    /// The ports a test swaps after the session is built; `rewire_use_cases` builds the use cases over them again.
    struct Ports {
        engine: Rc<dyn CompletionEngine>,
        modes: Rc<dyn TerminalModes>,
        themes: Rc<dyn ThemeRepository>,
        live_settings: Rc<dyn LiveSettings>,
        working_directory: Rc<dyn WorkingDirectory>,
        session_contexts: Rc<dyn SessionContextRepository>,
        generator_processes: Rc<dyn GeneratorProcessReaper>,
    }

    fn session(json: &str) -> (SessionRunner, Taps) {
        let (s, taps, _) = session_named(json, SID);
        (s, taps)
    }

    /// A session whose id is `id` (the shell integration markers keep the id `SID`).
    fn session_named(json: &str, id: &str) -> (SessionRunner, Taps, Ports) {
        let settings = settings(json);
        let (input, shell_input) = mpsc::channel();
        let stdout = Rc::new(CapturingTerminalWriter::new());
        let marker = MarkerTracker::new(
            SessionId::new(SID.to_string()).unwrap(),
            Some(Shell::Bash),
            false,
        );
        let screen: Rc<dyn TerminalScreen> = Rc::new(AlacrittyTerminalScreen::for_tests(80, 24));
        let engine: Rc<dyn CompletionEngine> = Rc::new(RecordingCompletionEngine::new(mpsc::channel().0));
        let pty = Rc::new(PtyWrappedShell::new(input, Box::new(RecordingMasterPty::new())));
        let modes: Rc<dyn TerminalModes> = Rc::new(RecordingTerminalModes::new());
        let themes: Rc<dyn ThemeRepository> = Rc::new(InMemoryThemeRepository::default());
        let live_settings: Rc<dyn LiveSettings> = Rc::new(InMemoryLiveSettings::new(Settings::default()));
        let working_directory: Rc<dyn WorkingDirectory> =
            Rc::new(FixedWorkingDirectory::new(std::env::current_dir().ok()));
        let session_contexts: Rc<dyn SessionContextRepository> =
            Rc::new(InMemorySessionContextRepository::default());
        let generator_processes: Rc<dyn GeneratorProcessReaper> = Rc::new(RecordingGeneratorProcessReaper::new());
        let clock: Arc<dyn Clock + Send + Sync> = Arc::new(SystemClock);
        let use_cases = SessionUseCases::new(SessionPortsParts {
            engine: engine.clone(),
            screen: screen.clone(),
            writer: stdout.clone(),
            shell: pty.clone(),
            reaper: generator_processes.clone(),
            modes: modes.clone(),
            live_settings: live_settings.clone(),
            session_contexts: session_contexts.clone(),
            themes: themes.clone(),
            working_directory: working_directory.clone(),
            clock: clock.clone(),
        });
        let s = SessionRunner::new(SessionRunnerParts {
            inbox: mpsc::channel().1,
            credits: mpsc::sync_channel(PTY_CREDITS).0,
            pty,
            session: TerminalSession::new(TerminalSessionParts {
                id: SessionId::new(id.to_string()).unwrap(),
                launched_shell: Shell::Bash,
                settings,
                tracker: marker,
                terminal_background: None,
                settings_modified: None,
                now: Instant::now(),
            }),
            screen,
            output: stdout.clone(),
            use_cases,
            clock,
        });
        let ports = Ports { engine, modes, themes, live_settings, working_directory, session_contexts, generator_processes };
        (s, Taps { shell_input, stdout }, ports)
    }

    /// The use cases hold the ports they were built over: a test that swaps a port of the session builds them again.
    fn rewire_use_cases(s: &mut SessionRunner, ports: &Ports) {
        s.use_cases = SessionUseCases::new(SessionPortsParts {
            engine: ports.engine.clone(),
            screen: s.screen.clone(),
            writer: s.output.clone(),
            shell: s.pty.clone(),
            reaper: ports.generator_processes.clone(),
            modes: ports.modes.clone(),
            live_settings: ports.live_settings.clone(),
            session_contexts: ports.session_contexts.clone(),
            themes: ports.themes.clone(),
            working_directory: ports.working_directory.clone(),
            clock: Arc::new(SystemClock),
        });
    }

    /// A bash prompt `$ ` with the integration markers, then `text` echoed.
    fn prompt(s: &mut SessionRunner, text: &str) {
        let p = format!(
            "\x1b]697;StartPrompt\x07$ \x1b]697;EndPrompt\x07\x1b]697;NewCmd={SID}\x07{text}"
        );
        s.use_cases.process_shell_output.execute(&mut s.session, p.as_bytes());
    }

    /// Engine reply for the latest request (twice: the first one after a reset only arms the menu).
    fn reply(s: &mut SessionRunner, items: Vec<Suggestion>, ghost: Option<&str>) {
        let (buffer, _) = s.session.sent().expect("a request was sent");
        let mut c = comp(buffer, 1, items);
        c = c.with_request_id(s.session.request_id()).with_ghost(ghost.map(String::from));
        s.use_cases.apply_completion_result.execute(&mut s.session, c.clone());
        s.use_cases.apply_completion_result.execute(&mut s.session, c);
    }

    fn subs(q: &str) -> Vec<Suggestion> {
        ["cherry-pick", "checkout"]
            .iter()
            .map(|n| item(n, SuggestionKind::Subcommand, q))
            .collect()
    }

    fn sent(pty: &Taps) -> Vec<u8> {
        pty.shell_input.try_iter().flatten().collect()
    }

    #[test]
    fn late_probe_reply_is_not_typed() {
        let (mut s, pty) = session("{}");
        s.session.expect_late_probe_reply(Instant::now());
        prompt(&mut s, "");
        s.on_input(b"\x1b]11;rgb:ffff/ffff/ffff\x1b\\\x1b[5;1Rx");
        assert_eq!(sent(&pty), b"x");
        assert_eq!(
            (s.session.terminal_background().map(TerminalBackground::rgb), s.session.late_probe_until()),
            (Some((255, 255, 255)), None)
        );
    }

    #[test]
    fn panic_turns_the_session_into_a_pass_through() {
        let (mut s, pty) = session("{}");
        prompt(&mut s, "git ch");
        reply(&mut s, subs("ch"), None);
        pty.stdout.clear();
        assert!(!s.guard(|_| panic!("injected")));
        assert!(
            s.session.is_passthrough() && pty.stdout.written().starts_with(b"\x1b[?2026h"),
            "overlay cleared"
        );
        pty.stdout.clear();
        let mut stash = None;
        assert!(s.step(SessionMessage::Pty(b"out\x1b]697;NewCmd\x07".to_vec()), &mut stash));
        assert_eq!(pty.stdout.written(), b"out\x1b]697;NewCmd\x07");
        assert!(s.step(SessionMessage::Stdin(b"\r".to_vec()), &mut stash));
        assert_eq!(sent(&pty), b"\r", "keys reach the shell unintercepted");
        assert!(s.step(SessionMessage::ChildExit(3), &mut stash));
        assert!(!s.step(SessionMessage::PtyEof, &mut stash));
        assert_eq!((s.session.reported_exit_code(), s.next_deadline().is_some()), (Some(3), true));
    }

    #[test]
    fn panic_while_handling_keys_still_forwards_them() {
        let (mut s, pty) = session("{}");
        prompt(&mut s, "ls");
        set_panic_in_key_handling(true);
        assert!(s.step(SessionMessage::Stdin(b"BOOM\r".to_vec()), &mut None));
        set_panic_in_key_handling(false);
        assert!(s.session.is_passthrough());
        assert_eq!(sent(&pty), b"BOOM\r", "the read is not lost");
        // With the menu intercepting it is dropped: an Enter meant for an item never runs the line.
        let (mut s, pty) = session("{}");
        prompt(&mut s, "git ch");
        reply(&mut s, subs("ch"), None);
        set_panic_in_key_handling(true);
        s.step(SessionMessage::Stdin(b"\r".to_vec()), &mut None);
        set_panic_in_key_handling(false);
        assert!(s.session.is_passthrough() && sent(&pty).is_empty());
    }

    #[cfg(windows)]
    #[test]
    fn conpty_is_closed_off_the_main_loop() {
        use portable_pty::MasterPty;
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
        s.pty = Rc::new(PtyWrappedShell::new(mpsc::channel().0, Box::new(Master(tx))));
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
    /// that drive `SessionRunner::step`.
    struct Rig {
        s: SessionRunner,
        ports: Ports,
        pty: Taps,
        engine: Receiver<CompletionEngineCall>,
        log: String,
        mark: usize,
        inbox: Sender<SessionMessage>,
        credit_returns: Receiver<()>,
        stash: Option<SessionMessage>,
    }

    impl Rig {
        fn new(settings_json: &str) -> Rig {
            let (mut s, pty, mut ports) = session_named(settings_json, SID);
            let (calls, engine) = mpsc::channel();
            ports.engine = Rc::new(RecordingCompletionEngine::new(calls));
            rewire_use_cases(&mut s, &ports);
            let (inbox, rx) = mpsc::channel();
            let (credits, credit_returns) = mpsc::sync_channel(PTY_CREDITS);
            s.inbox = rx;
            s.credits = credits;
            Rig { s, ports, pty, engine, log: String::new(), mark: 0, inbox, credit_returns, stash: None }
        }

        fn read_settings_from(&mut self, temp_dir: &TempDir) {
            self.ports.live_settings = Rc::new(LenientLiveSettings::new(Rc::new(JsonFileSettingsRepository::new(
                DataDirectory::new(Some(temp_dir.path().to_path_buf()), None),
            ))));
            self.rewire_use_cases();
        }

        fn rewire_use_cases(&mut self) {
            rewire_use_cases(&mut self.s, &self.ports);
        }

        fn sent(&self) -> Vec<u8> {
            sent(&self.pty)
        }

        fn note(&mut self, step: &str) {
            let written = self.pty.stdout.written();
            let out = written[self.mark..].to_vec();
            self.mark = written.len();
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
            let request = self.s.session.request_id();
            self.complete(CompletionResult::error(request, buffer.into(), cursor));
        }

        fn reply_loading(&mut self, buffer: &str, items: Vec<Suggestion>) {
            let loading = comp(buffer, 1, items).with_request_id(self.s.session.request_id()).with_loading(true);
            self.complete(loading);
        }

        fn request_id(&self) -> u64 {
            self.s.session.request_id()
        }

        fn input(&mut self, bytes: &[u8]) {
            self.s.on_input(bytes);
        }

        fn output(&mut self, bytes: &[u8]) {
            self.s.use_cases.process_shell_output.execute(&mut self.s.session, bytes);
        }

        fn complete(&mut self, result: CompletionResult) {
            self.s.use_cases.apply_completion_result.execute(&mut self.s.session, result);
        }

        fn tick(&mut self) {
            self.s.use_cases.handle_timer_tick.execute(&mut self.s.session);
        }

        fn redraw(&mut self) {
            self.s.use_cases.painter().redraw(&self.s.session);
        }

        fn resize(&mut self, columns: u16, rows: u16) {
            self.s.use_cases.resize_terminal.execute(&mut self.s.session, columns, rows);
        }

        fn end(&mut self) {
            self.s.use_cases.end_session.execute(&self.s.session);
        }

        fn poke_lock_due(&mut self) {
            self.s.session.expire_insertion_deadline(Instant::now());
        }

        fn poke_resize_due(&mut self) {
            self.s.session.expire_resize_settle_at(Instant::now());
        }

        fn poke_llm_due(&mut self) {
            self.s.session.schedule_llm_request_at(Instant::now());
        }

        fn poke_poll_due(&mut self) {
            self.s.session.expire_poll_at(Instant::now());
        }

        fn poke_hold_due(&mut self) {
            self.s.session.expire_hold_at(Instant::now());
        }

        fn poke_spinner_due(&mut self) {
            self.s.session.set_spinner_at(Some(Instant::now()));
        }

        fn poke_probe_window(&mut self) {
            self.s.session.expect_late_probe_reply(Instant::now());
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
            let message = self.s.inbox.try_recv().expect("a queued message");
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
            self.s.session.is_passthrough()
        }

        fn exit_code(&self) -> Option<i32> {
            self.s.session.reported_exit_code()
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

    // --- Phase 9 loop guards: how `SessionRunner::step` and `run` batch, hand back credits and decide to leave. ---------
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
            set_panic_in_key_handling(false);
        }
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
                output_ended: self.rig.s.session.has_seen_pty_eof(),
                grace_armed: self.rig.s.session.exit_deadline().is_some(),
            }
        }

        /// The exit deadline lies exactly `delay` after the last step began.
        fn exit_deadline_is_set_after(&self, delay: Duration) -> bool {
            self.rig
                .s
                .session
                .exit_deadline()
                .is_some_and(|at| at >= self.stepped_before + delay && at <= self.stepped_after + delay)
        }

        /// What reached stdout since the last call.
        fn stdout(&mut self) -> Vec<u8> {
            let all = self.rig.pty.stdout.written();
            let written = all[self.stdout_mark..].to_vec();
            self.stdout_mark = all.len();
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
            self.rig.ports.engine = Rc::new(PanickingCompletionEngine);
            self.rig.rewire_use_cases();
        }

        fn panic_in_key_handling(&mut self) {
            set_panic_in_key_handling(true);
            self.key_handling_panic = Some(KeyHandlingPanic);
        }

        /// A real pseudo-terminal of 80x24 stands in for the child's.
        fn open_a_pty(&mut self) {
            let pair = portable_pty::native_pty_system()
                .openpty(PtySize { rows: 24, cols: 80, pixel_width: 0, pixel_height: 0 })
                .unwrap();
            let (input, shell_input) = mpsc::channel();
            self.rig.s.pty = Rc::new(PtyWrappedShell::new(input, pair.master));
            self.rig.rewire_use_cases();
            self.rig.pty.shell_input = shell_input;
            self.pty_slave = Some(pair.slave);
        }

        fn pty_size(&self) -> PtySize {
            self.rig.s.pty.size().unwrap()
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
