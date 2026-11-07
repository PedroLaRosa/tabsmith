use crate::modules::shared::application::ports::clock::Clock;
use crate::modules::terminal_session::application::completion_requester::CompletionRequester;
use crate::modules::terminal_session::application::keystroke_handler::KeystrokeHandler;
use crate::modules::terminal_session::application::overlay_painter::OverlayPainter;
use crate::modules::terminal_session::application::ports::completion_engine::CompletionEngine;
use crate::modules::terminal_session::application::ports::terminal_writer::TerminalWriter;
use crate::modules::terminal_session::application::session_file_poller::SessionFilePoller;
use crate::modules::terminal_session::domain::entities::terminal_session::TerminalSession;
use std::rc::Rc;
use std::sync::Arc;

/// A time-based deadline of the session may have come (SPEC 4.5): the resize settles, the line is looked at again, the
/// held keys are replayed, the LLM is asked, the spinner turns and the files are polled. One tick, one write.
pub(crate) struct HandleTimerTickUseCase {
    engine: Rc<dyn CompletionEngine>,
    writer: Rc<dyn TerminalWriter>,
    clock: Arc<dyn Clock + Send + Sync>,
    requester: Rc<CompletionRequester>,
    painter: Rc<OverlayPainter>,
    poller: Rc<SessionFilePoller>,
    handler: Rc<KeystrokeHandler>,
}

impl HandleTimerTickUseCase {
    pub(crate) fn new(
        engine: Rc<dyn CompletionEngine>,
        writer: Rc<dyn TerminalWriter>,
        clock: Arc<dyn Clock + Send + Sync>,
        requester: Rc<CompletionRequester>,
        painter: Rc<OverlayPainter>,
        poller: Rc<SessionFilePoller>,
        handler: Rc<KeystrokeHandler>,
    ) -> Self {
        HandleTimerTickUseCase { engine, writer, clock, requester, painter, poller, handler }
    }

    /// The tick's `now` is read once. The order is fixed (P9-T67): the resize settles, a prompt redraw's grace ends,
    /// held keys are replayed, and only then is the insertion lock judged, because the replay may have created one.
    /// The LLM is asked, the spinner turns (judged at a fresh read), and the files are polled before anything is
    /// drawn: a reload changes the theme and the settings the frame is built from. Repaint and frame go out in one
    /// write, else a plain redraw.
    pub(crate) fn execute(&self, session: &mut TerminalSession) {
        let now = self.clock.now_instant();
        let mut redraw = false;
        session.settle_resize_if_due(now);
        if session.take_due_redraw_grace(now) {
            self.requester.refresh(session); // the line really is empty
            redraw = true;
        }
        self.handler.release_held(session);
        if session.insertion_lock_due(now) {
            self.requester.refresh(session); // releases the lock and catches up with the buffer
            redraw = true;
        }
        if let Some((request, buffer)) = session.take_due_llm_request(now) {
            self.engine.request_llm_ghost_text(
                request,
                buffer,
                self.requester.working_directory_of(session),
                session.shell(),
            );
        }
        redraw |= session.tick_spinner(now, self.clock.now_instant());
        if session.poll_due(now) {
            redraw |= self.poller.poll(session);
        }
        let repaint = self.painter.take_repaint(session);
        if !repaint.is_empty() {
            let mut out = repaint;
            out.extend(self.painter.frame(session));
            self.writer.write(&out);
        } else if redraw {
            self.painter.redraw(session);
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::modules::completion::domain::test_fixtures::{comp, item, settings};
    use crate::modules::completion::domain::value_objects::completion_result::CompletionResult;
    use crate::modules::shared::application::ports::clock::{Clock, FixedClock};
    use crate::modules::shared::domain::value_objects::shell::Shell;
    use crate::modules::specs::domain::value_objects::suggestion_kind::SuggestionKind;
    use crate::modules::terminal_session::application::ports::completion_engine::CompletionEngineCall;
    use crate::modules::terminal_session::application::test_fixtures::{shell_line, subs, Io, TerminalSessionHarness};
    use crate::modules::terminal_session::domain::services::session_deadlines::{FILE_POLL, REDRAW_GRACE, RESIZE_DEBOUNCE};
    use std::sync::Arc;
    use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

    const LLM_SETTINGS: &str = r#"{"inline.source": "mock", "inline.debounceMs": 300, "inline.profiles.mock": {"provider": "openai", "model": "m", "base_url": "http://127.0.0.1:9/v1"}}"#;
    const LIGHT_THEME: &str = r#"{"autocomplete.theme": "light"}"#;

    fn at(secs: u64) -> SystemTime {
        UNIX_EPOCH + Duration::from_secs(secs)
    }

    /// A prompt with `git ch` and its menu of two items, on a clock that stands still until the test moves it.
    fn menu_shown(settings_json: &str) -> (TerminalSessionHarness, Arc<FixedClock>) {
        let clock = Arc::new(FixedClock::new(0));
        let mut harness = TerminalSessionHarness::with_clock(settings_json, clock.clone());
        harness.prompt("git ch");
        harness.reply(subs("ch"), None);
        (harness, clock)
    }

    /// A prompt with an unknown command whose reply armed the LLM debounce.
    fn waiting_for_the_llm_debounce() -> (TerminalSessionHarness, Arc<FixedClock>) {
        let clock = Arc::new(FixedClock::new(0));
        let mut harness = TerminalSessionHarness::with_clock(LLM_SETTINGS, clock.clone());
        harness.prompt("zzzcmd foo");
        harness.complete(CompletionResult::error(harness.session.request_id(), "zzzcmd foo".into(), 10));
        harness.engine_calls();
        (harness, clock)
    }

    #[test]
    fn should_request_the_llm_ghost_text_when_the_debounce_ran_out() {
        let (mut harness, clock) = waiting_for_the_llm_debounce();
        clock.advance(Duration::from_millis(300));

        harness.tick();

        let calls = harness.engine_calls();
        let [CompletionEngineCall::RequestLlmGhostText { request, buffer, cwd, shell }] = &calls[..] else {
            panic!("{calls:?}")
        };
        assert_eq!(
            (*request, buffer.as_str(), cwd.clone(), *shell),
            (harness.session.request_id(), "zzzcmd foo", std::env::current_dir().unwrap(), Shell::Bash)
        );
    }

    #[test]
    fn should_drop_the_llm_schedule_when_nothing_was_sent() {
        let (mut harness, clock) = waiting_for_the_llm_debounce();
        harness.session.forget_sent();
        clock.advance(Duration::from_millis(300));

        harness.tick();

        assert_eq!((harness.engine_calls().len(), harness.session.llm_due_at()), (0, None));
    }

    #[test]
    fn should_spin_while_a_reply_is_loading() {
        let (mut harness, clock) = menu_shown("{}");
        harness
            .complete(comp("git ch", 1, subs("ch")).with_request_id(harness.session.request_id()).with_loading(true));
        clock.advance(Duration::from_millis(300));
        harness.clear_written();

        harness.tick();

        assert_eq!((harness.session.spinner_frame(), harness.written().is_empty()), (1, false));
    }

    #[test]
    fn should_poll_the_files_only_after_the_poll_interval() {
        let (mut harness, clock) = menu_shown("{}");
        let reloaded = settings(r#"{"inline.enabled": false}"#);
        harness.live_settings.set(reloaded.clone(), at(5));
        clock.advance(FILE_POLL - Duration::from_millis(1));
        harness.tick();
        let settings_while_waiting = harness.session.settings().clone();

        clock.advance(Duration::from_millis(1));
        harness.tick();

        assert_eq!((settings_while_waiting, harness.session.settings().clone()), (settings("{}"), reloaded));
    }

    #[test]
    fn should_release_held_keys_before_checking_the_insertion_lock() {
        let clock = Arc::new(FixedClock::new(0));
        let mut harness = TerminalSessionHarness::with_clock("{}", clock.clone());
        harness.prompt("git chec");
        let items =
            ["checkout", "checkout-index"].iter().map(|name| item(name, SuggestionKind::Subcommand, "chec")).collect();
        harness.reply(items, Some("kout feature-login"));
        harness.input(b"\t");
        harness.session.hold_keys(b"\x1b[C", clock.now_instant());
        harness.session.expire_insertion_lock(clock.now_instant(), clock.now_instant());
        harness.sent();

        harness.tick();

        // The lock still holds when the key is replayed, so the ghost text is built on the line with the Tab's insertion.
        assert_eq!(shell_line("git chec", &[&b"kout"[..], &harness.sent()].concat()), "git checkout feature-login");
    }

    #[test]
    fn should_write_the_repaint_and_the_frame_in_one_write() {
        let (mut harness, clock) = menu_shown("{}");
        harness.resize(100, 30);
        clock.advance(RESIZE_DEBOUNCE);
        let writes_before = harness.journal.events().len();

        harness.tick();

        let writes = harness.journal.events().split_off(writes_before);
        let [Io::Stdout(written)] = &writes[..] else { panic!("{writes:?}") };
        assert!(String::from_utf8_lossy(written).contains("erry-pick"), "{:?}", String::from_utf8_lossy(written));
    }

    #[test]
    fn should_act_on_an_empty_line_once_the_redraw_grace_is_over() {
        let (mut harness, clock) = menu_shown("{}");
        harness.redraw_prompt("");
        clock.advance(REDRAW_GRACE - Duration::from_millis(1));
        harness.tick();
        let kept_during_the_grace = harness.session.menu().has_items();

        clock.advance(Duration::from_millis(1));
        harness.tick();

        assert_eq!((kept_during_the_grace, harness.session.menu().has_items()), (true, false));
    }

    #[test]
    fn should_redraw_when_the_settings_changed() {
        let (mut harness, clock) = menu_shown("{}");
        harness.live_settings.set(settings(LIGHT_THEME), at(5));
        clock.advance(FILE_POLL);
        harness.clear_written();

        harness.tick();

        assert!(!harness.written().is_empty());
    }

    /// A menu on screen, a resize that settles and a settings file with the light theme, all due at `FILE_POLL`.
    fn settled_resize_and_changed_settings() -> (TerminalSessionHarness, Arc<FixedClock>) {
        let (mut harness, clock) = menu_shown("{}");
        harness.resize(100, 30);
        harness.live_settings.set(settings(LIGHT_THEME), at(5));
        clock.advance(FILE_POLL);
        harness.clear_written();
        (harness, clock)
    }

    #[test]
    fn should_poll_the_files_before_drawing_the_repaint_and_the_frame() {
        let (mut polled_earlier, _) = settled_resize_and_changed_settings();
        polled_earlier.use_cases.poller().poll(&mut polled_earlier.session);
        polled_earlier.tick();
        let (mut same_tick, _) = settled_resize_and_changed_settings();

        same_tick.tick();

        assert_eq!((same_tick.written().is_empty(), same_tick.written() == polled_earlier.written()), (false, true));
    }

    #[test]
    fn slow_echo_keeps_the_insertion_in_the_basis() {
        // Tab inserts `kout`; the shell has echoed nothing when the 100 ms lock runs out, then Right accepts the ghost.
        let mut h = TerminalSessionHarness::new("{}");
        h.prompt("git chec");
        let items = ["checkout", "checkout-index"]
            .iter()
            .map(|n| item(n, SuggestionKind::Subcommand, "chec"))
            .collect();
        h.reply(items, Some("kout feature-login"));
        h.input(b"\t");
        h.session.expire_insertion_deadline(Instant::now());
        h.tick();
        assert!(h.session.has_insertion_lock(), "re-armed: nothing echoed");
        h.input(b"\x1b[C");
        assert_eq!(
            shell_line("git chec", &h.sent()),
            "git checkout feature-login"
        );
        // Never past the cap.
        h.session.expire_insertion_lock(Instant::now(), Instant::now());
        h.tick();
        assert!(!h.session.has_insertion_lock());
    }
}
