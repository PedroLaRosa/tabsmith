use crate::dropdown::domain::entities::menu::Menu;
use crate::dropdown::domain::value_objects::keymap::Keymap;
use crate::settings::domain::value_objects::settings::Settings;
use crate::shared::domain::value_objects::shell::Shell;
use crate::terminal_session::domain::entities::echo_hold::EchoHold;
use crate::terminal_session::domain::entities::ghost_candidates::GhostCandidates;
use crate::terminal_session::domain::entities::insertion_lock::InsertionLock;
use crate::terminal_session::domain::entities::key_decoder::KeyDecoder;
use crate::terminal_session::domain::entities::marker_splitter::MarkerSplitter;
use crate::terminal_session::domain::entities::marker_tracker::MarkerTracker;
use crate::terminal_session::domain::entities::reload_watch::ReloadWatch;
use crate::terminal_session::domain::entities::resize_debounce::ResizeDebounce;
use crate::terminal_session::domain::entities::shell_exit::ShellExit;
use crate::terminal_session::domain::services::session_deadlines;
use crate::terminal_session::domain::value_objects::edit_buffer::EditBuffer;
use crate::terminal_session::domain::value_objects::session_id::SessionId;
use crate::terminal_session::domain::value_objects::terminal_background::TerminalBackground;
use std::time::{Duration, Instant, SystemTime};

/// After a startup probe timeout, a late reply arriving within this window is dropped instead of typed into the shell.
pub(crate) const LATE_PROBE: Duration = Duration::from_secs(3);

/// The wrapper's view of the shell it runs (SPEC §4.3-4.5, §7): the latest completion request, the menu, the ghost
/// texts, what was sent ahead of the shell's echo, the timers and the exit state. It decides; it never reads a clock
/// (every method that needs the time takes it), a file or the screen (`ScreenState` is passed in), and it never calls
/// the engine, the shell or the terminal: the use cases do, with what it returns.
// The fields are pub(crate) until step 9.3i: the legacy `Session` still reads them while its logic moves in.
pub(crate) struct TerminalSession {
    pub(crate) id: SessionId,
    /// Shell of the spawned program (the tracker's `Shell=` wins once known).
    pub(crate) launched_shell: Shell,
    pub(crate) settings: Settings,
    pub(crate) keymap: Keymap,
    pub(crate) menu: Menu,
    /// Shell-integration state of the session (SPEC 5.1).
    pub(crate) tracker: MarkerTracker,
    /// Splits the shell's output into bytes, window names and OSC 697 markers; keeps the cut sequences between reads.
    pub(crate) splitter: MarkerSplitter,
    /// Kept across stdin reads: a bracketed paste split over several reads stays one opaque unit (SPEC §7.2.1).
    pub(crate) decoder: KeyDecoder,
    pub(crate) terminal_background: Option<TerminalBackground>,
    /// The startup probe timed out: drop its late reply from stdin until then.
    pub(crate) late_probe_until: Option<Instant>,
    /// Latest completion request id and the buffer/cursor it was sent for.
    pub(crate) request: u64,
    pub(crate) sent: Option<(String, usize)>,
    pub(crate) changed_at: Instant,
    /// Edit buffer at the last batch.
    pub(crate) edit_buffer: Option<EditBuffer>,
    /// Insertion lock (SPEC §7.3), re-armed up to its cap while the shell has echoed nothing of the insertion.
    pub(crate) lock: Option<InsertionLock>,
    /// What was sent ahead of the shell's echo and the keys held back (SPEC §7.3).
    pub(crate) echo: EchoHold,
    /// Ghost candidates (SPEC §8): local (engine) and LLM, with the LLM request id and its debounce deadline.
    pub(crate) ghost: GhostCandidates,
    /// A prompt redraw is in progress (see `REDRAW_GRACE`).
    pub(crate) redraw_until: Option<Instant>,
    /// The quiet spell after a resize and the repaint it owes (SPEC §7.6).
    pub(crate) resize: ResizeDebounce,
    pub(crate) spinner: usize,
    pub(crate) spinner_at: Option<Instant>,
    /// When the settings and the session context files are looked at, and the modification times last seen.
    pub(crate) reload: ReloadWatch,
    /// The exit code, the end of the pty output and the grace the rest of the output gets.
    pub(crate) exit: ShellExit,
    /// After a panic in a handler: plain pass-through until the child exits.
    pub(crate) passthrough: bool,
}

pub(crate) struct TerminalSessionParts {
    pub(crate) id: SessionId,
    pub(crate) launched_shell: Shell,
    pub(crate) settings: Settings,
    pub(crate) tracker: MarkerTracker,
    pub(crate) terminal_background: Option<TerminalBackground>,
    pub(crate) settings_modified: Option<SystemTime>,
    pub(crate) now: Instant,
}

impl TerminalSession {
    pub(crate) fn new(parts: TerminalSessionParts) -> Self {
        TerminalSession {
            id: parts.id,
            launched_shell: parts.launched_shell,
            keymap: Keymap::from_settings(&parts.settings),
            settings: parts.settings,
            menu: Menu::new(),
            tracker: parts.tracker,
            splitter: MarkerSplitter::default(),
            decoder: KeyDecoder::default(),
            terminal_background: parts.terminal_background,
            late_probe_until: None,
            request: 0,
            sent: None,
            changed_at: parts.now,
            edit_buffer: None,
            lock: None,
            echo: EchoHold::default(),
            ghost: GhostCandidates::default(),
            redraw_until: None,
            resize: ResizeDebounce::default(),
            spinner: 0,
            spinner_at: None,
            reload: ReloadWatch::new(parts.now, parts.settings_modified),
            exit: ShellExit::default(),
            passthrough: false,
        }
    }

    pub(crate) fn id(&self) -> &SessionId {
        &self.id
    }

    /// The shell the tracker saw, else the one the wrapper spawned.
    pub(crate) fn shell(&self) -> Shell {
        self.tracker.shell().unwrap_or(self.launched_shell)
    }

    /// The startup probe got no cursor report by `probe_ended_at`: its late reply is dropped for `LATE_PROBE`.
    pub(crate) fn expect_late_probe_reply(&mut self, probe_ended_at: Instant) {
        self.late_probe_until = Some(probe_ended_at + LATE_PROBE);
    }

    pub(crate) fn late_probe_open(&self, now: Instant) -> bool {
        self.late_probe_until.is_some_and(|t| now < t)
    }

    /// A cursor report is the probe's last reply: the window closes (an expired one stays open until then).
    pub(crate) fn probe_cursor_reported(&mut self) {
        self.late_probe_until = None;
    }

    /// When the session loop must wake up next.
    pub(crate) fn next_deadline(&self) -> Option<Instant> {
        session_deadlines::next_deadline(
            self.passthrough,
            self.exit.deadline(),
            [
                self.resize.deadline(),
                self.lock.as_ref().map(InsertionLock::deadline),
                self.ghost.llm_due_at(),
                self.spinner_at,
                Some(self.reload.deadline()),
                self.redraw_until,
                self.echo.deadline(),
            ],
        )
    }

    pub(crate) fn resize_applied(&mut self, now: Instant) {
        self.resize.resized(now);
    }

    pub(crate) fn child_exited(&mut self, code: i32, now: Instant) -> bool {
        self.exit.child_exited(code, now)
    }

    pub(crate) fn enter_passthrough(&mut self) {
        self.passthrough = true;
    }

    #[cfg(test)]
    pub(crate) fn late_probe_until(&self) -> Option<Instant> {
        self.late_probe_until
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::completion::domain::test_fixtures::settings;
    use crate::terminal_session::domain::services::session_deadlines::{
        EXIT_GRACE, FILE_POLL, RESIZE_DEBOUNCE,
    };

    const SID: &str = "0123456789abcdef0123456789abcdef";

    /// Arms one source of a deadline at the given instant.
    type Arm = fn(&mut TerminalSession, Instant);

    fn session_at(now: Instant) -> TerminalSession {
        let id = SessionId::new(SID.into()).unwrap();
        TerminalSession::new(TerminalSessionParts {
            id: id.clone(),
            launched_shell: Shell::Bash,
            settings: settings("{}"),
            tracker: MarkerTracker::new(id, Some(Shell::Bash), false),
            terminal_background: None,
            settings_modified: None,
            now,
        })
    }

    fn session() -> TerminalSession {
        session_at(Instant::now())
    }

    #[test]
    fn should_prefer_the_shell_the_tracker_saw_over_the_one_the_wrapper_launched() {
        let id = SessionId::new(SID.into()).unwrap();
        let session_with = |tracked: Option<Shell>| {
            TerminalSession::new(TerminalSessionParts {
                id: id.clone(),
                launched_shell: Shell::Zsh,
                settings: settings("{}"),
                tracker: MarkerTracker::new(id.clone(), tracked, false),
                terminal_background: None,
                settings_modified: None,
                now: Instant::now(),
            })
        };

        assert_eq!(
            (session_with(None).shell(), session_with(Some(Shell::Fish)).shell()),
            (Shell::Zsh, Shell::Fish)
        );
    }

    #[test]
    fn should_open_the_late_probe_window_until_a_cursor_report_closes_it() {
        let start = Instant::now();
        let mut session = session();
        session.expect_late_probe_reply(start);

        let inside = session.late_probe_open(start + Duration::from_secs(2));
        let after = session.late_probe_open(start + LATE_PROBE);
        session.probe_cursor_reported();

        assert_eq!((inside, after, session.late_probe_until()), (true, false, None));
    }

    #[test]
    fn should_wake_up_for_the_nearest_deadline_and_only_for_the_exit_in_a_passthrough() {
        let start = Instant::now();
        let mut session = session_at(start);
        session.resize_applied(start);
        let nearest = session.next_deadline();
        session.child_exited(0, start);
        session.enter_passthrough();

        assert_eq!(
            (nearest, session.next_deadline()),
            (Some(start + RESIZE_DEBOUNCE), Some(start + EXIT_GRACE))
        );
    }

    #[test]
    fn should_wake_for_each_source_of_a_deadline_on_its_own() {
        let start = Instant::now();
        let at = start + Duration::from_secs(1);
        let sources: [(&str, Arm, Instant); 9] = [
            ("nothing armed: the file poll", |_, _| {}, start + FILE_POLL),
            ("resize settle", |s, at| s.resize.expire_settle_at(at), at),
            (
                "insertion lock",
                |s, at| {
                    let mut lock = InsertionLock::new(at, "git checkout".into(), "git chec".into());
                    lock.expire_at(at, at + Duration::from_secs(5));
                    s.lock = Some(lock);
                },
                at,
            ),
            ("llm schedule", |s, at| s.ghost.schedule_llm(at), at),
            ("spinner", |s, at| s.spinner_at = Some(at), at),
            ("file poll", |s, at| s.reload.expire_poll_at(at), at),
            (
                "child exit",
                |s, at| {
                    s.exit.child_exited(0, at - EXIT_GRACE);
                },
                at,
            ),
            ("redraw grace", |s, at| s.redraw_until = Some(at), at),
            ("held keys", |s, at| s.echo.expire_hold_at(at), at),
        ];

        let woken: Vec<(&str, Option<Instant>)> = sources
            .iter()
            .map(|(name, arm, _)| {
                let mut session = session_at(start);
                arm(&mut session, at);
                (*name, session.next_deadline())
            })
            .collect();

        let expected: Vec<(&str, Option<Instant>)> =
            sources.iter().map(|(name, _, wake)| (*name, Some(*wake))).collect();
        assert_eq!(woken, expected);
    }
}
