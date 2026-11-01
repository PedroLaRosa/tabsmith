use crate::terminal_session::domain::services::session_deadlines::EXIT_GRACE;
use std::time::Instant;

/// How the wrapped shell ends: its exit code, the end of its output, and the grace the rest of its output gets.
#[derive(Clone, Debug, Default)]
pub(crate) struct ShellExit {
    code: Option<i32>,
    output_ended: bool,
    leave_at: Option<Instant>,
}

impl ShellExit {
    /// The child exited. True while the session must go on (waiting for the rest of its output).
    pub(crate) fn child_exited(&mut self, code: i32, now: Instant) -> bool {
        self.code = Some(code);
        if self.output_ended {
            return false;
        }
        self.leave_at = Some(now + EXIT_GRACE);
        true
    }

    /// The pty reached EOF. True while the session must go on (the child has not exited yet).
    pub(crate) fn output_ended(&mut self) -> bool {
        self.output_ended = true;
        self.code.is_none()
    }

    /// The wrapper was told to stop (SIGTERM and SIGHUP alike).
    pub(crate) fn terminated(&mut self) {
        self.code = Some(129);
    }

    pub(crate) fn grace_over(&self, now: Instant) -> bool {
        self.leave_at.is_some_and(|d| d <= now)
    }

    pub(crate) fn deadline(&self) -> Option<Instant> {
        self.leave_at
    }

    pub(crate) fn code(&self) -> Option<i32> {
        self.code
    }

    /// The code the wrapper exits with: 0 when the child never reported one.
    pub(crate) fn exit_code(&self) -> i32 {
        self.code.unwrap_or(0)
    }

    /// Test arrangement and inspection: whether the pty reached EOF.
    #[cfg(test)]
    pub(crate) fn has_seen_eof(&self) -> bool {
        self.output_ended
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn should_wait_the_grace_for_the_rest_of_the_output_after_the_child_exited() {
        let now = Instant::now();
        let mut exit = ShellExit::default();

        let goes_on = exit.child_exited(3, now);

        assert_eq!(
            (goes_on, exit.grace_over(now + Duration::from_millis(199)), exit.grace_over(now + EXIT_GRACE)),
            (true, false, true)
        );
    }

    #[test]
    fn should_leave_at_once_when_the_output_had_already_ended() {
        let mut exit = ShellExit::default();
        exit.output_ended();

        let goes_on = exit.child_exited(0, Instant::now());

        assert_eq!((goes_on, exit.deadline()), (false, None));
    }

    #[test]
    fn should_go_on_after_the_output_ended_until_the_child_exits() {
        let mut exit = ShellExit::default();

        let before_exit = exit.output_ended();
        exit.child_exited(5, Instant::now());

        assert_eq!((before_exit, exit.code()), (true, Some(5)));
    }

    #[test]
    fn should_exit_with_129_when_terminated_and_with_zero_when_nothing_was_reported() {
        let mut terminated = ShellExit::default();
        terminated.terminated();

        assert_eq!((terminated.exit_code(), ShellExit::default().exit_code()), (129, 0));
    }
}
