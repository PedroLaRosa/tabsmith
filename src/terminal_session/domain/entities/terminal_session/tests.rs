use super::*;
use crate::completion::domain::test_fixtures::settings;
use crate::terminal_session::domain::entities::insertion_lock::INSERT_LOCK;
use crate::terminal_session::domain::value_objects::completion_query::CompletionQuery;
use crate::terminal_session::domain::value_objects::screen_state::ScreenStateParts;
use crate::terminal_session::domain::value_objects::shell_marker::ShellMarker;
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

/// A session at a prompt (a command cycle is open), with the settings given as JSON.
fn session_at_prompt(json: &str) -> TerminalSession {
    let id = SessionId::new(SID.into()).unwrap();
    let mut tracker = MarkerTracker::new(id.clone(), Some(Shell::Bash), false);
    tracker.on_marker(ShellMarker::parse(&format!("NewCmd={SID}")), 0, 0);
    TerminalSession::new(TerminalSessionParts {
        id,
        launched_shell: Shell::Bash,
        settings: settings(json),
        tracker,
        terminal_background: None,
        settings_modified: None,
        now: Instant::now(),
    })
}

fn screen() -> ScreenState {
    ScreenState::plain(80, 24)
}

fn line(text: &str) -> Option<EditBuffer> {
    Some(EditBuffer::new(text.into(), text.len(), false, true))
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

#[test]
fn should_ask_for_a_changed_line_with_the_next_request_id() {
    let mut session = session_at_prompt("{}");

    let query = session.refresh(&screen(), line("git ch"), Instant::now());

    assert_eq!(query, Some(CompletionQuery::new(1, "git ch".into(), 6, false)));
}

#[test]
fn should_not_ask_again_for_an_unchanged_line() {
    let mut session = session_at_prompt("{}");
    session.refresh(&screen(), line("git ch"), Instant::now());

    let again = session.refresh(&screen(), line("git ch"), Instant::now());

    assert_eq!((again, session.request_id()), (None, 1));
}

#[test]
fn should_count_a_blank_line_as_a_request_but_ask_nothing_and_reset_the_menu() {
    let mut session = session_at_prompt("{}");
    session.refresh(&screen(), line("git ch"), Instant::now());

    let query = session.refresh(&screen(), line(" "), Instant::now());

    assert_eq!((query, session.request_id(), session.menu().has_items()), (None, 2, false));
}

#[test]
fn should_see_no_command_line_on_the_alternate_screen() {
    let mut session = session_at_prompt("{}");
    let alternate = ScreenState::new(ScreenStateParts {
        alt_screen: true,
        columns: 80,
        lines: 24,
        ..ScreenStateParts::default()
    });

    let query = session.refresh(&alternate, line("vim"), Instant::now());

    assert_eq!((query, session.edit_buffer().cloned()), (None, None));
}

#[test]
fn should_see_no_command_line_when_the_cursor_is_off_it() {
    let mut session = session_at_prompt("{}");
    let below_the_prompt = Some(EditBuffer::new("  git ch".into(), 8, false, false));

    let query = session.refresh(&screen(), below_the_prompt, Instant::now());

    assert_eq!((query, session.edit_buffer().cloned()), (None, None));
}

// slow_echo_keeps_the_insertion_in_the_basis, at the aggregate: no sleeping, the instants are given.
#[test]
fn should_keep_an_insertion_in_the_basis_while_the_shell_echoes_nothing_and_release_it_at_the_cap() {
    let start = Instant::now();
    let mut session = session_at_prompt("{}");
    session.refresh(&screen(), line("git chec"), start);
    session.lock = Some(InsertionLock::new(start, "git checkout".into(), "git chec".into()));

    let held = session.refresh(&screen(), line("git chec"), start + INSERT_LOCK);
    let basis = session.basis();
    let released = session.refresh(&screen(), line("git chec"), start + Duration::from_secs(1));

    assert_eq!(
        (held, basis, session.has_insertion_lock(), released),
        (None, Some("git checkout".to_string()), false, None)
    );
}
