use crate::completion::domain::value_objects::completion_result::CompletionResult;
use crate::shared::application::ports::clock::Clock;
use crate::terminal_session::application::keystroke_handler::KeystrokeHandler;
use crate::terminal_session::application::overlay_painter::OverlayPainter;
use crate::terminal_session::domain::entities::terminal_session::TerminalSession;
use std::rc::Rc;
use std::sync::Arc;

/// The completion engine's reply arrives (SPEC §4.5).
pub(crate) struct ApplyCompletionResultUseCase {
    clock: Arc<dyn Clock + Send + Sync>,
    painter: Rc<OverlayPainter>,
    handler: Rc<KeystrokeHandler>,
}

impl ApplyCompletionResultUseCase {
    pub(crate) fn new(
        clock: Arc<dyn Clock + Send + Sync>,
        painter: Rc<OverlayPainter>,
        handler: Rc<KeystrokeHandler>,
    ) -> Self {
        ApplyCompletionResultUseCase { clock, painter, handler }
    }

    /// A reply for another request than the latest changes nothing: no redraw, no release of held keys. An accepted
    /// one is drawn, and the keys held back for the echo of the line it answers are replayed after the frame.
    pub(crate) fn execute(&self, session: &mut TerminalSession, result: CompletionResult) {
        if !session.apply_completion(result, self.clock.now_instant()) {
            return;
        }
        self.painter.redraw(session);
        self.handler.release_held(session);
    }
}

#[cfg(test)]
mod tests {
    use crate::completion::domain::test_fixtures::comp;
    use crate::completion::domain::value_objects::completion_phase::CompletionPhase;
    use crate::completion::domain::value_objects::completion_result::CompletionResult;
    use crate::dropdown::domain::value_objects::key_action::KeyAction;
    use crate::shared::application::ports::clock::{Clock, FixedClock};
    use crate::terminal_session::application::test_fixtures::{subs, Io, TerminalSessionHarness};
    use crate::terminal_session::domain::services::session_deadlines::SPINNER_TICK;
    use std::sync::Arc;
    use std::time::Duration;

    const LLM_SETTINGS: &str = r#"{"inline.source": "mock", "inline.debounceMs": 300, "inline.profiles.mock": {"provider": "openai", "model": "m", "base_url": "http://127.0.0.1:9/v1"}}"#;

    /// A prompt with `git che` typed, its menu shown, and Backspace then Enter typed: Enter waits for the echo.
    fn waiting_for_the_echo_of_a_backspace() -> TerminalSessionHarness {
        let mut harness = TerminalSessionHarness::new("{}");
        harness.prompt("git che");
        harness.reply(subs("che"), None);
        harness.input(b"\x7f");
        harness.input(b"\r");
        harness.clear_written();
        harness
    }

    #[test]
    fn should_drop_a_reply_for_an_older_request_without_drawing() {
        let mut harness = TerminalSessionHarness::new("{}");
        harness.prompt("git ch");
        harness.reply(subs("ch"), None);
        harness.session.run_menu_action(KeyAction::NavigateDown);
        harness.clear_written();
        let older = comp("git ch", 1, subs("ch")).with_request_id(harness.session.request_id() - 1);

        harness.complete(older);

        assert!(harness.written().is_empty());
    }

    #[test]
    fn should_arm_the_llm_debounce_from_the_time_the_line_changed() {
        let clock = Arc::new(FixedClock::new(0));
        let mut harness = TerminalSessionHarness::with_clock(LLM_SETTINGS, clock.clone());
        harness.prompt("zzzcmd foo");
        let line_changed_at = clock.now_instant();
        clock.advance(Duration::from_millis(500));

        harness.complete(CompletionResult::error(harness.session.request_id(), "zzzcmd foo".into(), 10));

        assert_eq!(harness.session.llm_due_at(), Some(line_changed_at + Duration::from_millis(300)));
    }

    #[test]
    fn should_arm_the_spinner_only_once() {
        let clock = Arc::new(FixedClock::new(0));
        let mut harness = TerminalSessionHarness::with_clock("{}", clock.clone());
        harness.prompt("git ch");
        let first_loading_at = clock.now_instant();
        let loading = || comp("git ch", 1, subs("ch")).with_loading(true);
        harness.complete(loading().with_request_id(harness.session.request_id()));
        clock.advance(Duration::from_millis(50));
        harness.complete(loading().with_request_id(harness.session.request_id()));

        harness.session.tick_spinner(first_loading_at + SPINNER_TICK, first_loading_at + SPINNER_TICK);

        assert_eq!(harness.session.spinner_frame(), 1);
    }

    #[test]
    fn should_keep_the_local_ghost_of_a_generators_reply_without_one() {
        let mut harness = TerminalSessionHarness::new("{}");
        harness.prompt("git ch");
        harness.reply(subs("ch"), Some("eckout"));
        let from_the_generators =
            comp("git ch", 1, subs("ch")).with_request_id(harness.session.request_id()).with_phase(CompletionPhase::Generators);

        harness.complete(from_the_generators);

        assert_eq!(harness.ghost_text(false), Some("eckout".to_string()));
    }

    #[test]
    fn should_clear_the_echo_wait_only_for_a_later_request() {
        let mut harness = waiting_for_the_echo_of_a_backspace();
        let same_request = comp("git che", 1, subs("che")).with_request_id(harness.session.request_id());
        harness.complete(same_request);
        let held_after_the_same_request = harness.session.is_holding_keys();
        harness.output(b"\x08\x1b[K");

        harness.reply(subs("ch"), None);

        assert_eq!((held_after_the_same_request, harness.session.is_holding_keys()), (true, false));
    }

    #[test]
    fn should_redraw_then_release_the_held_keys() {
        let mut harness = waiting_for_the_echo_of_a_backspace();
        harness.output(b"\x08\x1b[K");
        let writes_before = harness.journal.events().len();
        let answer = comp("git ch", 1, subs("ch")).with_request_id(harness.session.request_id());

        harness.complete(answer);

        let order: Vec<_> = harness
            .journal
            .events()
            .into_iter()
            .skip(writes_before)
            .map(|event| match event {
                Io::Stdout(_) => "frame",
                Io::ShellInput(_) => "shell",
            })
            .collect();
        // The reply's frame, then the replayed Enter draws its own, then it reaches the shell.
        assert_eq!(order, ["frame", "frame", "shell"]);
    }
}
