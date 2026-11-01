//! `tabsmith _ pty`: the PTY wrapper session (SPEC §4.3–4.5, §7.2.4, §7.3, §7.6, §8).
//! OWNER: wrapper agent.
use crate::shared::infrastructure::logger;
use crate::shared::application::ports::clock::Clock;
use crate::terminal_session::application::ports::terminal_screen::TerminalScreen;
use crate::terminal_session::application::ports::terminal_writer::TerminalWriter;
use crate::terminal_session::application::ports::wrapped_shell::WrappedShell;
use crate::terminal_session::application::session_use_cases::SessionUseCases;
use crate::terminal_session::application::shell_end::ShellEnd;
use crate::terminal_session::domain::entities::terminal_session::TerminalSession;
use crate::terminal_session::infrastructure::adapters::pty_wrapped_shell::PtyWrappedShell;
use crate::terminal_session::domain::value_objects::terminal_background::TerminalBackground;
use crate::terminal_session::infrastructure::pty::terminal_probe::parse_probe;
use crate::terminal_session::infrastructure::pty::session_message::SessionMessage;
use std::panic::AssertUnwindSafe;
use std::rc::Rc;
use std::sync::mpsc::{Receiver, RecvTimeoutError, SyncSender};
use std::sync::Arc;
use std::time::Instant;

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
    pub(crate) fn new(parts: SessionRunnerParts) -> Self {
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
mod tests;
