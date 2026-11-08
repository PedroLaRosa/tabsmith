use crate::modules::shared::application::ports::clock::Clock;
use crate::modules::terminal_session::application::ports::terminal_screen::TerminalScreen;
use crate::modules::terminal_session::application::ports::wrapped_shell::WrappedShell;
use crate::modules::terminal_session::domain::entities::terminal_session::TerminalSession;
use std::rc::Rc;
use std::sync::Arc;

/// The user's terminal was resized (SPEC §7.6).
pub(crate) struct ResizeTerminalUseCase {
    screen: Rc<dyn TerminalScreen>,
    shell: Rc<dyn WrappedShell>,
    clock: Arc<dyn Clock + Send + Sync>,
}

impl ResizeTerminalUseCase {
    pub(crate) fn new(
        screen: Rc<dyn TerminalScreen>,
        shell: Rc<dyn WrappedShell>,
        clock: Arc<dyn Clock + Send + Sync>,
    ) -> Self {
        ResizeTerminalUseCase { screen, shell, clock }
    }

    /// A size of zero, or the one the model already has, is ignored. The shell is resized first (columns first), then
    /// the model, and the quiet spell before the repaint starts last. In passthrough the runner resizes the shell alone.
    pub(crate) fn execute(&self, session: &mut TerminalSession, columns: u16, rows: u16) {
        if !session.resize_wanted(columns, rows, &self.screen.state()) {
            return;
        }
        self.shell.resize(columns, rows);
        self.screen.resize(columns as usize, rows as usize);
        session.resize_applied(self.clock.now_instant());
    }
}

#[cfg(test)]
mod tests {
    use crate::modules::shared::application::ports::clock::{Clock, FixedClock};
    use crate::modules::terminal_session::application::ports::terminal_screen::TerminalScreen;
    use crate::modules::terminal_session::application::ports::wrapped_shell::WrappedShell;
    use crate::modules::terminal_session::application::resize_terminal_use_case::ResizeTerminalUseCase;
    use crate::modules::terminal_session::application::test_fixtures::{subs, TerminalSessionHarness};
    use crate::modules::terminal_session::domain::services::session_deadlines::RESIZE_DEBOUNCE;
    use crate::modules::terminal_session::infrastructure::adapters::alacritty_terminal_screen::AlacrittyTerminalScreen;
    use std::cell::RefCell;
    use std::rc::Rc;
    use std::sync::Arc;
    use std::time::Duration;

    /// A shell that notes the size the model still had when it was resized.
    struct ShellWatchingTheScreen {
        screen: Rc<AlacrittyTerminalScreen>,
        model_sizes_seen: RefCell<Vec<(usize, usize)>>,
    }

    impl WrappedShell for ShellWatchingTheScreen {
        fn write_input(&self, _bytes: Vec<u8>) {}

        fn resize(&self, _columns: u16, _rows: u16) {
            self.model_sizes_seen.borrow_mut().push(self.screen.state().size());
        }
    }

    #[test]
    fn should_resize_the_shell_before_the_screen() {
        let mut harness = TerminalSessionHarness::new("{}");
        let shell = Rc::new(ShellWatchingTheScreen { screen: harness.screen.clone(), model_sizes_seen: RefCell::default() });
        let resize = ResizeTerminalUseCase::new(harness.screen.clone(), shell.clone(), harness.clock.clone());

        resize.execute(&mut harness.session, 100, 30);

        assert_eq!(
            (shell.model_sizes_seen.take(), harness.screen.state().size()),
            (vec![(80, 24)], (100, 30))
        );
    }

    #[test]
    fn should_pass_columns_before_rows_to_the_shell() {
        let mut harness = TerminalSessionHarness::new("{}");

        harness.resize(100, 30);

        assert_eq!(harness.shell.resizes(), vec![(100, 30)]);
    }

    #[test]
    fn should_ignore_a_size_of_zero() {
        let mut harness = TerminalSessionHarness::new("{}");

        harness.resize(0, 30);
        harness.resize(100, 0);
        harness.resize(100, 30);

        assert_eq!(harness.shell.resizes(), vec![(100, 30)]);
    }

    #[test]
    fn should_ignore_the_size_the_model_already_has() {
        let mut harness = TerminalSessionHarness::new("{}");

        harness.resize(80, 24);
        harness.resize(100, 30);
        harness.resize(100, 30);

        assert_eq!(harness.shell.resizes(), vec![(100, 30)]);
    }

    #[test]
    fn should_repaint_after_the_quiet_time() {
        let clock = Arc::new(FixedClock::new(0));
        let mut harness = TerminalSessionHarness::with_clock("{}", clock.clone());
        harness.resize(100, 30);
        clock.advance(RESIZE_DEBOUNCE - Duration::from_millis(1));
        harness.session.settle_resize_if_due(clock.now_instant());
        let repaint_while_quiet = harness.session.take_repaint(&harness.screen.state());

        clock.advance(Duration::from_millis(2));
        harness.session.settle_resize_if_due(clock.now_instant());
        let repaint_when_settled = harness.session.take_repaint(&harness.screen.state());

        assert_eq!((repaint_while_quiet, repaint_when_settled), (false, true));
    }

    #[test]
    fn should_forget_the_cached_clear_when_resized() {
        let mut harness = TerminalSessionHarness::new("{}");
        harness.prompt("git ch");
        harness.reply(subs("ch"), None);
        let frame_was_drawn = !harness.written().is_empty();

        harness.resize(100, 30);

        assert_eq!((frame_was_drawn, harness.screen.take_clear().is_empty()), (true, true));
    }
}
