use crate::modules::completion::domain::value_objects::insertion::Insertion;
use crate::modules::dropdown::domain::value_objects::menu_effect::MenuEffect;
use crate::modules::shared::application::ports::clock::Clock;
use crate::modules::terminal_session::application::completion_requester::CompletionRequester;
use crate::modules::terminal_session::application::overlay_painter::OverlayPainter;
use crate::modules::terminal_session::application::ports::completion_engine::CompletionEngine;
use crate::modules::terminal_session::application::ports::terminal_screen::TerminalScreen;
use crate::modules::terminal_session::application::ports::terminal_writer::TerminalWriter;
use crate::modules::terminal_session::application::ports::wrapped_shell::WrappedShell;
use crate::modules::terminal_session::domain::entities::terminal_session::TerminalSession;
use crate::modules::terminal_session::domain::services::key_router::plain_char;
use crate::modules::terminal_session::domain::value_objects::key_route::KeyRoute;
use std::rc::Rc;
use std::sync::Arc;

/// Handles what the user types (SPEC §7.2.4): intercepts the keys bound to the menu while it is shown, accepts ghost
/// text, holds a key that would act while an unmodelled key's echo is pending, and forwards everything else to the
/// shell. Several events replay held keys through it, so it is a service, not a use case.
pub(crate) struct KeystrokeHandler {
    engine: Rc<dyn CompletionEngine>,
    screen: Rc<dyn TerminalScreen>,
    shell: Rc<dyn WrappedShell>,
    writer: Rc<dyn TerminalWriter>,
    clock: Arc<dyn Clock + Send + Sync>,
    requester: Rc<CompletionRequester>,
    painter: Rc<OverlayPainter>,
}

impl KeystrokeHandler {
    pub(crate) fn new(
        engine: Rc<dyn CompletionEngine>,
        screen: Rc<dyn TerminalScreen>,
        shell: Rc<dyn WrappedShell>,
        writer: Rc<dyn TerminalWriter>,
        clock: Arc<dyn Clock + Send + Sync>,
        requester: Rc<CompletionRequester>,
        painter: Rc<OverlayPainter>,
    ) -> Self {
        KeystrokeHandler { engine, screen, shell, writer, clock, requester, painter }
    }

    /// A stdin read: the keys of `chunk` one by one. The write to the shell is the last statement, so a panic above it
    /// leaves the read to be forwarded raw by the caller.
    pub(crate) fn handle(&self, session: &mut TerminalSession, chunk: &[u8]) {
        if session.is_holding_keys() {
            session.hold_behind(chunk); // keys keep their order behind held ones
            return;
        }
        let mut fwd = Vec::new();
        let (mut redraw, mut rerequest) = (false, false);
        let mut typed = 0usize;
        for ev in session.decode_keys(chunk) {
            #[cfg(test)]
            crate::modules::terminal_session::application::test_fixtures::maybe_panic();
            let key = ev.key();
            let effects = match session.route_key(&key, typed, &self.screen.state()) {
                KeyRoute::Hold => {
                    // The menu / ghost is for a buffer the shell may no longer have: wait for the echo and its reply.
                    // ponytail: one hold buffer; separate reads replayed together count as one for the paste rule.
                    session.hold_keys(&chunk[ev.bytes().start..], self.clock.now_instant());
                    break;
                }
                KeyRoute::MenuAction(a) => session.run_menu_action(a),
                KeyRoute::Swallow => Vec::new(),
                KeyRoute::ShowOnTab => session.show_menu_on_tab(),
                KeyRoute::AcceptGhost(g) => {
                    fwd.extend_from_slice(g.as_bytes());
                    session.accept_ghost(&g);
                    redraw = true;
                    continue;
                }
                KeyRoute::Forward => {
                    fwd.extend_from_slice(&chunk[ev.bytes()]);
                    typed += usize::from(plain_char(&key).is_some());
                    if session.forward_key(&key) {
                        redraw = true;
                    }
                    continue;
                }
            };
            redraw = true;
            for e in effects {
                match e {
                    MenuEffect::Insert { ins, record, .. } => self.insert(session, &ins, record, &mut fwd),
                    MenuEffect::Write(b) => {
                        fwd.extend_from_slice(&b);
                        session.wrote_to_shell(&b);
                    }
                    MenuEffect::Bell => self.writer.write(b"\x07"),
                    MenuEffect::Rerequest => rerequest = true,
                    MenuEffect::ToggleFuzzy => {
                        self.engine.toggle_fuzzy_search();
                        rerequest = true;
                    }
                }
            }
        }
        if rerequest {
            session.forget_sent();
            self.requester.refresh(session);
        }
        if redraw {
            self.painter.redraw(session);
        }
        if !fwd.is_empty() {
            self.shell.write_input(fwd); // last: after a panic above, `step` forwards the read raw
        }
    }

    /// Insertion with reconciliation + lock (SPEC §7.3). Dropped while a command runs.
    pub(crate) fn insert(
        &self,
        session: &mut TerminalSession,
        ins: &Insertion,
        record: Option<(String, String)>,
        fwd: &mut Vec<u8>,
    ) {
        let Some(bytes) = session.insert(ins, self.clock.now_instant()) else {
            return;
        };
        fwd.extend(bytes);
        if let Some((cmd, name)) = record {
            self.engine.record_insertion(cmd, name);
        }
    }

    /// Replay held keys once the reply for the buffer after the unmodelled key came (or the hold timed out).
    pub(crate) fn release_held(&self, session: &mut TerminalSession) {
        let Some(held) = session.release_held_keys(self.clock.now_instant()) else {
            return;
        };
        self.handle(session, &held);
    }
}
