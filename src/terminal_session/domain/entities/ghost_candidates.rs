use crate::ghost_text::domain::value_objects::ghost_text::GhostText;
use std::time::Instant;

/// The ghost texts the session can show (SPEC §8): the local one (from the engine's reply) and the LLM one, with the
/// LLM request's id and its debounce deadline.
#[derive(Clone, Debug, Default)]
pub(crate) struct GhostCandidates {
    local: Option<GhostText>,
    llm: Option<GhostText>,
    llm_request: u64,
    llm_due_at: Option<Instant>,
}

impl GhostCandidates {
    /// A command started or ended: nothing computed for the old line survives.
    pub(crate) fn reset(&mut self) {
        self.local = None;
        self.llm = None;
        self.llm_due_at = None;
    }

    /// The user accepted a ghost text: the debounce of a pending LLM request goes on.
    pub(crate) fn accepted(&mut self) {
        self.local = None;
        self.llm = None;
    }

    pub(crate) fn replace_local(&mut self, ghost: Option<GhostText>) {
        self.local = ghost;
    }

    pub(crate) fn schedule_llm(&mut self, at: Instant) {
        self.llm_due_at = Some(at);
    }

    pub(crate) fn cancel_llm_schedule(&mut self) {
        self.llm_due_at = None;
    }

    pub(crate) fn llm_due_at(&self) -> Option<Instant> {
        self.llm_due_at
    }

    /// The LLM request is due: the deadline is spent.
    pub(crate) fn take_due_llm(&mut self, now: Instant) -> bool {
        if self.llm_due_at.is_some_and(|d| d <= now) {
            self.llm_due_at = None;
            return true;
        }
        false
    }

    /// The LLM request for `request` went out: only its reply is taken.
    pub(crate) fn expect_llm_reply(&mut self, request: u64) {
        self.llm_request = request;
    }

    /// The LLM's reply: kept for the buffer the latest request was sent for, and only if it answers the latest
    /// request that went out. True when it was taken (the frame must be drawn again).
    pub(crate) fn accept_llm_reply(
        &mut self,
        reply: u64,
        latest_request: u64,
        text: Option<String>,
        sent_buffer: Option<&str>,
    ) -> bool {
        if reply != latest_request || reply != self.llm_request {
            return false;
        }
        self.llm = text
            .zip(sent_buffer.map(String::from))
            .map(|(text, buffer)| GhostText::new(buffer, text));
        true
    }

    /// What is left of the first candidate that still fits `buffer`: the local one wins over the LLM's.
    pub(crate) fn rest_after(&self, buffer: &str) -> Option<String> {
        [&self.local, &self.llm]
            .into_iter()
            .flatten()
            .find_map(|g| g.rest_after(buffer))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn should_prefer_the_local_ghost_text_over_the_llm_one_of_the_current_request() {
        let mut ghost = GhostCandidates::default();
        ghost.expect_llm_reply(3);

        let taken = ghost.accept_llm_reply(3, 3, Some("tus".into()), Some("git sta"));
        let llm_only = ghost.rest_after("git sta");
        ghost.replace_local(Some(GhostText::new("git sta".into(), "sh".into())));
        let with_local = ghost.rest_after("git sta");

        assert_eq!((taken, llm_only, with_local), (true, Some("tus".to_string()), Some("sh".to_string())));
    }

    #[test]
    fn should_ignore_an_llm_reply_for_another_request() {
        let mut ghost = GhostCandidates::default();
        ghost.expect_llm_reply(3);

        let taken = ghost.accept_llm_reply(4, 4, Some("x".into()), Some("git sta"));

        assert_eq!((taken, ghost.rest_after("git sta")), (false, None));
    }

    #[test]
    fn should_clear_the_llm_ghost_text_for_an_empty_reply() {
        let mut ghost = GhostCandidates::default();
        ghost.expect_llm_reply(3);
        ghost.accept_llm_reply(3, 3, Some("tus".into()), Some("git sta"));

        let taken = ghost.accept_llm_reply(3, 3, None, Some("git sta"));

        assert_eq!((taken, ghost.rest_after("git sta")), (true, None));
    }

    #[test]
    fn should_keep_the_llm_debounce_when_a_ghost_text_is_accepted_but_not_when_a_command_starts() {
        let at = Instant::now();
        let mut ghost = GhostCandidates::default();
        ghost.schedule_llm(at);

        ghost.accepted();
        let after_accept = ghost.llm_due_at();
        ghost.reset();

        assert_eq!((after_accept, ghost.llm_due_at()), (Some(at), None));
    }

    #[test]
    fn should_spend_the_deadline_when_the_llm_request_is_taken() {
        let at = Instant::now();
        let mut ghost = GhostCandidates::default();
        ghost.schedule_llm(at);

        let early = ghost.take_due_llm(at - std::time::Duration::from_millis(1));
        let due = ghost.take_due_llm(at);

        assert_eq!((early, due, ghost.llm_due_at()), (false, true, None));
    }
}
