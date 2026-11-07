use crate::modules::shared::application::ports::clock::Clock;
use crate::modules::terminal_session::application::frame_input::FrameInput;
use crate::modules::terminal_session::application::ports::terminal_screen::TerminalScreen;
use crate::modules::terminal_session::application::ports::terminal_writer::TerminalWriter;
use crate::modules::terminal_session::domain::entities::terminal_session::TerminalSession;
use std::rc::Rc;
use std::sync::Arc;

/// Draws the overlay (menu and ghost text) of a session over its terminal: the frame for the state the session is
/// in, the redraw that writes it, and the repaint of the whole model once a resize settled.
pub(crate) struct OverlayPainter {
    screen: Rc<dyn TerminalScreen>,
    writer: Rc<dyn TerminalWriter>,
    clock: Arc<dyn Clock + Send + Sync>,
}

impl OverlayPainter {
    pub(crate) fn new(
        screen: Rc<dyn TerminalScreen>,
        writer: Rc<dyn TerminalWriter>,
        clock: Arc<dyn Clock + Send + Sync>,
    ) -> Self {
        OverlayPainter { screen, writer, clock }
    }

    /// The bytes that bring the overlay up to date: the clear of a changed frame and the frame, or nothing when it
    /// is the one already drawn. The renderer applies the screen guards itself, so the frame is asked to be
    /// suppressed only for the wrapper-level reasons (`suppressed`, not `drawable`). The clock is read last, when
    /// the frame is built.
    pub(crate) fn frame(&self, session: &TerminalSession) -> Vec<u8> {
        let state = self.screen.state();
        let ghost = session.ghost_text(false, &state);
        let input = FrameInput {
            settings: session.settings(),
            ghost: ghost.as_deref(),
            spinner: session.spinner_frame(),
            suppress: session.suppressed(&state),
            scroll_region: state.scroll_region(),
            now: self.clock.now_instant(),
        };
        self.screen.draw_overlay(session.menu(), &input)
    }

    /// State-only change: the renderer clears a changed frame in the same write and skips an identical one
    /// (SPEC §7.5.3); the writer writes nothing for an empty frame.
    pub(crate) fn redraw(&self, session: &TerminalSession) {
        self.writer.write(&self.frame(session));
    }

    /// Full repaint once the resize settled (postponed while on the alternate screen); empty when none is due.
    pub(crate) fn take_repaint(&self, session: &mut TerminalSession) -> Vec<u8> {
        if !session.take_repaint(&self.screen.state()) {
            return Vec::new();
        }
        self.screen.full_repaint()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::completion::domain::test_fixtures::{comp, item, settings};
    use crate::modules::shared::application::ports::clock::FixedClock;
    use crate::modules::shared::domain::value_objects::shell::Shell;
    use crate::modules::specs::domain::value_objects::suggestion_kind::SuggestionKind;
    use crate::modules::terminal_session::application::ports::terminal_writer::CapturingTerminalWriter;
    use crate::modules::terminal_session::application::shell_output_feeder::ShellOutputFeeder;
    use crate::modules::terminal_session::application::test_fixtures::{subs, TerminalSessionHarness, SID};
    use crate::modules::terminal_session::domain::entities::marker_tracker::MarkerTracker;
    use crate::modules::terminal_session::domain::entities::terminal_session::TerminalSessionParts;
    use crate::modules::terminal_session::domain::value_objects::session_id::SessionId;
    use crate::modules::terminal_session::infrastructure::adapters::alacritty_terminal_screen::AlacrittyTerminalScreen;
    use std::time::Duration;

    const SYNC_BEGIN: &[u8] = b"\x1b[?2026h";
    const SPINNER_FIRST_FRAME: &str = "⠋";

    /// A session at a bash prompt over the real screen, with the painter under test on a clock that stands still.
    struct Scene {
        session: TerminalSession,
        screen: Rc<AlacrittyTerminalScreen>,
        writer: Rc<CapturingTerminalWriter>,
        clock: Arc<FixedClock>,
        feeder: ShellOutputFeeder,
        painter: OverlayPainter,
    }

    impl Scene {
        fn new() -> Self {
            let clock = Arc::new(FixedClock::new(0));
            let screen = Rc::new(AlacrittyTerminalScreen::for_tests(80, 24));
            let writer = Rc::new(CapturingTerminalWriter::new());
            let session = TerminalSession::new(TerminalSessionParts {
                id: SessionId::new(SID.to_string()).unwrap(),
                launched_shell: Shell::Bash,
                settings: settings("{}"),
                tracker: MarkerTracker::new(SessionId::new(SID.to_string()).unwrap(), Some(Shell::Bash), false),
                terminal_background: None,
                settings_modified: None,
                now: clock.now_instant(),
            });
            let feeder = ShellOutputFeeder::new(screen.clone(), clock.clone());
            let painter = OverlayPainter::new(screen.clone(), writer.clone(), clock.clone());
            Scene { session, screen, writer, clock, feeder, painter }
        }

        /// The prompt `$ ` with the shell integration markers, then `text` echoed, as the shell prints it.
        fn type_line(&mut self, text: &str) {
            let now = self.clock.now_instant();
            let output = format!(
                "\x1b]697;StartPrompt\x07$ \x1b]697;EndPrompt\x07\x1b]697;NewCmd={SID}\x07{text}"
            );
            let mut body = Vec::new();
            let (splitter, tracker) = self.session.output_parts();
            let events = self.feeder.feed(splitter, tracker, output.as_bytes(), &mut body);
            for event in events {
                self.session.on_shell_event(&event, now);
            }
            let state = self.screen.state();
            let extracted = self
                .session
                .tracker()
                .edit_buffer_context()
                .and_then(|context| self.screen.edit_buffer(&context));
            self.session.refresh(&state, extracted, now);
        }

        /// The engine's reply for the latest request (twice: the first one after a reset only arms the menu).
        fn reply(&mut self, loading: bool) {
            let items = ["cherry-pick", "checkout"]
                .iter()
                .map(|name| item(name, SuggestionKind::Subcommand, "ch"))
                .collect();
            let reply = comp("git ch", 1, items).with_request_id(self.session.request_id()).with_loading(loading);
            let now = self.clock.now_instant();
            self.session.apply_completion(reply.clone(), now);
            self.session.apply_completion(reply, now);
        }

        fn with_menu(loading: bool) -> Self {
            let mut scene = Scene::new();
            scene.type_line("git ch");
            scene.reply(loading);
            scene
        }

        fn resized_and_settled(&mut self) {
            let now = self.clock.now_instant();
            self.session.resize_applied(now);
            self.session.expire_resize_settle_at(now);
            self.session.settle_resize_if_due(now);
        }
    }

    fn contains(haystack: &[u8], needle: &[u8]) -> bool {
        haystack.windows(needle.len()).any(|window| window == needle)
    }

    #[test]
    fn should_write_the_frame_the_screen_draws() {
        let scene = Scene::with_menu(false);

        scene.painter.redraw(&scene.session);

        let written = scene.writer.written();
        assert!(
            contains(&written, SYNC_BEGIN) && contains(&written, b"eckout"),
            "{:?}",
            String::from_utf8_lossy(&written)
        );
    }

    #[test]
    fn should_draw_the_frame_at_the_instant_the_clock_reads_when_it_is_built() {
        let scene = Scene::with_menu(true);
        scene.clock.advance(Duration::from_millis(300));

        let frame = scene.painter.frame(&scene.session);

        assert!(String::from_utf8_lossy(&frame).contains(SPINNER_FIRST_FRAME), "{frame:?}");
    }

    #[test]
    fn should_write_nothing_for_a_frame_identical_to_the_last_one() {
        let scene = Scene::with_menu(false);
        scene.painter.redraw(&scene.session);
        scene.writer.clear();

        scene.painter.redraw(&scene.session);

        assert!(scene.writer.written().is_empty());
    }

    #[test]
    fn should_repaint_the_model_once_after_a_resize_settled() {
        let mut scene = Scene::with_menu(false);
        scene.resized_and_settled();

        let first = scene.painter.take_repaint(&mut scene.session);
        let second = scene.painter.take_repaint(&mut scene.session);

        assert_eq!((first.is_empty(), second.is_empty()), (false, true));
    }

    #[test]
    fn should_not_repaint_on_the_alternate_screen() {
        let mut scene = Scene::with_menu(false);
        scene.resized_and_settled();
        scene.screen.pass_through(b"\x1b[?1049h", &mut Vec::new());

        let repaint = scene.painter.take_repaint(&mut scene.session);

        assert!(repaint.is_empty());
    }

    #[test]
    fn identical_redraw_writes_nothing() {
        let mut h = TerminalSessionHarness::new("{}");
        h.prompt("git ch");
        h.reply(subs("ch"), None);
        assert!(
            h.written().windows(8).any(|w| w == b"\x1b[?2026h"),
            "a frame was drawn"
        );
        h.clear_written();
        h.redraw();
        assert!(h.written().is_empty(), "{:?}", String::from_utf8_lossy(&h.written()));
    }
}
