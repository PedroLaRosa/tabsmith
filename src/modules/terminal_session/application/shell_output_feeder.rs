use crate::modules::shared::application::ports::clock::Clock;
use crate::modules::terminal_session::application::ports::terminal_screen::TerminalScreen;
use crate::modules::terminal_session::domain::entities::marker_splitter::MarkerSplitter;
use crate::modules::terminal_session::domain::entities::marker_tracker::MarkerTracker;
use crate::modules::terminal_session::domain::value_objects::marker_effects::MarkerEffects;
use crate::modules::terminal_session::domain::value_objects::marker_reaction::MarkerReaction;
use crate::modules::terminal_session::domain::value_objects::output_piece::OutputPiece;
use crate::modules::terminal_session::domain::value_objects::shell_event::ShellEvent;
use crate::modules::terminal_session::domain::value_objects::shell_marker::ShellMarker;
use std::rc::Rc;
use std::sync::Arc;

/// The batch loop of the shell's output (SPEC 4.5): the pieces of one PTY read are handled in stream order, so a
/// marker's screen commands reach the screen before the bytes that follow it, and the tracker sees the cursor column
/// and the time of its own byte offset.
pub(crate) struct ShellOutputFeeder {
    screen: Rc<dyn TerminalScreen>,
    clock: Arc<dyn Clock + Send + Sync>,
}

impl ShellOutputFeeder {
    pub(crate) fn new(screen: Rc<dyn TerminalScreen>, clock: Arc<dyn Clock + Send + Sync>) -> Self {
        ShellOutputFeeder { screen, clock }
    }

    /// One PTY batch: markers handled at their byte offset, window names appended (filtered) to `out` only,
    /// everything else fed to the screen and appended (filtered) to `out` for the real terminal. The events are for
    /// the caller to dispatch after the whole batch, in the order they were emitted.
    pub(crate) fn feed(
        &self,
        splitter: &mut MarkerSplitter,
        tracker: &mut MarkerTracker,
        batch: &[u8],
        out: &mut Vec<u8>,
    ) -> Vec<ShellEvent> {
        let mut events = Vec::new();
        for piece in splitter.feed(batch) {
            match piece {
                OutputPiece::Bytes(bytes) => self.screen.pass_through(&bytes, out),
                OutputPiece::WindowName(name) => self.screen.pass_window_name_through(&name, out),
                OutputPiece::Marker(payload) => self.handle_marker(tracker, &payload, &mut events),
            }
        }
        // Windows: apply deferred markers once a batch brings no new ones (FIFO).
        let effects = tracker.end_batch(self.screen.cursor_column(), self.clock.now_secs());
        self.apply(effects, &mut events);
        events
    }

    fn handle_marker(
        &self,
        tracker: &mut MarkerTracker,
        payload: &str,
        events: &mut Vec<ShellEvent>,
    ) {
        let marker = ShellMarker::parse(payload);
        let reaction =
            tracker.on_marker(marker, self.screen.cursor_column(), self.clock.now_secs());
        match reaction {
            MarkerReaction::Handled(effects) => self.apply(effects, events),
            MarkerReaction::EditBufferNeeded(effects) => {
                self.apply(effects, events);
                let scraped = tracker
                    .edit_buffer_context()
                    .and_then(|context| self.screen.edit_buffer(&context));
                let effects = tracker.complete_pre_exec(scraped.as_ref(), self.clock.now_secs());
                self.apply(effects, events);
            }
        }
    }

    /// The screen commands reach the screen right away and in order; the events wait for the end of the batch.
    fn apply(&self, effects: MarkerEffects, events: &mut Vec<ShellEvent>) {
        for &command in effects.screen_commands() {
            self.screen.apply(command);
        }
        events.extend(effects.into_events());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::shared::application::ports::clock::Clock;
    use crate::modules::shared::domain::value_objects::shell::Shell;
    use crate::modules::terminal_session::domain::entities::marker_splitter::MarkerSplitter;
    use crate::modules::terminal_session::domain::entities::marker_tracker::MarkerTracker;
    use crate::modules::terminal_session::domain::value_objects::session_id::SessionId;
    use crate::modules::terminal_session::domain::value_objects::shell_event::ShellEvent;
    use crate::modules::terminal_session::infrastructure::adapters::alacritty_terminal_screen::AlacrittyTerminalScreen;
    use alacritty_terminal::index::{Column, Line};
    use std::rc::Rc;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::sync::Arc;
    use std::time::Instant;

    const SID: &str = "0123456789abcdef0123456789abcdef";
    const SP: &str = "\x1b]697;StartPrompt\x07";
    const EP: &str = "\x1b]697;EndPrompt\x07";

    fn nc() -> String {
        format!("\x1b]697;NewCmd={SID}\x07")
    }

    fn osc(payload: &str) -> String {
        format!("\x1b]697;{payload}\x1b\\")
    }

    fn prompt(text: &str) -> String {
        format!("{SP}{text}{EP}{}", nc())
    }

    /// Every read of the wall clock is one second later than the read before.
    struct TickingClock {
        reads: AtomicU64,
    }

    impl Clock for TickingClock {
        fn now_ms(&self) -> u64 {
            (self.reads.fetch_add(1, Ordering::SeqCst) + 1) * 1000
        }

        fn now_instant(&self) -> Instant {
            Instant::now()
        }
    }

    /// The feeder over the real screen, with the splitter and the tracker it is handed on every batch.
    struct Rig {
        screen: Rc<AlacrittyTerminalScreen>,
        feeder: ShellOutputFeeder,
        splitter: MarkerSplitter,
        tracker: MarkerTracker,
    }

    impl Rig {
        fn new(shell: Shell, defer_markers: bool) -> Self {
            let clock = Arc::new(TickingClock {
                reads: AtomicU64::new(0),
            });
            let screen = Rc::new(AlacrittyTerminalScreen::for_tests(30, 6));
            let session_id = SessionId::new(SID.to_string()).unwrap();
            Rig {
                feeder: ShellOutputFeeder::new(screen.clone(), clock),
                screen,
                splitter: MarkerSplitter::default(),
                tracker: MarkerTracker::new(session_id, Some(shell), defer_markers),
            }
        }

        fn feed(&mut self, batch: &str) -> (Vec<u8>, Vec<ShellEvent>) {
            let mut out = Vec::new();
            let events = self.feeder.feed(
                &mut self.splitter,
                &mut self.tracker,
                batch.as_bytes(),
                &mut out,
            );
            (out, events)
        }

        fn tag_of(&self, column: usize) -> Option<String> {
            let model = self.screen.model();
            model.grid()[Line(0)][Column(column)]
                .hyperlink()
                .map(|link| link.uri().to_string())
        }

        fn line_starts_at_column_zero(&self) -> bool {
            self.tracker
                .edit_buffer_context()
                .expect("a cycle is open")
                .line_starts_at_column_zero()
        }
    }

    #[test]
    fn should_apply_the_screen_commands_of_a_marker_before_advancing_the_next_bytes() {
        let mut rig = Rig::new(Shell::Bash, false);

        rig.feed(&format!("{}ab", nc()));

        assert_eq!(
            (rig.tag_of(0), rig.tag_of(1)),
            (
                Some("tabsmith:c1".to_string()),
                Some("tabsmith:c1".to_string())
            )
        );
    }

    #[test]
    fn should_send_a_window_name_to_the_terminal_only() {
        let mut rig = Rig::new(Shell::Zsh, false);
        let shell_output = "ab\x1bkvim\x1b\\cd";

        let (out, _) = rig.feed(shell_output);

        assert_eq!(
            (out, rig.screen.cursor_column()),
            (shell_output.as_bytes().to_vec(), 4)
        );
    }

    #[test]
    fn should_read_the_cursor_column_just_before_each_marker() {
        let mut marker_first = Rig::new(Shell::Bash, false);
        let mut marker_last = Rig::new(Shell::Bash, false);

        marker_first.feed(&format!("{}ab", nc()));
        marker_last.feed(&format!("ab{}", nc()));

        assert_eq!(
            (
                marker_first.line_starts_at_column_zero(),
                marker_last.line_starts_at_column_zero()
            ),
            (true, false)
        );
    }

    #[test]
    fn should_read_the_clock_before_each_marker() {
        let mut rig = Rig::new(Shell::Bash, false);
        // Reads: the three markers of the prompt (1-3), then the end of the batch (4).
        rig.feed(&format!("{}ls", prompt("$ ")));
        // Reads: the PreExec marker (5), the completion of the PreExec (6), then the end of the batch (7).
        rig.feed(&format!("\r\n{}", osc("PreExec")));

        let (_, events) = rig.feed(&prompt("$ "));

        let [ShellEvent::NewCommand(Some(entry))] = &events[..] else {
            panic!("{events:?}")
        };
        assert_eq!(entry.to_primitives().start_time, 6);
    }

    #[test]
    fn should_flush_the_deferred_markers_when_a_batch_has_no_marker() {
        let mut rig = Rig::new(Shell::Pwsh, true);
        let (_, events_of_the_batch_with_markers) = rig.feed(&prompt("PS> "));

        let (_, events) = rig.feed("ab");

        assert_eq!(
            (
                events_of_the_batch_with_markers,
                events,
                rig.tracker.cycle(),
                rig.line_starts_at_column_zero()
            ),
            (vec![], vec![ShellEvent::NewCommand(None)], Some(1), false)
        );
    }

    #[test]
    fn should_apply_the_flush_before_reading_the_edit_buffer_on_pre_exec() {
        let mut rig = Rig::new(Shell::Pwsh, true);
        rig.feed(&prompt("PS> "));
        rig.feed("\x1b[?25h");
        rig.feed("ls");
        // The redrawn prompt's NewCmd is still queued when the PreExec arrives: the flush keeps the typed line.
        rig.feed(&format!("\rPS> {}{}", nc(), osc("PreExec")));
        rig.feed(&format!("{}{}", osc(&format!("OSCUnlock={SID}")), nc()));

        let (_, events) = rig.feed("\x1b[?25h");

        let [ShellEvent::NewCommand(Some(entry))] = &events[..] else {
            panic!("{events:?}")
        };
        assert_eq!(entry.command(), "ls");
    }

    #[test]
    fn should_complete_pre_exec_with_the_edit_buffer_the_screen_returned() {
        let mut rig = Rig::new(Shell::Bash, false);
        rig.feed(&format!("{}ls -l", prompt("$ ")));
        rig.feed(&format!("\r\n{}", osc("PreExec")));

        let (_, events) = rig.feed(&prompt("$ "));

        let [ShellEvent::NewCommand(Some(entry))] = &events[..] else {
            panic!("{events:?}")
        };
        assert_eq!(entry.command(), "ls -l");
    }

    #[test]
    fn should_return_the_events_of_a_batch_in_emission_order() {
        let mut rig = Rig::new(Shell::Bash, false);

        let (_, events) = rig.feed(&format!("{}ls\r\n{}", prompt("$ "), osc("PreExec")));

        assert_eq!(events, [ShellEvent::NewCommand(None), ShellEvent::PreExec]);
    }

    #[test]
    fn should_end_the_batch_even_when_it_has_no_piece() {
        let mut rig = Rig::new(Shell::Pwsh, true);
        rig.feed(&prompt("PS> "));

        let (out, events) = rig.feed("");

        assert_eq!(
            (out, events, rig.tracker.cycle()),
            (vec![], vec![ShellEvent::NewCommand(None)], Some(1))
        );
    }
}
