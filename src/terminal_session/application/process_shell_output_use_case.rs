use crate::shared::application::ports::clock::Clock;
use crate::terminal_session::application::completion_requester::CompletionRequester;
use crate::terminal_session::application::keystroke_handler::KeystrokeHandler;
use crate::terminal_session::application::overlay_painter::OverlayPainter;
use crate::terminal_session::application::ports::completion_engine::CompletionEngine;
use crate::terminal_session::application::ports::terminal_screen::TerminalScreen;
use crate::terminal_session::application::ports::terminal_writer::TerminalWriter;
use crate::terminal_session::application::session_file_poller::SessionFilePoller;
use crate::terminal_session::application::shell_output_feeder::ShellOutputFeeder;
use crate::terminal_session::domain::entities::terminal_session::TerminalSession;
use crate::terminal_session::domain::value_objects::shell_event::ShellEvent;
use std::rc::Rc;
use std::sync::Arc;

/// The collaborators of `ProcessShellOutputUseCase`.
pub(crate) struct ProcessShellOutputParts {
    pub(crate) engine: Rc<dyn CompletionEngine>,
    pub(crate) screen: Rc<dyn TerminalScreen>,
    pub(crate) writer: Rc<dyn TerminalWriter>,
    pub(crate) clock: Arc<dyn Clock + Send + Sync>,
    pub(crate) feeder: ShellOutputFeeder,
    pub(crate) requester: Rc<CompletionRequester>,
    pub(crate) painter: Rc<OverlayPainter>,
    pub(crate) poller: Rc<SessionFilePoller>,
    pub(crate) handler: Rc<KeystrokeHandler>,
}

/// A batch of the shell's output arrives (SPEC 4.5): the screen takes it, the events of the whole batch are handled,
/// and one write brings the user's terminal up to date. The keys held back for an echo are replayed after that write.
pub(crate) struct ProcessShellOutputUseCase {
    parts: ProcessShellOutputParts,
}

impl ProcessShellOutputUseCase {
    pub(crate) fn new(parts: ProcessShellOutputParts) -> Self {
        ProcessShellOutputUseCase { parts }
    }

    /// Clear, bytes (markers stripped), repaint, frame: one write. The clear is taken after the refresh, so a panic
    /// before it leaves the clear cached for the caller's guard. A panic while the held keys are replayed comes after
    /// the write, which makes the caller write the batch again raw.
    pub(crate) fn execute(&self, session: &mut TerminalSession, batch: &[u8]) {
        let parts = &self.parts;
        session.output_arrived(parts.clock.now_instant());
        let before = parts.screen.state();
        let mut body = Vec::new();
        let (splitter, tracker) = session.output_parts();
        let events = parts.feeder.feed(splitter, tracker, batch, &mut body);
        for event in events {
            self.handle(session, event);
        }
        session.screen_changed(&before, &parts.screen.state());
        parts.requester.refresh(session);
        let mut out = parts.screen.take_clear();
        out.extend(body);
        out.extend(parts.painter.take_repaint(session));
        out.extend(parts.painter.frame(session));
        parts.writer.write(&out);
        parts.handler.release_held(session);
    }

    fn handle(&self, session: &mut TerminalSession, event: ShellEvent) {
        match &event {
            ShellEvent::NewCommand(Some(entry)) => self.parts.engine.command_finished(entry.clone()),
            ShellEvent::CommandEnded => self.parts.engine.command_ended(), // NoHistory: the end-of-command reset, nothing recorded
            ShellEvent::NewCommand(None) | ShellEvent::PreExec => {}
        }
        if session.on_shell_event(&event, self.parts.clock.now_instant()) {
            self.parts.poller.poll(session);
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::completion::domain::test_fixtures::{comp, item, settings};
    use crate::completion::domain::value_objects::suggestion::Suggestion;
    use crate::shared::application::ports::clock::{Clock, FixedClock};
    use crate::specs::domain::value_objects::suggestion_kind::SuggestionKind;
    use crate::terminal_session::application::ports::completion_engine::CompletionEngineCall;
    use crate::terminal_session::application::ports::terminal_screen::TerminalScreen;
    use crate::terminal_session::application::shell_output_feeder::ShellOutputFeeder;
    use crate::terminal_session::application::test_fixtures::{
        Io, PanickingCompletionEngine, TerminalSessionHarness, SID,
    };
    use crate::terminal_session::domain::entities::session_context::SessionContext;
    use crate::terminal_session::domain::repositories::session_context_repository::InMemorySessionContextRepository;
    use crate::terminal_session::domain::services::session_deadlines::RESIZE_DEBOUNCE;
    use crate::terminal_session::domain::value_objects::session_id::SessionId;
    use std::collections::HashMap;
    use std::panic::{catch_unwind, AssertUnwindSafe};
    use std::rc::Rc;
    use std::sync::Arc;
    use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

    const PROMPT: &str = "\x1b]697;StartPrompt\x07$ \x1b]697;EndPrompt\x07";

    /// A session whose shell has printed before the part of the scenario that is looked at. The arrangement goes
    /// through the feeder and the aggregate alone, so it does not depend on the use case under test.
    struct Scene {
        harness: TerminalSessionHarness,
        feeder: ShellOutputFeeder,
    }

    impl Scene {
        fn on(harness: TerminalSessionHarness) -> Self {
            let feeder = ShellOutputFeeder::new(harness.screen.clone(), harness.clock.clone());
            Scene { harness, feeder }
        }

        fn new() -> Self {
            Self::on(TerminalSessionHarness::new("{}"))
        }

        /// The shell prints `output`, markers and all.
        fn shell_prints(&mut self, output: &str) {
            let now = self.harness.clock.now_instant();
            let mut body = Vec::new();
            let (splitter, tracker) = self.harness.session.output_parts();
            let events = self.feeder.feed(splitter, tracker, output.as_bytes(), &mut body);
            for event in events {
                self.harness.session.on_shell_event(&event, now);
            }
        }

        /// The prompt `$ ` with the shell integration markers, then `text` echoed.
        fn shell_prints_prompt_and_echo(&mut self, text: &str) {
            self.shell_prints(&format!("{PROMPT}\x1b]697;NewCmd={SID}\x07{text}"));
        }

        /// The session asks for the line on screen and the engine answers with `items`. The first reply after a reset
        /// only arms the menu, so it comes twice.
        fn menu_shown_for_the_line(&mut self, items: Vec<Suggestion>) {
            let h = &mut self.harness;
            let state = h.screen.state();
            let extracted = h.session.tracker().edit_buffer_context().and_then(|context| h.screen.edit_buffer(&context));
            h.session.refresh(&state, extracted, h.clock.now_instant());
            let buffer = h.session.sent().expect("a request was sent").0.to_string();
            let reply = comp(&buffer, 1, items).with_request_id(h.session.request_id());
            h.session.apply_completion(reply.clone(), h.clock.now_instant());
            h.session.apply_completion(reply, h.clock.now_instant());
        }

        /// What the engine was told, by name, `Complete` left out.
        fn engine_told(&self) -> Vec<&'static str> {
            self.harness
                .engine_calls
                .try_iter()
                .filter_map(|call| match call {
                    CompletionEngineCall::Complete(_) => None,
                    CompletionEngineCall::CommandFinished(_) => Some("CommandFinished"),
                    CompletionEngineCall::ApplySessionContext { .. } => Some("ApplySessionContext"),
                    CompletionEngineCall::ApplySettings(_) => Some("ApplySettings"),
                    _ => Some("other"),
                })
                .collect()
        }
    }

    fn cherry_pick_and_checkout() -> Vec<Suggestion> {
        ["cherry-pick", "checkout"].iter().map(|name| item(name, SuggestionKind::Subcommand, "ch")).collect()
    }

    fn at(secs: u64) -> SystemTime {
        UNIX_EPOCH + Duration::from_secs(secs)
    }

    fn position(haystack: &[u8], needle: &str) -> Option<usize> {
        haystack.windows(needle.len()).position(|window| window == needle.as_bytes())
    }

    #[test]
    fn should_write_the_batch_in_one_write_and_release_held_keys_after_it() {
        let mut scene = Scene::new();
        scene.shell_prints_prompt_and_echo("git che");
        scene.menu_shown_for_the_line(cherry_pick_and_checkout());
        scene.harness.input(b"\x7f");
        scene.harness.input(b"\r");
        scene.harness.session.expire_hold_at(Instant::now());
        let writes_before = scene.harness.journal.events().len();

        scene.harness.output(b"\x08\x1b[K");

        let names: Vec<_> = scene.harness.journal.events().split_off(writes_before).iter().map(|event| match event {
            Io::Stdout(_) => "stdout",
            Io::ShellInput(_) => "shell",
        }).collect();
        // The batch, then what the replayed key does: it draws the menu again and writes to the shell.
        assert_eq!(names, ["stdout", "stdout", "shell"]);
    }

    #[test]
    fn should_send_command_finished_before_the_session_context_and_settings() {
        let id = SessionId::new("abc".to_string()).unwrap();
        let contexts = Rc::new(InMemorySessionContextRepository::default());
        contexts.save_modified_at(&SessionContext::new(id.clone(), HashMap::new(), None, 0), at(7));
        let mut scene = Scene::on(TerminalSessionHarness::with_session_contexts(id, contexts));
        scene.harness.live_settings.set(settings(r#"{"inline.enabled": false}"#), at(5));
        scene.shell_prints_prompt_and_echo("ls");
        scene.shell_prints("\r\n\x1b]697;PreExec\x07");

        scene.harness.output(format!("{PROMPT}\x1b]697;NewCmd={SID}\x07").as_bytes());

        assert_eq!(scene.engine_told(), ["CommandFinished", "ApplySessionContext", "ApplySettings"]);
    }

    #[test]
    fn should_record_the_finished_command_once() {
        let mut scene = Scene::new();
        scene.shell_prints_prompt_and_echo("ls -l");
        scene.shell_prints("\r\n\x1b]697;PreExec\x07");

        scene.harness.output(format!("{PROMPT}\x1b]697;NewCmd={SID}\x07").as_bytes());

        let finished: Vec<_> = scene
            .harness
            .engine_calls
            .try_iter()
            .filter_map(|call| match call {
                CompletionEngineCall::CommandFinished(entry) => Some(entry.command().to_string()),
                _ => None,
            })
            .collect();
        assert_eq!(finished, ["ls -l"]);
    }

    #[test]
    fn should_reset_the_menu_when_the_alternate_screen_ends() {
        let mut scene = Scene::new();
        scene.shell_prints_prompt_and_echo("git ch");
        scene.menu_shown_for_the_line(cherry_pick_and_checkout());
        scene.harness.output(b"\x1b[?1049h");
        let shown_on_the_alternate_screen = scene.harness.session.menu().has_items();

        scene.harness.output(b"\x1b[?1049l");

        assert_eq!(
            (shown_on_the_alternate_screen, scene.harness.session.menu().has_items()),
            (true, false)
        );
    }

    #[test]
    fn should_extend_a_resize_quiet_time_by_output() {
        let clock = Arc::new(FixedClock::new(0));
        let mut harness = TerminalSessionHarness::with_clock("{}", clock.clone());
        harness.session.resize_applied(clock.now_instant());
        clock.advance(RESIZE_DEBOUNCE - Duration::from_millis(1));

        harness.output(b"x");
        clock.advance(Duration::from_millis(2));
        harness.session.settle_resize_if_due(clock.now_instant());

        assert!(!harness.session.take_repaint(&harness.screen.state()), "the quiet time is not over");
    }

    #[test]
    fn should_take_the_clear_after_the_refresh() {
        let mut scene = Scene::on(TerminalSessionHarness::with_engine("{}", Rc::new(PanickingCompletionEngine)));
        scene.shell_prints_prompt_and_echo("git ch");
        scene.menu_shown_for_the_line(cherry_pick_and_checkout());
        scene.harness.redraw();
        scene.harness.clear_written();

        let outcome = catch_unwind(AssertUnwindSafe(|| scene.harness.output(b"e")));

        assert_eq!(
            (
                outcome.is_err(),
                scene.harness.screen.take_clear().is_empty(),
                scene.harness.written().is_empty()
            ),
            (true, false, true)
        );
    }

    #[test]
    fn should_write_the_repaint_before_the_frame_in_the_same_write() {
        let clock = Arc::new(FixedClock::new(0));
        let mut scene = Scene::on(TerminalSessionHarness::with_clock("{}", clock.clone()));
        scene.shell_prints_prompt_and_echo("git ch");
        scene.menu_shown_for_the_line(cherry_pick_and_checkout());
        scene.harness.session.resize_applied(clock.now_instant());
        clock.advance(RESIZE_DEBOUNCE);
        scene.harness.session.settle_resize_if_due(clock.now_instant());

        scene.harness.output(b"");

        let writes = scene.harness.journal.events();
        let [Io::Stdout(written)] = &writes[..] else { panic!("{writes:?}") };
        let repaint = position(written, "git");
        let frame = position(written, "erry-pick");
        assert!(repaint.is_some() && frame.is_some() && repaint < frame, "{:?}", String::from_utf8_lossy(written));
    }
}
