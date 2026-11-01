use super::*;
use crate::completion::domain::test_fixtures::{comp, settings};
use crate::completion::domain::value_objects::completion_result::CompletionResult;
use crate::terminal_session::domain::entities::insertion_lock::INSERT_LOCK;
use crate::terminal_session::domain::value_objects::completion_query::CompletionQuery;
use crate::terminal_session::domain::value_objects::screen_state::ScreenStateParts;
use crate::terminal_session::domain::services::session_deadlines::REDRAW_GRACE;
use crate::terminal_session::domain::value_objects::shell_event::ShellEvent;
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

#[test]
fn should_not_act_on_an_empty_line_before_the_prompt_redraw_grace_is_over() {
    let start = Instant::now();
    let mut session = session_at_prompt("{}");
    session.refresh(&screen(), line("git ch"), start);
    session.on_shell_event(&ShellEvent::NewCommand(None), start);

    let within_grace = session.refresh(&screen(), line(""), start + Duration::from_millis(10));
    let requests_within_grace = session.request_id();
    let after_grace = session.refresh(&screen(), line(""), start + REDRAW_GRACE);

    assert_eq!(
        (within_grace, requests_within_grace, after_grace, session.request_id()),
        (None, 1, None, 2)
    );
}

#[test]
fn should_keep_the_menu_and_the_unchanged_line_over_a_prompt_redraw() {
    let start = Instant::now();
    let mut session = session_at_prompt("{}");
    session.refresh(&screen(), line("git ch"), start);

    let polls = session.on_shell_event(&ShellEvent::NewCommand(None), start);
    let again = session.refresh(&screen(), line("git ch"), start);

    assert_eq!((polls, again, session.request_id()), (true, None, 1));
}

#[test]
fn should_forget_what_was_sent_when_a_command_ends_so_the_same_line_is_asked_again() {
    let start = Instant::now();
    let mut session = session_at_prompt("{}");
    session.refresh(&screen(), line("git ch"), start);

    let polls = session.on_shell_event(&ShellEvent::CommandEnded, start);
    let again = session.refresh(&screen(), line("git ch"), start);

    assert_eq!((polls, again.map(|query| query.request())), (true, Some(2)));
}

#[test]
fn should_not_poll_the_files_for_a_command_that_starts() {
    let mut session = session_at_prompt("{}");

    let polls = session.on_shell_event(&ShellEvent::PreExec, Instant::now());

    assert!(!polls);
}

// llm_ghost_requested_for_commands_without_spec, at the aggregate.
#[test]
fn should_schedule_the_llm_request_for_an_error_reply_with_the_cursor_at_the_end_and_not_mid_word() {
    let start = Instant::now();
    let mut session = session_at_prompt(
        r#"{"inline.source": "mock", "inline.profiles.mock": {"provider": "openai", "model": "m", "base_url": "http://127.0.0.1:9/v1"}}"#,
    );
    session.refresh(&screen(), line("zzzcmd foo"), start);

    session.apply_completion(CompletionResult::error(session.request_id(), "zzzcmd foo".into(), 10), start);
    let at_end = session.llm_due_at();
    session.cancel_llm_schedule();
    session.apply_completion(CompletionResult::error(session.request_id(), "zzzcmd foo".into(), 3), start);

    assert_eq!((at_end.is_some(), session.llm_due_at()), (true, None));
}

#[test]
fn should_drop_a_reply_for_an_older_request() {
    let start = Instant::now();
    let mut session = session_at_prompt("{}");
    session.refresh(&screen(), line("git ch"), start);
    session.refresh(&screen(), line("git che"), start);

    let older = session.apply_completion(comp("git ch", 1, vec![]).with_request_id(1), start);
    let latest = session.apply_completion(comp("git che", 1, vec![]).with_request_id(2), start);

    assert_eq!((older, latest), (false, true));
}

#[test]
fn should_be_drawable_on_a_plain_command_line_and_not_when_a_guard_holds() {
    let mut session = session_at_prompt("{}");
    session.refresh(&screen(), line("git ch"), Instant::now());
    let narrow = ScreenState::plain(9, 24);
    let in_string = ScreenState::new(ScreenStateParts {
        columns: 80,
        lines: 24,
        in_string: true,
        ..ScreenStateParts::default()
    });

    assert_eq!(
        (
            session.drawable(&screen()),
            session.drawable(&narrow),
            session.drawable(&in_string),
            session.suppressed(&narrow)
        ),
        (true, false, false, false)
    );
}

#[test]
fn should_hold_the_overlay_back_over_a_pwsh_list_view_only_for_a_non_empty_line() {
    let mut session = session_at_prompt("{}");
    session.tracker_mut().set_pwsh_list_view(true);
    session.refresh(&screen(), line("git ch"), Instant::now());
    let with_text = session.drawable(&screen());
    session.refresh(&screen(), line(""), Instant::now());
    let empty = session.drawable(&screen());

    assert_eq!((with_text, empty), (false, true));
}

// should_show_the_llm_ghost_text_of_the_current_request_behind_the_local_one, at the aggregate.
#[test]
fn should_draw_the_llm_ghost_text_of_the_current_request_behind_the_local_one() {
    let start = Instant::now();
    let mut session = session_at_prompt("{}");
    session.refresh(&screen(), line("git sta"), start);
    session.expect_llm_reply(session.request_id());
    let request = session.request_id();

    session.accept_llm_reply(request, Some("tus".into()));
    let from_the_current_request = session.ghost_text(false, &screen());
    session.accept_llm_reply(request + 1, Some("x".into()));
    let after_another_request = session.ghost_text(false, &screen());
    session.accept_llm_reply(request, None);
    let after_an_empty_reply = session.ghost_text(false, &screen());
    session.accept_llm_reply(request, Some("tus".into()));
    session.apply_completion(
        comp("git sta", 1, vec![]).with_request_id(request).with_ghost(Some("sh".into())),
        start,
    );
    let with_a_local_ghost_text = session.ghost_text(false, &screen());

    assert_eq!(
        (
            from_the_current_request,
            after_another_request,
            after_an_empty_reply,
            with_a_local_ghost_text
        ),
        (
            Some("tus".to_string()),
            Some("tus".to_string()),
            None,
            Some("sh".to_string())
        )
    );
}
