use super::*;
use crate::modules::history::domain::entities::history_entry::HistoryEntry;

const SID: &str = "0123456789abcdef0123456789abcdef";
const NOW: u64 = 1_700_000_000;

fn tracker(shell: Option<Shell>, defer: bool) -> MarkerTracker {
    MarkerTracker::new(SessionId::new(SID.into()).unwrap(), shell, defer)
}

fn handled(reaction: MarkerReaction) -> MarkerEffects {
    match reaction {
        MarkerReaction::Handled(effects) => effects,
        other => panic!("expected Handled, got {other:?}"),
    }
}

fn send(t: &mut MarkerTracker, payload: &str) -> MarkerEffects {
    handled(t.on_marker(ShellMarker::parse(payload), 0, NOW))
}

fn new_cmd(t: &mut MarkerTracker, column: usize) -> MarkerEffects {
    let marker = ShellMarker::parse(&format!("NewCmd={SID}"));
    handled(t.on_marker(marker, column, NOW))
}

/// `PreExec` accepted, the screen read as `scraped`, the command started at `NOW + 1`.
fn start_command(t: &mut MarkerTracker, scraped: &str) -> MarkerEffects {
    let reaction = t.on_marker(ShellMarker::parse("PreExec"), 0, NOW);
    assert!(
        matches!(reaction, MarkerReaction::EditBufferNeeded(_)),
        "PreExec was not accepted: {reaction:?}"
    );
    let buffer = EditBuffer::new(scraped.to_string(), scraped.len(), false, true);
    t.complete_pre_exec(Some(&buffer), NOW + 1)
}

fn finished_commands(effects: &MarkerEffects) -> Vec<&HistoryEntry> {
    effects
        .events()
        .iter()
        .filter_map(|event| match event {
            ShellEvent::NewCommand(Some(entry)) => Some(entry),
            _ => None,
        })
        .collect()
}

fn the_finished_command(effects: &MarkerEffects) -> &HistoryEntry {
    let [ShellEvent::NewCommand(Some(entry))] = effects.events() else {
        panic!("expected one finished command, got {:?}", effects.events())
    };
    entry
}

#[test]
fn should_open_a_cycle_and_mark_the_line_start_on_new_cmd() {
    let mut t = tracker(Some(Shell::Zsh), false);

    let effects = new_cmd(&mut t, 2);

    assert_eq!(
        (
            effects.screen_commands(),
            effects.events(),
            t.cycle(),
            t.at_prompt()
        ),
        (
            &[
                ScreenCommand::Tag(CellTag::Cycle(1)),
                ScreenCommand::MarkLineStart {
                    redrawn_cycle: None
                }
            ][..],
            &[ShellEvent::NewCommand(None)][..],
            Some(1),
            true
        )
    );
}

#[test]
fn should_mark_a_redraw_with_the_cycle_that_was_open() {
    let mut t = tracker(Some(Shell::Zsh), false);
    new_cmd(&mut t, 2);

    let effects = new_cmd(&mut t, 2);

    assert_eq!(
        effects.screen_commands(),
        [
            ScreenCommand::Tag(CellTag::Cycle(2)),
            ScreenCommand::MarkLineStart {
                redrawn_cycle: Some(1)
            }
        ]
    );
}

#[test]
fn should_ignore_a_new_cmd_of_another_session_or_without_one() {
    let mut t = tracker(Some(Shell::Zsh), false);

    let foreign = send(&mut t, "NewCmd=ffff");
    let bare = send(&mut t, "NewCmd");
    let padded = send(&mut t, &format!("NewCmd= {SID}"));

    assert_eq!(
        (foreign, bare, padded, t.cycle()),
        (
            MarkerEffects::default(),
            MarkerEffects::default(),
            MarkerEffects::default(),
            None
        )
    );
}

#[test]
fn should_defer_end_prompt_and_new_cmd_and_apply_them_in_order_when_a_batch_has_no_marker() {
    let mut t = tracker(Some(Shell::Pwsh), true);
    send(&mut t, "EndPrompt");
    send(&mut t, &format!("NewCmd={SID}"));
    let queued = t.deferred().iter().copied().collect::<Vec<_>>();

    let still_marked = t.end_batch(4, NOW);
    let unmarked = t.end_batch(4, NOW);

    assert_eq!(
        (
            queued,
            still_marked,
            unmarked.screen_commands(),
            unmarked.events()
        ),
        (
            vec![DeferredMarker::EndPrompt, DeferredMarker::NewCmd],
            MarkerEffects::default(),
            &[
                ScreenCommand::ClearTag,
                ScreenCommand::Tag(CellTag::Cycle(1)),
                ScreenCommand::MarkLineStart {
                    redrawn_cycle: None
                }
            ][..],
            &[ShellEvent::NewCommand(None)][..]
        )
    );
}

#[test]
fn should_count_an_ignored_marker_as_a_marker_of_the_batch() {
    let mut t = tracker(Some(Shell::Pwsh), true);
    send(&mut t, &format!("NewCmd={SID}"));
    t.end_batch(0, NOW);
    send(&mut t, "DoneSourcing");
    let ignored = t.end_batch(0, NOW);
    send(&mut t, &format!("OSCLock={SID}"));
    let lock = t.end_batch(0, NOW);
    send(&mut t, "EndPrompt");
    let locked_out = t.end_batch(0, NOW);

    let empty = t.end_batch(0, NOW);

    assert_eq!(
        [&ignored, &lock, &locked_out, &empty].map(|effects| effects.events().len()),
        [0, 0, 0, 1]
    );
}

#[test]
fn should_flush_the_deferred_markers_before_asking_for_the_edit_buffer_on_pre_exec() {
    let mut t = tracker(Some(Shell::Pwsh), true);
    send(&mut t, &format!("NewCmd={SID}"));

    let reaction = t.on_marker(ShellMarker::parse("PreExec"), 4, NOW);

    let MarkerReaction::EditBufferNeeded(effects) = reaction else {
        panic!("{reaction:?}")
    };
    let context_is_of_cycle_1 = t.edit_buffer_context().is_some_and(|c| c.cycle() == 1);
    assert_eq!(
        (
            effects.screen_commands(),
            effects.events(),
            context_is_of_cycle_1
        ),
        (
            &[
                ScreenCommand::Tag(CellTag::Cycle(1)),
                ScreenCommand::MarkLineStart {
                    redrawn_cycle: None
                }
            ][..],
            &[ShellEvent::NewCommand(None)][..],
            true
        )
    );
}

#[test]
fn should_ignore_pre_exec_and_no_history_while_a_command_runs() {
    let mut t = tracker(Some(Shell::Bash), false);
    new_cmd(&mut t, 2);
    start_command(&mut t, "ls");

    let pre_exec = t.on_marker(ShellMarker::parse("PreExec"), 0, NOW);
    let no_history = send(&mut t, "NoHistory");

    assert_eq!(
        (pre_exec, no_history, t.no_history()),
        (
            MarkerReaction::Handled(MarkerEffects::default()),
            MarkerEffects::default(),
            false
        )
    );
}

#[test]
fn should_let_only_the_lock_exempt_markers_through_while_locked() {
    let mut t = tracker(Some(Shell::Zsh), false);
    send(&mut t, &format!("OSCLock={SID}"));

    let dropped = [
        send(&mut t, "Dir=/b"),
        send(&mut t, "StartPrompt"),
        send(&mut t, "EndPrompt"),
        send(&mut t, "OSCUnlock=other"),
    ];
    let new_cmd_effects = new_cmd(&mut t, 0);
    send(&mut t, "NoHistory");
    let while_locked = (
        dropped
            .iter()
            .all(|effects| *effects == MarkerEffects::default()),
        t.cwd().is_none(),
        new_cmd_effects.events().len(),
        t.no_history(),
        t.osc_lock(),
    );
    send(&mut t, &format!("OSCUnlock={SID}"));

    assert_eq!(
        (while_locked, t.osc_lock()),
        ((true, true, 1, true, true), false)
    );
}

#[test]
fn should_record_the_scraped_command_with_the_shell_pid_cwd_and_time_it_started() {
    let mut t = tracker(Some(Shell::Zsh), false);
    send(&mut t, "Shell=/bin/zsh");
    send(&mut t, "PID=42");
    send(&mut t, "Dir=/tmp");
    new_cmd(&mut t, 2);
    start_command(&mut t, " ls -l ");
    send(&mut t, "ExitCode=2");

    let effects = new_cmd(&mut t, 2);

    let p = the_finished_command(&effects).to_primitives();
    assert_eq!(
        (
            p.command.as_str(),
            p.shell.as_str(),
            p.pid,
            p.cwd.as_deref(),
            p.start_time,
            p.end_time,
            p.exit_code,
            p.session_id.as_str()
        ),
        (
            "ls -l",
            "/bin/zsh",
            Some(42),
            Some("/tmp"),
            NOW + 1,
            Some(NOW),
            Some(2),
            SID
        )
    );
}

#[test]
fn should_finish_the_innermost_command_first_with_the_exit_code_of_the_outer_prompt() {
    let mut t = tracker(Some(Shell::Fish), false);
    let prompt_of = |t: &mut MarkerTracker, pid: u32, exit: u32| {
        send(t, &format!("PID={pid}"));
        send(t, &format!("ExitCode={exit}"));
        new_cmd(t, 2)
    };
    prompt_of(&mut t, 100, 0);
    start_command(&mut t, "bash");
    prompt_of(&mut t, 200, 0);
    start_command(&mut t, "exit 5");

    let effects = prompt_of(&mut t, 100, 5);

    let finished: Vec<(&str, Option<i32>)> = finished_commands(&effects)
        .iter()
        .map(|entry| (entry.command(), entry.exit_code()))
        .collect();
    assert_eq!(
        (finished, t.pending_is_empty()),
        (vec![("exit 5", Some(5)), ("bash", Some(5))], true)
    );
}

#[test]
fn should_record_a_blank_command_for_no_history_and_end_it_without_recording() {
    let mut t = tracker(Some(Shell::Fish), false);
    new_cmd(&mut t, 2);
    send(&mut t, "NoHistory");
    let flagged = t.no_history();
    start_command(&mut t, " set -l TOKEN hunter2");

    let effects = new_cmd(&mut t, 2);

    assert_eq!(
        (flagged, t.no_history(), effects.events()),
        (true, false, &[ShellEvent::CommandEnded][..])
    );
}

#[test]
fn should_prefer_the_submitted_line_when_the_scrape_ends_with_shell_output_after_it() {
    let mut t = tracker(Some(Shell::Zsh), false);
    new_cmd(&mut t, 2);
    t.set_submitted(Some("echo !!".into()));
    start_command(&mut t, "echo !!\necho echo second");

    let effects = new_cmd(&mut t, 2);

    assert_eq!(
        (the_finished_command(&effects).command(), t.submitted()),
        ("echo !!", None)
    );
}

#[test]
fn should_keep_the_submitted_line_through_a_redraw_prompt_that_finishes_nothing() {
    let mut t = tracker(Some(Shell::Zsh), false);
    new_cmd(&mut t, 2);
    t.set_submitted(Some("echo hi".into()));

    let redraw = new_cmd(&mut t, 2);

    assert_eq!(
        (redraw.events(), t.submitted()),
        (&[ShellEvent::NewCommand(None)][..], Some("echo hi"))
    );
}

#[test]
fn should_replace_a_known_shell_by_none_and_keep_the_raw_name() {
    let mut t = tracker(Some(Shell::Zsh), false);
    send(&mut t, "Shell=sh");
    new_cmd(&mut t, 2);
    start_command(&mut t, "ls");

    let effects = new_cmd(&mut t, 2);

    assert_eq!(
        (
            t.shell(),
            the_finished_command(&effects).to_primitives().shell
        ),
        (None, "sh".to_string())
    );
}

#[test]
fn should_have_no_edit_buffer_context_before_the_first_cycle_or_while_a_command_runs() {
    let mut t = tracker(Some(Shell::Zsh), false);
    let before_the_first_cycle = t.edit_buffer_context().is_none();
    new_cmd(&mut t, 0);
    let at_the_prompt = t
        .edit_buffer_context()
        .is_some_and(|c| c.line_starts_at_column_zero());

    start_command(&mut t, "ls");

    assert_eq!(
        (
            before_the_first_cycle,
            at_the_prompt,
            t.edit_buffer_context().is_none()
        ),
        (true, true, true)
    );
}

#[test]
fn should_reset_the_list_view_when_a_command_starts() {
    let mut t = tracker(Some(Shell::Pwsh), false);
    send(&mut t, "PwshListView=1");
    new_cmd(&mut t, 0);
    let at_the_prompt = t.pwsh_list_view();

    start_command(&mut t, "ls");

    assert_eq!((at_the_prompt, t.pwsh_list_view()), (true, false));
}

#[test]
fn should_clear_the_tag_and_report_pre_exec_when_the_command_starts() {
    let mut t = tracker(Some(Shell::Zsh), false);
    new_cmd(&mut t, 2);
    t.on_marker(ShellMarker::parse("PreExec"), 2, NOW);

    let effects = t.complete_pre_exec(None, NOW);

    assert_eq!(
        (
            effects.screen_commands(),
            effects.events(),
            t.preexec(),
            t.at_prompt()
        ),
        (
            &[ScreenCommand::ClearTag][..],
            &[ShellEvent::PreExec][..],
            true,
            false
        )
    );
}

#[test]
fn should_tag_the_prompt_on_start_prompt() {
    let mut t = tracker(Some(Shell::Zsh), false);

    let effects = send(&mut t, "StartPrompt");

    assert_eq!(
        (effects.screen_commands(), effects.events()),
        (&[ScreenCommand::Tag(CellTag::Prompt)][..], &[][..])
    );
}

#[test]
fn should_end_the_prompt_with_the_open_cycle_tag_or_clear_it_when_none_is_open() {
    let mut t = tracker(Some(Shell::Zsh), false);
    let before_any_cycle = send(&mut t, "EndPrompt");
    new_cmd(&mut t, 2);
    let in_the_open_cycle = send(&mut t, "EndPrompt");
    start_command(&mut t, "ls");

    let while_a_command_runs = send(&mut t, "EndPrompt");

    assert_eq!(
        (
            before_any_cycle.screen_commands(),
            in_the_open_cycle.screen_commands(),
            while_a_command_runs.screen_commands()
        ),
        (
            &[ScreenCommand::ClearTag][..],
            &[ScreenCommand::Tag(CellTag::Cycle(1))][..],
            &[ScreenCommand::ClearTag][..]
        )
    );
}

#[test]
fn should_ignore_a_pre_exec_whose_new_cmd_is_still_queued() {
    let mut t = tracker(Some(Shell::Zsh), true);
    send(&mut t, &format!("NewCmd={SID}"));
    start_command(&mut t, "sleep 5");
    send(&mut t, "EndPrompt");
    send(&mut t, &format!("NewCmd={SID}"));

    let reaction = t.on_marker(ShellMarker::parse("PreExec"), 0, NOW);

    assert_eq!(
        (
            reaction,
            t.preexec(),
            t.deferred().iter().copied().collect::<Vec<_>>()
        ),
        (
            MarkerReaction::Handled(MarkerEffects::default()),
            true,
            vec![DeferredMarker::EndPrompt, DeferredMarker::NewCmd]
        )
    );
}

#[test]
fn should_not_flush_a_queued_new_cmd_for_no_history_while_a_command_runs() {
    let mut t = tracker(Some(Shell::Zsh), true);
    send(&mut t, &format!("NewCmd={SID}"));
    start_command(&mut t, "sleep 5");
    send(&mut t, "EndPrompt");
    send(&mut t, &format!("NewCmd={SID}"));

    let effects = send(&mut t, "NoHistory");

    assert_eq!(
        (
            effects,
            t.no_history(),
            t.deferred().iter().copied().collect::<Vec<_>>()
        ),
        (
            MarkerEffects::default(),
            false,
            vec![DeferredMarker::EndPrompt, DeferredMarker::NewCmd]
        )
    );
}

#[test]
fn should_flush_the_deferred_markers_before_no_history() {
    let mut t = tracker(Some(Shell::Zsh), true);
    send(&mut t, &format!("NewCmd={SID}"));

    let effects = send(&mut t, "NoHistory");

    assert_eq!(
        (
            effects.screen_commands(),
            effects.events(),
            t.no_history(),
            t.cycle()
        ),
        (
            &[
                ScreenCommand::Tag(CellTag::Cycle(1)),
                ScreenCommand::MarkLineStart {
                    redrawn_cycle: None
                }
            ][..],
            &[ShellEvent::NewCommand(None)][..],
            true,
            Some(1)
        )
    );
}

#[test]
fn should_apply_the_deferred_markers_even_while_the_lock_is_on() {
    let mut t = tracker(Some(Shell::Zsh), true);
    send(&mut t, &format!("NewCmd={SID}"));
    send(&mut t, &format!("OSCLock={SID}"));
    send(&mut t, "EndPrompt");
    t.end_batch(0, NOW);

    let unmarked = t.end_batch(0, NOW);

    assert_eq!(
        (unmarked.screen_commands(), unmarked.events(), t.osc_lock()),
        (
            &[
                ScreenCommand::Tag(CellTag::Cycle(1)),
                ScreenCommand::MarkLineStart {
                    redrawn_cycle: None
                }
            ][..],
            &[ShellEvent::NewCommand(None)][..],
            true
        )
    );
}

#[test]
fn should_consume_the_exit_code_when_a_prompt_finishes_nothing() {
    let mut t = tracker(Some(Shell::Zsh), false);
    send(&mut t, "ExitCode=3");
    new_cmd(&mut t, 2);
    start_command(&mut t, "ls");

    let effects = new_cmd(&mut t, 2);

    assert_eq!(the_finished_command(&effects).exit_code(), None);
}

#[test]
fn should_match_a_pending_command_without_pid_when_no_pid_was_reported() {
    let mut t = tracker(Some(Shell::Zsh), false);
    new_cmd(&mut t, 2);
    start_command(&mut t, "ls");

    let effects = new_cmd(&mut t, 2);

    let finished = the_finished_command(&effects);
    assert_eq!((finished.command(), finished.pid()), ("ls", None));
}

#[test]
fn should_take_the_scrape_when_the_submitted_line_is_not_in_it() {
    let mut t = tracker(Some(Shell::Zsh), false);
    new_cmd(&mut t, 2);
    t.set_submitted(Some("ls".into()));
    start_command(&mut t, "git status");
    let submitted_after_the_scrape = t.submitted().map(str::to_string);

    let effects = new_cmd(&mut t, 2);

    assert_eq!(
        (
            the_finished_command(&effects).command(),
            submitted_after_the_scrape
        ),
        ("git status", None)
    );
}

#[test]
fn should_reset_no_history_on_new_cmd() {
    let mut t = tracker(Some(Shell::Zsh), false);
    new_cmd(&mut t, 2);
    send(&mut t, "NoHistory");
    let flagged = t.no_history();

    new_cmd(&mut t, 2);

    assert_eq!((flagged, t.no_history()), (true, false));
}

#[test]
fn should_use_the_default_pwsh_continuation_until_the_prompt_reports_it() {
    let mut t = tracker(Some(Shell::Pwsh), false);
    new_cmd(&mut t, 0);

    let context = t.edit_buffer_context().unwrap();

    assert_eq!(
        (context.pwsh_continuation(), context.pwsh_prompt_text()),
        (">> ", &[][..])
    );
}

#[test]
fn should_overwrite_a_colour_with_none_when_the_new_one_does_not_parse() {
    let mut t = tracker(Some(Shell::Zsh), false);
    send(&mut t, "ZshAutosuggestionColor=fg=8,bold");
    send(&mut t, "FishSuggestionColor=555");
    send(&mut t, "PwshPredictionColor=[97;2;3m");
    new_cmd(&mut t, 0);
    let reported = t.edit_buffer_context().map(|c| {
        (
            c.zsh_color().is_some(),
            c.fish_color().is_some(),
            c.pwsh_color().is_some(),
        )
    });
    send(&mut t, "ZshAutosuggestionColor=bold");
    send(&mut t, "FishSuggestionColor=not a colour");
    send(&mut t, "PwshPredictionColor=[0m");

    let context = t.edit_buffer_context().unwrap();

    assert_eq!(
        (
            reported,
            context.zsh_color(),
            context.fish_color(),
            context.pwsh_color()
        ),
        (Some((true, true, true)), None, None, None)
    );
}

#[test]
fn should_report_the_context_of_the_open_cycle_with_the_reported_pwsh_prompts() {
    let mut t = tracker(Some(Shell::Pwsh), false);
    send(&mut t, "PwshListView=1");
    send(&mut t, "PwshPrompts=... \x1f> \x1f\x1f>>> ");
    new_cmd(&mut t, 0);
    new_cmd(&mut t, 3);

    let context = t.edit_buffer_context().unwrap();

    assert_eq!(
        (
            context.cycle(),
            context.shell(),
            context.line_starts_at_column_zero(),
            context.pwsh_list_view(),
            context.pwsh_continuation(),
            context.pwsh_prompt_text()
        ),
        (
            2,
            Some(Shell::Pwsh),
            false,
            true,
            "... ",
            &["> ".to_string(), ">>> ".to_string()][..]
        )
    );
}
