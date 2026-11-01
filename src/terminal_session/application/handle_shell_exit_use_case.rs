use crate::shared::application::ports::clock::Clock;
use crate::terminal_session::application::shell_end::ShellEnd;
use crate::terminal_session::domain::entities::terminal_session::TerminalSession;
use std::sync::Arc;

/// The shell's child exited, its output ended, or the wrapper was told to stop.
pub(crate) struct HandleShellExitUseCase {
    clock: Arc<dyn Clock + Send + Sync>,
}

impl HandleShellExitUseCase {
    pub(crate) fn new(clock: Arc<dyn Clock + Send + Sync>) -> Self {
        HandleShellExitUseCase { clock }
    }

    /// True while the loop goes on: the output may still hold the rest after the child exited (a grace of 200 ms), and
    /// the child may still exit after the output ended. The runner has already released the ConPTY master.
    pub(crate) fn execute(&self, session: &mut TerminalSession, end: ShellEnd) -> bool {
        match end {
            ShellEnd::ChildExited(code) => session.child_exited(code, self.clock.now_instant()),
            ShellEnd::OutputClosed => session.shell_output_ended(),
            ShellEnd::Terminated => {
                session.terminated();
                false
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::shared::application::ports::clock::{Clock, FixedClock};
    use crate::terminal_session::application::shell_end::ShellEnd;
    use crate::terminal_session::application::test_fixtures::TerminalSessionHarness;
    use crate::terminal_session::domain::services::session_deadlines::EXIT_GRACE;
    use std::sync::Arc;
    use std::time::Duration;

    #[test]
    fn should_go_on_for_the_rest_of_the_output_after_the_child_exited() {
        let clock = Arc::new(FixedClock::new(0));
        let mut harness = TerminalSessionHarness::with_clock("{}", clock.clone());

        let goes_on = harness.end(ShellEnd::ChildExited(3));
        clock.advance(EXIT_GRACE - Duration::from_millis(1));
        let grace_over_too_early = harness.session.exit_grace_over(clock.now_instant());
        clock.advance(Duration::from_millis(1));
        let grace_over = harness.session.exit_grace_over(clock.now_instant());

        assert_eq!(
            (goes_on, harness.session.exit_code(), grace_over_too_early, grace_over),
            (true, 3, false, true)
        );
    }

    #[test]
    fn should_stop_the_loop_at_once_when_the_output_had_already_ended() {
        let clock = Arc::new(FixedClock::new(0));
        let mut harness = TerminalSessionHarness::with_clock("{}", clock.clone());
        harness.end(ShellEnd::OutputClosed);

        let goes_on = harness.end(ShellEnd::ChildExited(5));
        clock.advance(EXIT_GRACE);

        assert_eq!(
            (goes_on, harness.session.exit_code(), harness.session.exit_grace_over(clock.now_instant())),
            (false, 5, false)
        );
    }

    #[test]
    fn should_keep_the_loop_going_after_the_output_ended_until_the_child_exits() {
        let mut harness = TerminalSessionHarness::new("{}");

        let goes_on = harness.end(ShellEnd::OutputClosed);

        assert_eq!(
            (goes_on, harness.session.has_seen_pty_eof(), harness.session.exit_code()),
            (true, true, 0)
        );
    }

    #[test]
    fn should_stop_the_loop_with_exit_code_129_when_terminated() {
        let mut harness = TerminalSessionHarness::new("{}");

        let goes_on = harness.end(ShellEnd::Terminated);

        assert_eq!((goes_on, harness.session.exit_code()), (false, 129));
    }
}
