use crate::modules::terminal_session::application::overlay_painter::OverlayPainter;
use crate::modules::terminal_session::domain::entities::terminal_session::TerminalSession;
use std::rc::Rc;

/// The LLM's ghost text for a request arrives (SPEC §8.1).
pub(crate) struct ApplyLlmGhostTextUseCase {
    painter: Rc<OverlayPainter>,
}

impl ApplyLlmGhostTextUseCase {
    pub(crate) fn new(painter: Rc<OverlayPainter>) -> Self {
        ApplyLlmGhostTextUseCase { painter }
    }

    /// Only the reply to the current request is taken, and drawn. Unlike a completion reply it does not release the
    /// keys held back for an echo.
    pub(crate) fn execute(&self, session: &mut TerminalSession, request: u64, text: Option<String>) {
        if session.accept_llm_reply(request, text) {
            self.painter.redraw(session);
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::modules::dropdown::domain::value_objects::key_action::KeyAction;
    use crate::modules::terminal_session::application::test_fixtures::{subs, TerminalSessionHarness};

    /// A menu on a prompt whose selection then moved without a frame being drawn: any redraw would write.
    fn a_menu_moved_without_being_drawn() -> TerminalSessionHarness {
        let mut harness = TerminalSessionHarness::new("{}");
        harness.prompt("git ch");
        harness.reply(subs("ch"), None);
        harness.session.run_menu_action(KeyAction::NavigateDown);
        harness.clear_written();
        harness
    }

    #[test]
    fn should_not_release_held_keys_for_an_llm_reply() {
        let mut harness = TerminalSessionHarness::new("{}");
        harness.prompt("git che");
        harness.reply(subs("che"), None);
        harness.input(b"\x7f");
        harness.input(b"\r");
        harness.sent();
        harness.session.expire_hold_at(harness.clock.now_instant());
        harness.session.expect_llm_reply(harness.session.request_id());

        harness.llm_reply(harness.session.request_id(), Some("ckout"));

        assert_eq!(
            (harness.sent(), harness.session.is_holding_keys(), harness.ghost_text(false)),
            (Vec::new(), true, Some("ckout".to_string()))
        );
    }

    #[test]
    fn should_draw_nothing_for_an_llm_reply_to_another_request() {
        let mut harness = a_menu_moved_without_being_drawn();
        harness.session.expect_llm_reply(harness.session.request_id());

        harness.llm_reply(harness.session.request_id() + 1, Some("x"));

        assert!(harness.written().is_empty());
    }

    #[test]
    fn should_show_the_llm_ghost_text_of_the_current_request_behind_the_local_one() {
        let mut h = TerminalSessionHarness::new("{}");
        h.prompt("git sta");
        h.session.expect_llm_reply(h.session.request_id());

        h.llm_reply(h.session.request_id(), Some("tus"));
        let from_the_current_request = h.ghost_text(false);
        h.llm_reply(h.session.request_id() + 1, Some("x"));
        let after_another_request = h.ghost_text(false);
        h.llm_reply(h.session.request_id(), None);
        let after_an_empty_reply = h.ghost_text(false);
        h.llm_reply(h.session.request_id(), Some("tus"));
        h.reply(Vec::new(), Some("sh"));
        let with_a_local_ghost_text = h.ghost_text(false);

        assert_eq!(
            (from_the_current_request, after_another_request, after_an_empty_reply, with_a_local_ghost_text),
            (Some("tus".to_string()), Some("tus".to_string()), None, Some("sh".to_string()))
        );
    }
}
