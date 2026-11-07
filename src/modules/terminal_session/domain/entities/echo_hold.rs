use crate::modules::terminal_session::domain::entities::insertion_lock::INSERT_LOCK;
use crate::modules::terminal_session::domain::services::insertion_reconciler::unechoed_after;
use std::time::Instant;

/// What was sent to the shell ahead of its echo (SPEC §7.3): the printable keys typed but not yet shown, the request
/// of the last key the echo prediction cannot model, and the keys held back until that key's reply came.
#[derive(Clone, Debug, Default)]
pub(crate) struct EchoHold {
    /// Printable keys forwarded but not yet echoed (reconciliation basis).
    unechoed: String,
    /// The request id at the time a key the echo prediction cannot model was forwarded; cleared by a reply for a
    /// later request.
    pending: Option<u64>,
    /// Keys that would act on the menu or ghost while `pending`: replayed once the reply came, or at `held_at`.
    held: Vec<u8>,
    held_at: Option<Instant>,
}

impl EchoHold {
    pub(crate) fn unechoed(&self) -> &str {
        &self.unechoed
    }

    /// The shell showed `shown`: keys typed after `base` that it echoed are no longer ahead of it.
    pub(crate) fn consume_echo(&mut self, base: Option<&str>, shown: Option<&str>, insertion_in_flight: bool) {
        if !self.unechoed.is_empty() {
            self.unechoed = unechoed_after(&self.unechoed, base, shown, insertion_in_flight);
        }
    }

    /// A printable char at the end of the line comes back as echo.
    pub(crate) fn typed_ahead(&mut self, c: char) {
        self.unechoed.push(c);
    }

    /// Anything else forwarded (backspace, arrows, control keys, paste) cannot be modelled and drops the prediction.
    pub(crate) fn forwarded_unmodelled(&mut self, request: u64) {
        self.unechoed.clear();
        self.pending = Some(request);
    }

    pub(crate) fn accepted_ghost(&mut self, text: &str) {
        self.unechoed.push_str(text);
    }

    pub(crate) fn forget_unechoed(&mut self) {
        self.unechoed.clear();
    }

    /// A command started or ended: nothing is ahead of the shell any more (held keys stay held).
    pub(crate) fn reset(&mut self) {
        self.unechoed.clear();
        self.pending = None;
    }

    pub(crate) fn forget_pending(&mut self) {
        self.pending = None;
    }

    /// A reply for a request later than the unmodelled key's has come.
    pub(crate) fn answered_by(&mut self, reply: u64) {
        if self.pending.is_some_and(|r| reply > r) {
            self.pending = None;
        }
    }

    pub(crate) fn is_waiting_for_echo(&self) -> bool {
        self.pending.is_some()
    }

    pub(crate) fn is_holding(&self) -> bool {
        !self.held.is_empty()
    }

    /// Keys keep their order behind held ones.
    pub(crate) fn hold_behind(&mut self, keys: &[u8]) {
        self.held.extend_from_slice(keys);
    }

    pub(crate) fn hold(&mut self, keys: &[u8], now: Instant) {
        self.held.extend_from_slice(keys);
        self.held_at = Some(now + INSERT_LOCK);
    }

    /// Replay held keys once the reply for the buffer after the unmodelled key came (or the hold timed out).
    pub(crate) fn release(&mut self, now: Instant) -> Option<Vec<u8>> {
        let timed_out = self.held_at.is_some_and(|t| t <= now);
        if self.held.is_empty() || (self.pending.is_some() && !timed_out) {
            return None;
        }
        self.pending = None;
        self.held_at = None;
        Some(std::mem::take(&mut self.held))
    }

    pub(crate) fn deadline(&self) -> Option<Instant> {
        self.held_at
    }
}

#[cfg(test)]
impl EchoHold {
    pub(crate) fn expire_hold_at(&mut self, at: Instant) {
        self.held_at = Some(at);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn should_keep_the_typed_ahead_keys_the_shell_has_not_echoed() {
        let mut hold = EchoHold::default();
        hold.typed_ahead('l');
        hold.typed_ahead('o');

        hold.consume_echo(Some("hel"), Some("hell"), false);

        assert_eq!(hold.unechoed(), "o");
    }

    #[test]
    fn should_forget_the_prediction_and_wait_for_the_reply_after_an_unmodelled_key() {
        let mut hold = EchoHold::default();
        hold.typed_ahead('x');

        hold.forwarded_unmodelled(7);

        assert_eq!((hold.unechoed(), hold.is_waiting_for_echo()), ("", true));
    }

    #[test]
    fn should_stop_waiting_only_for_a_reply_to_a_later_request() {
        let mut hold = EchoHold::default();
        hold.forwarded_unmodelled(7);

        hold.answered_by(7);
        let after_the_same_request = hold.is_waiting_for_echo();
        hold.answered_by(8);

        assert_eq!((after_the_same_request, hold.is_waiting_for_echo()), (true, false));
    }

    #[test]
    fn should_not_release_held_keys_while_the_reply_is_pending_and_the_hold_is_young() {
        let now = Instant::now();
        let mut hold = EchoHold::default();
        hold.forwarded_unmodelled(1);
        hold.hold(b"\r", now);

        let released = hold.release(now + Duration::from_millis(99));

        assert_eq!(released, None);
    }

    #[test]
    fn should_release_held_keys_once_the_hold_timed_out_even_without_a_reply() {
        let now = Instant::now();
        let mut hold = EchoHold::default();
        hold.forwarded_unmodelled(1);
        hold.hold(b"\r", now);

        let released = hold.release(now + INSERT_LOCK);

        assert_eq!((released, hold.is_waiting_for_echo(), hold.deadline()), (Some(b"\r".to_vec()), false, None));
    }

    #[test]
    fn should_release_held_keys_when_the_reply_came() {
        let now = Instant::now();
        let mut hold = EchoHold::default();
        hold.forwarded_unmodelled(1);
        hold.hold(b"\x1b[C", now);
        hold.answered_by(2);

        let released = hold.release(now);

        assert_eq!(released, Some(b"\x1b[C".to_vec()));
    }

    #[test]
    fn should_keep_the_held_keys_in_their_order() {
        let now = Instant::now();
        let mut hold = EchoHold::default();
        hold.hold(b"a", now);

        hold.hold_behind(b"b");

        assert_eq!(hold.release(now + INSERT_LOCK), Some(b"ab".to_vec()));
    }

    #[test]
    fn should_leave_held_keys_alone_when_a_command_starts() {
        let now = Instant::now();
        let mut hold = EchoHold::default();
        hold.forwarded_unmodelled(1);
        hold.hold(b"x", now);

        hold.reset();

        assert_eq!((hold.is_holding(), hold.is_waiting_for_echo()), (true, false));
    }
}
