use std::time::{Duration, Instant};

pub(crate) const RESIZE_DEBOUNCE: Duration = Duration::from_millis(150);
/// A prompt redraw whose line arrives in a later read: its empty line is not acted on before this.
pub(crate) const REDRAW_GRACE: Duration = Duration::from_millis(50);
pub(crate) const SPINNER_TICK: Duration = Duration::from_millis(80);
pub(crate) const FILE_POLL: Duration = Duration::from_secs(2);
/// After the child exits, wait this long for the rest of its output (PTY EOF) before leaving.
pub(crate) const EXIT_GRACE: Duration = Duration::from_millis(200);

/// When the session loop must wake up next. In a pass-through only the exit grace counts; otherwise every pending
/// deadline does (`poll` is always pending, so the loop never waits forever).
pub(crate) fn next_deadline(
    passthrough: bool,
    exit: Option<Instant>,
    pending: impl IntoIterator<Item = Option<Instant>>,
) -> Option<Instant> {
    if passthrough {
        return exit;
    }
    pending.into_iter().chain([exit]).flatten().min()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn should_wake_for_the_earliest_pending_deadline() {
        let now = Instant::now();
        let at = |secs| Some(now + Duration::from_secs(secs));

        let next = next_deadline(false, at(3), [at(5), None, at(2)]);

        assert_eq!(next, at(2));
    }

    #[test]
    fn should_look_only_at_the_exit_deadline_in_a_passthrough() {
        let now = Instant::now();
        let at = |secs| Some(now + Duration::from_secs(secs));

        let next = next_deadline(true, at(9), [at(1)]);

        assert_eq!(next, at(9));
    }
}
