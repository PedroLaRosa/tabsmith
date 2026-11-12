use super::*;
use crate::modules::completion::domain::test_fixtures::{comp, item};
use crate::modules::dropdown::domain::entities::menu::Menu;
use crate::modules::shared::application::ports::clock::FixedClock;
use crate::modules::shared::domain::value_objects::shell::Shell;
use crate::modules::shared::infrastructure::testing::fixture_log::assert_matches_fixture;
use crate::modules::settings::domain::value_objects::settings::Settings;
use crate::modules::specs::domain::value_objects::suggestion_kind::SuggestionKind;
use crate::modules::terminal_session::application::shell_output_feeder::ShellOutputFeeder;
use crate::modules::terminal_session::domain::entities::marker_splitter::MarkerSplitter;
use crate::modules::terminal_session::domain::entities::marker_tracker::MarkerTracker;
use crate::modules::terminal_session::domain::value_objects::cell_tag::CellTag;
use crate::modules::terminal_session::domain::value_objects::deferred_marker::DeferredMarker;
use crate::modules::terminal_session::domain::value_objects::session_id::SessionId;
use crate::modules::terminal_session::domain::value_objects::shell_event::ShellEvent;
use alacritty_terminal::index::{Column, Line};
use alacritty_terminal::vte::ansi::{Color, NamedColor};
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::path::Path;
use std::rc::Rc;
use std::sync::Arc;
use std::time::Instant;

fn pass(screen: &AlacrittyTerminalScreen, bytes: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    screen.pass_through(bytes, &mut out);
    out
}

/// The characters of a model row, without the trailing blanks.
fn row_text(screen: &AlacrittyTerminalScreen, row: i32) -> String {
    let model = screen.model();
    let text: String = (0..model.columns())
        .map(|column| model.grid()[Line(row)][Column(column)].c)
        .collect();
    text.trim_end().to_string()
}

fn foreground(screen: &AlacrittyTerminalScreen, column: usize) -> Color {
    screen.model().grid()[Line(0)][Column(column)].fg
}

fn menu(settings: &Settings) -> Menu {
    let mut menu = Menu::new();
    let items = vec![item("checkout", SuggestionKind::Subcommand, "c")];
    menu.on_completion(comp("c", 1, items), settings, Instant::now());
    menu
}

fn frame_input(settings: &Settings) -> FrameInput<'_> {
    FrameInput {
        settings,
        ghost: None,
        spinner: 0,
        suppress: false,
        scroll_region: false,
        now: Instant::now(),
    }
}

/// A screen showing the prompt `$ c`, with a menu drawn over it (so a clear is cached).
fn screen_with_a_drawn_menu(settings: &Settings) -> (AlacrittyTerminalScreen, Menu) {
    let screen = AlacrittyTerminalScreen::for_tests(40, 12);
    pass(&screen, b"$ c");
    let menu = menu(settings);
    assert!(!screen
        .draw_overlay(&menu, &frame_input(settings))
        .is_empty());
    (screen, menu)
}

#[test]
fn should_append_the_filtered_bytes_and_advance_the_model_on_pass_through() {
    let screen = AlacrittyTerminalScreen::for_tests(20, 5);
    let mut out = b"|".to_vec();

    screen.pass_through(b"ab\x1b[>1ucd", &mut out);

    assert_eq!(
        (out, row_text(&screen, 0)),
        (b"|abcd".to_vec(), "abcd".to_string())
    );
}

#[test]
fn should_hold_an_incomplete_utf8_tail_until_the_next_pass_through() {
    let screen = AlacrittyTerminalScreen::for_tests(20, 5);
    let mut out = Vec::new();
    screen.pass_through(b"a\xc3", &mut out);
    let after_the_first_half = out.clone();

    screen.pass_through(b"\xa9b", &mut out);

    assert_eq!(
        (after_the_first_half, out),
        (b"a".to_vec(), "a\u{e9}b".as_bytes().to_vec())
    );
}

#[test]
fn should_leave_the_model_alone_for_a_window_name() {
    let screen = AlacrittyTerminalScreen::for_tests(20, 5);
    let mut out = Vec::new();

    screen.pass_window_name_through(b"\x1bkvim\x1b\\", &mut out);

    assert_eq!(
        (out, row_text(&screen, 0)),
        (b"\x1bkvim\x1b\\".to_vec(), String::new())
    );
}

#[test]
fn should_end_a_synchronized_update_after_each_pass_through_piece() {
    let screen = AlacrittyTerminalScreen::for_tests(20, 5);

    pass(&screen, b"\x1b[?2026hhello");

    assert_eq!(row_text(&screen, 0), "hello");
}

#[test]
fn should_keep_one_parser_across_pass_through_calls() {
    let split_between_pass_throughs = AlacrittyTerminalScreen::for_tests(20, 5);
    pass(&split_between_pass_throughs, b"\x1b[3");
    pass(&split_between_pass_throughs, b"1mx");
    let split_after_the_startup_write = AlacrittyTerminalScreen::for_tests(20, 5);
    split_after_the_startup_write.advance_model(b"\x1b[3");
    pass(&split_after_the_startup_write, b"1mx");

    let colours = (
        foreground(&split_between_pass_throughs, 0),
        foreground(&split_after_the_startup_write, 0),
    );

    let red = Color::Named(NamedColor::Red);
    assert_eq!(colours, (red, red));
}

#[test]
fn should_report_the_alternate_screen_of_the_model() {
    let screen = AlacrittyTerminalScreen::for_tests(20, 5);
    let before = screen.state().alt_screen();

    pass(&screen, b"\x1b[?1049h");

    assert_eq!((before, screen.state().alt_screen()), (false, true));
}

#[test]
fn should_report_origin_and_insert_mode() {
    let screen = AlacrittyTerminalScreen::for_tests(20, 5);
    let modes = |screen: &AlacrittyTerminalScreen| {
        (screen.state().origin_mode(), screen.state().insert_mode())
    };
    let before = modes(&screen);

    pass(&screen, b"\x1b[?6h\x1b[4h");

    assert_eq!((before, modes(&screen)), ((false, false), (true, true)));
}

#[test]
fn should_report_a_pending_wrap_after_the_last_column() {
    let screen = AlacrittyTerminalScreen::for_tests(10, 5);
    pass(&screen, b"012345678");
    let before = screen.state().pending_wrap();

    pass(&screen, b"9");

    assert_eq!((before, screen.state().pending_wrap()), (false, true));
}

#[test]
fn should_report_a_partial_scroll_region_and_forget_it_on_resize() {
    let screen = AlacrittyTerminalScreen::for_tests(20, 10);
    let before = screen.state().scroll_region();
    pass(&screen, b"\x1b[2;5r");
    let during = screen.state().scroll_region();

    screen.resize(20, 12);

    assert_eq!(
        (before, during, screen.state().scroll_region()),
        (false, true, false)
    );
}

#[test]
fn should_report_an_open_string_until_it_is_terminated() {
    let screen = AlacrittyTerminalScreen::for_tests(20, 5);
    pass(&screen, b"a\x1bP+q");
    let open = screen.state().in_string();

    pass(&screen, b"\x1b\\");

    assert_eq!((open, screen.state().in_string()), (true, false));
}

#[test]
fn should_report_the_size_of_the_model_and_follow_a_resize() {
    let screen = AlacrittyTerminalScreen::for_tests(20, 5);
    let before = screen.state().size();

    screen.resize(30, 8);

    assert_eq!((before, screen.state().size()), ((20, 5), (30, 8)));
}

#[test]
fn should_report_the_cursor_column_and_not_move_it_when_commands_apply() {
    let screen = AlacrittyTerminalScreen::for_tests(4, 5);
    pass(&screen, b"ab");
    let before = screen.cursor_column();

    screen.apply(ScreenCommand::Tag(CellTag::Cycle(1)));
    screen.apply(ScreenCommand::MarkLineStart {
        redrawn_cycle: None,
    });
    let after_the_commands = screen.cursor_column();

    pass(&screen, b"cd");
    assert_eq!(
        (before, after_the_commands, screen.cursor_column()),
        (2, 2, 3)
    );
}

#[test]
fn should_keep_two_thousand_lines_of_scrollback_in_production() {
    let screen =
        AlacrittyTerminalScreen::new(10, 3, OverlayRenderer::new(Theme::dark(), false), false);

    pass(&screen, "x\r\n".repeat(3000).as_bytes());

    assert_eq!(screen.history_size(), 2000);
}

#[test]
fn should_forget_the_cached_clear_on_resize() {
    let settings = Settings::default();
    let (screen, _menu) = screen_with_a_drawn_menu(&settings);

    screen.resize(40, 10);

    assert_eq!(screen.take_clear(), Vec::<u8>::new());
}

#[test]
fn should_return_the_clear_once_and_empty_after() {
    let settings = Settings::default();
    let (screen, _menu) = screen_with_a_drawn_menu(&settings);

    let (first, second) = (screen.take_clear(), screen.take_clear());

    assert_eq!((first.is_empty(), second), (false, Vec::new()));
}

#[test]
fn should_skip_an_identical_frame_and_draw_it_again_after_set_theme_even_with_the_same_theme() {
    let settings = Settings::default();
    let (screen, menu) = screen_with_a_drawn_menu(&settings);
    let input = frame_input(&settings);
    let identical = screen.draw_overlay(&menu, &input);

    screen.set_theme(Theme::dark());
    let after_set_theme = screen.draw_overlay(&menu, &input);

    assert_eq!(
        (identical.is_empty(), after_set_theme.is_empty()),
        (true, false)
    );
}

#[test]
fn should_repaint_the_whole_model_on_full_repaint() {
    let source = AlacrittyTerminalScreen::for_tests(20, 5);
    pass(&source, b"$ echo hi\r\nhi");
    let mirror = AlacrittyTerminalScreen::for_tests(20, 5);
    pass(&mirror, b"garbage\r\nmore garbage everywhere");

    pass(&mirror, &source.full_repaint());

    assert_eq!(
        (row_text(&mirror, 0), row_text(&mirror, 1)),
        ("$ echo hi".to_string(), "hi".to_string())
    );
}

#[test]
fn should_survive_a_panic_in_the_overlay_and_still_hand_out_the_clear() {
    let settings = Settings::default();
    let (screen, _menu) = screen_with_a_drawn_menu(&settings);

    let panicked = catch_unwind(AssertUnwindSafe(|| screen.panic_while_borrowed())).is_err();

    let (first, second) = (screen.take_clear(), screen.take_clear());
    assert_eq!(
        (panicked, first.is_empty(), second),
        (true, false, Vec::new())
    );
}

// --- The marker and extract scenarios: the real screen driven by the feeder, through one `Rig`. ----------------

/// The screen, the splitter, the marker tracker and the feeder that drives them, with the events they emitted.
struct Rig {
    screen: Rc<AlacrittyTerminalScreen>,
    splitter: MarkerSplitter,
    tracker: MarkerTracker,
    feeder: ShellOutputFeeder,
    events: Vec<ShellEvent>,
}

impl Rig {
    fn new(columns: usize, lines: usize, shell: Shell, defer_markers: bool) -> Self {
        let screen = Rc::new(AlacrittyTerminalScreen::for_tests(columns, lines));
        let session_id = SessionId::new(SID.to_string()).expect("a valid id");
        Rig {
            splitter: MarkerSplitter::default(),
            tracker: MarkerTracker::new(session_id, Some(shell), defer_markers),
            feeder: ShellOutputFeeder::new(screen.clone(), Arc::new(FixedClock::new(0))),
            screen,
            events: Vec::new(),
        }
    }

    /// The command line the screen shows for the open cycle, as the session reads it.
    fn edit_buffer(&self) -> Option<EditBuffer> {
        self.tracker
            .edit_buffer_context()
            .and_then(|context| self.screen.edit_buffer(&context))
    }
}

const SID: &str = "0123456789abcdef0123456789abcdef";
const SP: &str = "\x1b]697;StartPrompt\x07";
const EP: &str = "\x1b]697;EndPrompt\x07";

fn nc() -> String {
    format!("\x1b]697;NewCmd={SID}\x07")
}

fn osc(p: &str) -> String {
    format!("\x1b]697;{p}\x1b\\")
}

fn tracker_rig(cols: usize, rows: usize, shell: Shell, defer: bool) -> Rig {
    Rig::new(cols, rows, shell, defer)
}

fn setup(cols: usize, rows: usize, shell: Shell) -> Rig {
    tracker_rig(cols, rows, shell, false)
}

fn feed(rig: &mut Rig, s: &str) -> Vec<u8> {
    let mut out = Vec::new();
    let events = rig.feeder.feed(
        &mut rig.splitter,
        &mut rig.tracker,
        s.as_bytes(),
        &mut out,
    );
    rig.events.extend(events);
    out
}

fn buf(rig: &Rig) -> (String, usize) {
    let e = rig.edit_buffer().expect("at prompt");
    (e.buffer().to_string(), e.cursor())
}

fn prompt(p: &str) -> String {
    format!("{SP}{p}{EP}{}", nc())
}

/// A precmd's markers: unlock, then `extra` markers, then the prompt.
fn precmd(extra: &[&str], p: &str) -> String {
    let markers: String = extra.iter().map(|m| osc(m)).collect();
    format!("{}{markers}{}", osc(&format!("OSCUnlock={SID}")), prompt(p))
}

#[test]
fn markers_stripped_from_output() {
    let mut rig = setup(40, 5, Shell::Zsh);
    let out = feed(
        &mut rig,
        &format!("{}\x1b]0;t\x07{}x", osc("Dir=/tmp"), prompt("% ")),
    );
    assert_eq!(out, b"\x1b]0;t\x07% x");
    assert_eq!(rig.tracker.cwd(), Some(Path::new("/tmp")));
    assert_eq!(buf(&rig), ("x".into(), 1));
}

#[test]
fn window_name_reaches_the_terminal_but_is_not_printed() {
    // screen's `ESC k name ST` (oh-my-zsh titles inside tmux): tmux names the window, the model would print `vim`,
    // drift right and, once PROMPT_SP pads the line to the margin, wrap a row below the real cursor.
    let mut rig = setup(40, 5, Shell::Zsh);
    let shell_output = "ab\x1bkvim\x1b\\cd";

    let out = feed(&mut rig, shell_output);

    assert_eq!(out, shell_output.as_bytes());
    assert_eq!(rig.screen.cursor_column(), 4);
}

#[test]
fn lock_and_foreign_session_ids() {
    let mut rig = setup(40, 5, Shell::Zsh);
    feed(
        &mut rig,
        &format!("{}{}", osc("OSCLock=other"), osc("Dir=/a")),
    );
    assert!(!rig.tracker.osc_lock());
    assert_eq!(rig.tracker.cwd(), Some(Path::new("/a")));
    feed(
        &mut rig,
        &format!(
            "{}{}{}",
            osc(&format!("OSCLock={SID}")),
            osc("Dir=/b"),
            osc("OSCUnlock=other")
        ),
    );
    assert!(rig.tracker.osc_lock());
    assert_eq!(rig.tracker.cwd(), Some(Path::new("/a")));
    feed(
        &mut rig,
        &format!("{}{}", osc(&format!("OSCUnlock={SID}")), osc("Dir=/c")),
    );
    assert!(!rig.tracker.osc_lock());
    assert_eq!(rig.tracker.cwd(), Some(Path::new("/c")));
    // Empty values are ignored.
    feed(&mut rig, &osc("Dir="));
    assert_eq!(rig.tracker.cwd(), Some(Path::new("/c")));
    // NewCmd from a nested session, or without a sid, is ignored; only ours opens a cycle.
    feed(&mut rig, &osc("NewCmd=ffff"));
    assert_eq!(rig.tracker.cycle(), None);
    feed(&mut rig, &osc("NewCmd"));
    assert_eq!(rig.tracker.cycle(), None);
    feed(&mut rig, &nc());
    assert_eq!(rig.tracker.cycle(), Some(1));
}

#[test]
fn bare_newcmd_cannot_fake_a_prompt_while_locked() {
    let mut rig = setup(40, 5, Shell::Bash);
    feed(&mut rig, &prompt("$ "));
    feed(
        &mut rig,
        &format!(
            "cat x\r\n{}{}",
            osc(&format!("OSCLock={SID}")),
            osc("PreExec")
        ),
    );
    assert!(rig.tracker.preexec());
    // `cat` of untrusted output prints a bare NewCmd: the command keeps running.
    feed(&mut rig, "\x1b]697;NewCmd\x07");
    assert!(rig.tracker.preexec() && !rig.tracker.at_prompt());
    assert_eq!(rig.tracker.cycle(), Some(1));
}

#[test]
fn command_cycle_produces_history() {
    let mut rig = setup(40, 6, Shell::Zsh);
    feed(
        &mut rig,
        &format!("{}{}{}", osc("Shell=zsh"), osc("PID=42"), prompt("% ")),
    );
    assert_eq!(rig.events, [ShellEvent::NewCommand(None)]);
    rig.events.clear();
    feed(&mut rig, "ls -l");
    feed(
        &mut rig,
        &format!(
            "\r\n{}{}file\r\n",
            osc(&format!("OSCLock={SID}")),
            osc("PreExec")
        ),
    );
    assert_eq!(rig.events, [ShellEvent::PreExec]);
    assert!(rig.edit_buffer().is_none());
    rig.events.clear();
    feed(
        &mut rig,
        &format!(
            "{}{}{}",
            osc(&format!("OSCUnlock={SID}")),
            osc("ExitCode=2"),
            prompt("% ")
        ),
    );
    let [ShellEvent::NewCommand(Some(h))] = &rig.events[..] else {
        panic!("{:?}", rig.events)
    };
    let h = h.to_primitives();
    assert_eq!(
        (h.command.as_str(), h.exit_code, h.pid, h.shell.as_str()),
        ("ls -l", Some(2), Some(42), "zsh")
    );
    assert_eq!(h.session_id, SID);
    assert_eq!(buf(&rig), (String::new(), 0));
}

#[test]
fn extract_zsh_prompt_and_command() {
    let mut rig = setup(40, 5, Shell::Zsh);
    feed(&mut rig, &format!("{}git st", prompt("~/src % ")));
    assert_eq!(buf(&rig), ("git st".into(), 6));
    // Cursor moved left: byte offset of the cursor.
    feed(&mut rig, "\x1b[2D");
    assert_eq!(buf(&rig), ("git st".into(), 4));
}

#[test]
fn extract_ignores_rprompt() {
    let mut rig = setup(40, 5, Shell::Zsh);
    feed(
        &mut rig,
        &format!("{}\x1b[35G{SP}[rp]{EP}\x1b[3Gls", prompt("% ")),
    );
    assert_eq!(buf(&rig), ("ls".into(), 2));
}

#[test]
fn extract_ps2_multiline() {
    let mut rig = setup(40, 5, Shell::Zsh);
    feed(
        &mut rig,
        &format!("{}echo 'a\r\n{SP}> {EP}b", prompt("% ")),
    );
    assert_eq!(buf(&rig), ("echo 'a\nb".into(), 9));
    // Cursor back on the first line: the PS2 row below still counts.
    feed(&mut rig, "\x1b[A\x1b[4G");
    assert_eq!(buf(&rig), ("echo 'a\nb".into(), 1));
}

#[test]
fn extract_wrapped_long_command() {
    let mut rig = setup(20, 5, Shell::Bash);
    let cmd = "abcdefghijklmnopqrstuvwxyz0123456789";
    feed(&mut rig, &format!("{}{cmd}", prompt("$ ")));
    assert_eq!(buf(&rig), (cmd.into(), cmd.len()));
    // Exactly at the right margin (pending wrap) the cursor is at the end.
    let mut rig = setup(20, 5, Shell::Bash);
    feed(&mut rig, &format!("{}{}", prompt("$ "), &cmd[..18]));
    assert_eq!(buf(&rig), (cmd[..18].into(), 18));
    // Reflow on resize keeps the buffer.
    let mut rig = setup(20, 5, Shell::Bash);
    feed(&mut rig, &format!("{}{cmd}", prompt("$ ")));
    rig.screen.resize(15, 5);
    assert_eq!(buf(&rig), (cmd.into(), cmd.len()));
}

#[test]
fn extract_excludes_completion_listing_below() {
    let mut rig = setup(40, 6, Shell::Zsh);
    feed(
        &mut rig,
        &format!(
            "{}git ch\r\ncheckout  cherry-pick\r\ncherry\x1b[2A\r\x1b[8C",
            prompt("% ")
        ),
    );
    assert_eq!(buf(&rig), ("git ch".into(), 6));
}

#[test]
fn extract_ghost_after_cursor_vs_typed_grey() {
    let mut rig = setup(40, 5, Shell::Zsh);
    feed(
        &mut rig,
        &format!(
            "{}{}git st\x1b[90matus\x1b[0m\x1b[4D",
            osc("ZshAutosuggestionColor=fg=8"),
            prompt("% ")
        ),
    );
    let e = rig.edit_buffer().unwrap();
    assert_eq!(
        (e.buffer(), e.cursor(), e.shell_ghost()),
        ("git st", 6, true)
    );

    let mut rig = setup(40, 5, Shell::Zsh);
    feed(
        &mut rig,
        &format!(
            "{}{}\x1b[38;5;8mgit\x1b[0m st",
            osc("ZshAutosuggestionColor=fg=8"),
            prompt("% ")
        ),
    );
    let e = rig.edit_buffer().unwrap();
    assert_eq!(
        (e.buffer(), e.cursor(), e.shell_ghost()),
        ("git st", 6, false)
    );

    // pwsh: PSReadLine predictions are dim/italic.
    let mut rig = setup(40, 5, Shell::Pwsh);
    feed(
        &mut rig,
        &format!("{}ls\x1b[3m -Force\x1b[0m\x1b[7D", prompt("PS> ")),
    );
    let e = rig.edit_buffer().unwrap();
    assert_eq!(
        (e.buffer(), e.cursor(), e.shell_ghost()),
        ("ls", 2, true)
    );
}

#[test]
fn extract_keeps_blank_skipped_before_cursor() {
    // zsh redraw (Ctrl-L, reset-prompt) moves over the trailing blank with CUF instead of writing it.
    let mut rig = setup(40, 5, Shell::Zsh);
    feed(
        &mut rig,
        &format!("{}git \r{}git\x1b[K\x1b[1C", prompt("% "), prompt("% ")),
    );
    assert_eq!(buf(&rig), ("git ".into(), 4));
    // Blanks after the prompt alone are not buffer text; blanks after the cursor are still trimmed.
    feed(&mut rig, &format!("\r{}\x1b[K\x1b[2C", prompt("% ")));
    assert_eq!(buf(&rig), (String::new(), 0));
    feed(&mut rig, &format!("\r{}ls -l\x1b[2D", prompt("% ")));
    assert_eq!(buf(&rig), ("ls -l".into(), 3));
}

#[test]
fn pwsh_reported_prediction_colour() {
    // A custom InlinePredictionColor (plain grey, neither dim nor italic) is excluded after the cursor.
    let mut rig = setup(40, 5, Shell::Pwsh);
    let grey = "git ch\x1b[90meckout main\x1b[0m\x1b[11D";
    feed(
        &mut rig,
        &format!(
            "{}{}{grey}",
            osc("PwshPredictionColor=[90m"),
            prompt("PS> ")
        ),
    );
    let e = rig.edit_buffer().unwrap();
    assert_eq!(
        (e.buffer(), e.cursor(), e.shell_ghost()),
        ("git ch", 6, true)
    );
    // The default colour, reported: bright white + dim + italic; a plain bright white cell is typed text.
    feed(
        &mut rig,
        &format!(
            "\r\x1b[K{}{}ls\x1b[97;2;3m -Force\x1b[0;97m x\x1b[0m\x1b[9D",
            osc("PwshPredictionColor=[97;2;3m"),
            prompt("PS> ")
        ),
    );
    let e = rig.edit_buffer().unwrap();
    assert_eq!((e.buffer(), e.cursor()), ("ls        x", 2));
}

#[test]
fn no_history_marker_drops_the_command() {
    let mut rig = setup(40, 6, Shell::Fish);
    let run = |rig: &mut Rig, cmd: &str, private: bool| {
        let nh = if private {
            osc("NoHistory")
        } else {
            String::new()
        };
        feed(
            rig,
            &format!(
                "{cmd}\r\n{}{nh}{}out\r\n{}",
                osc(&format!("OSCLock={SID}")),
                osc("PreExec"),
                precmd(&["ExitCode=0"], "> ")
            ),
        );
        std::mem::take(&mut rig.events)
    };
    feed(&mut rig, &prompt("> "));
    rig.events.clear();
    // Sent while locked (fish emits it after OSCLock): still honoured; the menu still resets.
    // r3-nohistory-skips-command-finish: it still ends (the engine resets its generator results).
    assert_eq!(
        run(&mut rig, " set -l TOKEN hunter2", true),
        [ShellEvent::PreExec, ShellEvent::CommandEnded]
    );
    assert!(!rig.tracker.no_history());
    let ev = run(&mut rig, "ls", false);
    assert!(
        matches!(&ev[..], [ShellEvent::PreExec, ShellEvent::NewCommand(Some(h))] if h.command() == "ls"),
        "{ev:?}"
    );
    // A running command printing the marker cannot hide itself or the next command.
    feed(
        &mut rig,
        &format!(
            "make\r\n{}{}",
            osc(&format!("OSCLock={SID}")),
            osc("PreExec")
        ),
    );
    feed(
        &mut rig,
        &format!("{}{}", osc("NoHistory"), precmd(&[], "> ")),
    );
    assert!(
        matches!(&rig.events[..], [ShellEvent::PreExec, ShellEvent::NewCommand(Some(h))] if h.command() == "make"),
        "{:?}",
        rig.events
    );
}

#[test]
fn submitted_line_drops_shell_output_before_preexec() {
    let mut rig = setup(40, 6, Shell::Bash);
    let finish = |rig: &mut Rig, screen: &str, submitted: &str| {
        rig.events.clear();
        feed(rig, &prompt("$ "));
        feed(rig, screen);
        rig.tracker.set_submitted(Some(submitted.into()));
        feed(
            rig,
            &format!(
                "{}{}\r\n{}",
                osc(&format!("OSCLock={SID}")),
                osc("PreExec"),
                precmd(&[], "$ ")
            ),
        );
        let Some(ShellEvent::NewCommand(Some(h))) = rig.events.last() else {
            panic!("{:?}", rig.events)
        };
        assert_eq!(rig.tracker.submitted(), None);
        h.command().to_string()
    };
    // bash/zsh echo the `!!` expansion (and zsh its correction prompt) before PreExec.
    assert_eq!(
        finish(&mut rig, "echo !!\r\necho echo first", "echo !! "),
        "echo !!"
    );
    // A line the shell rewrote on Enter (fish abbreviation, PS2 loop) is taken from the screen.
    assert_eq!(
        finish(&mut rig, "git checkout", "gco"),
        "git checkout"
    );
    assert_eq!(
        finish(
            &mut rig,
            &format!("echo 'a\r\n{SP}> {EP}b'"),
            "echo 'a\nb'"
        ),
        "echo 'a\nb'"
    );
}

#[test]
fn bash_redraw_of_the_prompt_alone_keeps_the_line() {
    // bash-readline-reused-cells-lose-buffer: leaving i-search deletes its prefix (DCH) and reprints only the
    // prompt and the changed start; the shifted rest of the line stays.
    let mut rig = setup(80, 5, Shell::Bash);
    feed(
        &mut rig,
        &format!(
            "{}\r(reverse-i-search)`onel': git log --oneline -1 feature-",
            prompt("$ ")
        ),
    );
    feed(
        &mut rig,
        &format!(
            "\r\x1b[24P{}git log --onel{}",
            prompt("$ "),
            "\x08".repeat(5)
        ),
    );
    assert_eq!(buf(&rig), ("git log --oneline -1 feature-".into(), 9));
    // vi mode with show-mode-in-prompt: the mode string is redrawn and the line moved over with CUF.
    let mut rig = setup(40, 5, Shell::Bash);
    feed(&mut rig, &format!("(ins){}git ch", prompt("$ ")));
    feed(&mut rig, &format!("\r(cmd){}\x1b[5C", prompt("$ ")));
    feed(&mut rig, &format!("\r(ins){}\x1b[6Ce", prompt("$ ")));
    assert_eq!(buf(&rig), ("git che".into(), 7));
    // A two-line prompt: only its last line is redrawn, without StartPrompt; it is still the prompt.
    let mut rig = setup(40, 5, Shell::Bash);
    feed(
        &mut rig,
        &format!("{SP}repo\r\n[I]> {EP}{}git ch", nc()),
    );
    feed(&mut rig, &format!("\r[N]> {EP}{}\x1b[5C", nc()));
    let e = rig.edit_buffer().unwrap();
    assert_eq!(
        (e.buffer(), e.cursor(), e.at_prompt()),
        ("git ch", 5, true)
    );
    // A finished command's cells are not taken over by the next prompt.
    feed(
        &mut rig,
        &format!("\r\n{}{}", osc("PreExec"), prompt("$ ")),
    );
    assert_eq!(buf(&rig), (String::new(), 0));
}

#[test]
fn at_prompt_only_on_the_command_line() {
    let at = |rig: &Rig| rig.edit_buffer().map(|e| (e.buffer().to_string(), e.at_prompt()));
    let mut rig = setup(40, 12, Shell::Zsh);
    // R4-7: fzf Ctrl-T after typed text draws its query row below the line (no wrap, no prompt).
    feed(
        &mut rig,
        &format!("{}cat \r\n  3/3\r\n> read", prompt("% ")),
    );
    assert_eq!(at(&rig).map(|e| e.1), Some(false));
    // fzf exits; the widget redraws the prompt and the line.
    feed(
        &mut rig,
        &format!("\r\x1b[2A\x1b[J{}cat readme.md", prompt("% ")),
    );
    assert_eq!(at(&rig), Some(("cat readme.md".into(), true)));
    // fish-ctrl-r-pager-hijacked: the history pager's search field below the line.
    let mut rig = setup(40, 12, Shell::Fish);
    feed(
        &mut rig,
        &format!(
            "{}git s\r\nsearch: git s\x1b[s\r\n\u{25ba} git status --short\x1b[u",
            prompt("> ")
        ),
    );
    assert_eq!(at(&rig).map(|e| e.1), Some(false));
    // A long line wrapped onto the next rows is still the line; so is a PS2 row.
    let mut rig = setup(20, 8, Shell::Bash);
    feed(
        &mut rig,
        &format!("{}abcdefghijklmnopqrstuvwxyz0123456789", prompt("$ ")),
    );
    assert_eq!(at(&rig).map(|e| e.1), Some(true));
    feed(&mut rig, &format!("\r\n{SP}> {EP}b"));
    assert_eq!(at(&rig).map(|e| e.1), Some(true));
    // pwsh: PSReadLine's ContinuationPrompt (drawn after EndPrompt) starts a line of the command too.
    let mut rig = setup(40, 8, Shell::Pwsh);
    feed(
        &mut rig,
        &format!("{}ls |\r\n>> git ch", prompt("PS> ")),
    );
    assert_eq!(at(&rig), Some(("ls |\ngit ch".into(), true)));
}

#[test]
fn soft_wrap_redrawn_with_crlf_is_still_the_line() {
    // fish redraws a wrapped line after an edit on its first row: up to the margin, then CRLF (no WRAPLINE).
    let at = |rig: &Rig| rig.edit_buffer().map(|e| (e.buffer().to_string(), e.at_prompt()));
    let mut rig = setup(20, 8, Shell::Fish);
    feed(
        &mut rig,
        &format!("{}abcdefghijklmnopqr\r\nstu", prompt("$ ")),
    );
    assert_eq!(at(&rig), Some(("abcdefghijklmnopqrstu".into(), true)));
    let mut rig = setup(20, 8, Shell::Zsh);
    feed(
        &mut rig,
        &format!("{}\r\ngit ch", prompt("xxxxxxxxxxxxxxxxxx% ")),
    );
    assert_eq!(at(&rig), Some(("git ch".into(), true)));
    // A right prompt at the margin does not join an inline TUI's query row below (fzf --layout=reverse).
    let mut rig = setup(20, 8, Shell::Zsh);
    feed(
        &mut rig,
        &format!("{}cat\x1b[18G{SP}[R]{EP}\r\n> rea", prompt("$ ")),
    );
    assert_eq!(at(&rig).map(|e| e.1), Some(false));
}

#[test]
fn prompt_ending_in_a_newline_is_at_the_prompt() {
    // r4-newline-terminated-prompt-no-menu: PS1 ends in `\n`, so the line's row holds no prompt cell.
    let at = |rig: &Rig| rig.edit_buffer().map(|e| (e.buffer().to_string(), e.at_prompt()));
    let mut rig = setup(20, 12, Shell::Zsh);
    // The prompt's own line wraps too.
    feed(
        &mut rig,
        &format!("{SP}~/a/very/long/path/here\r\n{EP}{}git ch", nc()),
    );
    assert_eq!(at(&rig), Some(("git ch".into(), true)));
    // fzf (default layout) from an empty line starts on the line's row: its query row is not the line.
    feed(
        &mut rig,
        &format!(
            "\r\n{}{SP}~\r\n{EP}{}  a.txt\r\n  b.txt\r\n  2/2\r\n> rea",
            osc("PreExec"),
            nc()
        ),
    );
    assert_eq!(at(&rig).map(|e| e.1), Some(false));
    // After typed text it starts below the line.
    feed(
        &mut rig,
        &format!("\r\x1b[4A\x1b[J{SP}~\r\n{EP}{}cat \r\n> rea", nc()),
    );
    assert_eq!(at(&rig).map(|e| e.1), Some(false));
    // Reverse layout (fzf's Ctrl-T default) from an empty line: the query row is the line's own row.
    feed(
        &mut rig,
        &format!(
            "\r\x1b[2A\x1b[J{SP}~\r\n{EP}{}\x1b[s\r\n  2/2\r\n  a.txt\x1b[u  rea",
            nc()
        ),
    );
    assert_eq!(at(&rig).map(|e| e.1), Some(false));
    // fzf exits (clearing its rows); the line is back.
    feed(
        &mut rig,
        &format!("\r\x1b[J\x1b[A{SP}~\r\n{EP}{}cat a.txt", nc()),
    );
    assert_eq!(at(&rig), Some(("cat a.txt".into(), true)));
}

#[test]
fn prompt_drawn_without_markers_is_at_the_prompt() {
    // R4-1: pwsh re-wraps a foreign prompt at ReadLine and sends NewCmd without StartPrompt/EndPrompt.
    let mut rig = setup(40, 8, Shell::Pwsh);
    feed(
        &mut rig,
        &format!(
            "{}PS top> {}{}git ch",
            osc("PreExec"),
            osc(&format!("OSCUnlock={SID}")),
            nc()
        ),
    );
    let e = rig.edit_buffer().unwrap();
    assert_eq!(
        (e.buffer(), e.cursor(), e.at_prompt()),
        ("git ch", 6, true)
    );
    // The same after a command ran.
    feed(
        &mut rig,
        &format!(
            "\r\n{}{}out\r\nPS top> {}{}ls",
            osc(&format!("OSCLock={SID}")),
            osc("PreExec"),
            osc(&format!("OSCUnlock={SID}")),
            nc()
        ),
    );
    let e = rig.edit_buffer().unwrap();
    assert_eq!((e.buffer(), e.at_prompt()), ("ls", true));
}

#[test]
fn transient_prompt_before_preexec_keeps_the_submitted_line() {
    let mut rig = setup(60, 8, Shell::Zsh);
    let run = |rig: &mut Rig, typed: &str, finish: &str| {
        feed(rig, &format!("{}{typed}", prompt("% ")));
        rig.tracker.set_submitted(Some(typed.into()));
        feed(
            rig,
            &format!(
                "{finish}{}{}out\r\n{}",
                osc(&format!("OSCLock={SID}")),
                osc("PreExec"),
                precmd(&[], "% ")
            ),
        );
        let Some(ShellEvent::NewCommand(Some(h))) = std::mem::take(&mut rig.events).pop() else {
            panic!()
        };
        h.command().to_string()
    };
    // r3-transient-redraw-clears-submitted: zle-line-finish re-expands PS1 (NewCmd) between Enter and PreExec.
    let redraw = format!("\r{}echo !!\r\necho echo second\r\n", prompt("% "));
    assert_eq!(run(&mut rig, "echo !!", &redraw), "echo !!");
    // transient-prompt-pollutes-history: a transient prompt drawn without markers lands in the scrape.
    let transient = "\r\x1b[K\u{276f} gti status\r\nzsh: correct 'gti' to 'git' [nyae]? y\r\n";
    assert_eq!(run(&mut rig, "gti status", transient), "gti status");
    assert_eq!(
        run(&mut rig, "echo hi", "\r\x1b[K\u{276f} echo hi\r\n"),
        "echo hi"
    );
    // Only as whole words ending a line: a rewritten line is still read from the screen.
    assert_eq!(
        run(&mut rig, "gco", "\r\x1b[K\u{276f} git checkout\r\n"),
        "\u{276f} git checkout"
    );
    assert_eq!(
        run(&mut rig, "ls", "\r\x1b[K\u{276f} lsd\r\n"),
        "\u{276f} lsd"
    );
}

#[test]
fn nested_shell_prompt_keeps_outer_command_running() {
    let mut rig = setup(40, 8, Shell::Fish);
    let at = |pid: u32, exit: u32| {
        precmd(&[&format!("PID={pid}"), &format!("ExitCode={exit}")], "> ")
    };
    let run = |rig: &mut Rig, cmd: &str, then: String| {
        feed(
            rig,
            &format!(
                "{cmd}\r\n{}{}{then}",
                osc(&format!("OSCLock={SID}")),
                osc("PreExec")
            ),
        );
        std::mem::take(&mut rig.events)
            .into_iter()
            .filter_map(|e| {
                if let ShellEvent::NewCommand(h) = e {
                    Some(h.map(|h| (h.command().to_string(), h.exit_code())))
                } else {
                    None
                }
            })
            .collect::<Vec<_>>()
    };
    feed(&mut rig, &at(100, 0));
    rig.events.clear();
    // fish (100) → bash (200) → `false` → `exit 5` → fish prompt (exit 5).
    assert_eq!(run(&mut rig, "bash", at(200, 0)), [None]);
    assert_eq!(
        run(&mut rig, "false", at(200, 1)),
        [Some(("false".into(), Some(1)))]
    );
    let done = run(&mut rig, "exit 5", at(100, 5));
    assert_eq!(
        done,
        [
            Some(("exit 5".into(), Some(5))),
            Some(("bash".into(), Some(5)))
        ]
    );
    assert!(rig.tracker.pending_is_empty());
    // A prompt redraw of the same shell finishes nothing.
    feed(&mut rig, &format!("\r{}", prompt("> ")));
    assert_eq!(rig.events, [ShellEvent::NewCommand(None)]);
}

#[test]
fn pwsh_continuation_and_prompt_text_stripped() {
    // Default ContinuationPrompt `>> ` before the prompt reports it; the cursor keeps its character.
    let mut rig = setup(40, 6, Shell::Pwsh);
    feed(
        &mut rig,
        &format!("{}if ($true) {{\r\n>> git ch", prompt("PS> ")),
    );
    assert_eq!(buf(&rig), ("if ($true) {\ngit ch".into(), 19));
    feed(&mut rig, "\x1b[2D");
    assert_eq!(buf(&rig), ("if ($true) {\ngit ch".into(), 17));
    // Reported prompts; PSReadLine re-renders the PromptText `> ` (red) on a parse error.
    let mut rig = setup(40, 6, Shell::Pwsh);
    let prompts = osc("PwshPrompts=... \x1f> ");
    feed(
        &mut rig,
        &format!(
            "{prompts}{}echo (2\x1b[3G\x1b[31m> \x1b[0m\x1b[12G",
            prompt("PS> ")
        ),
    );
    assert_eq!(buf(&rig), ("echo (2".into(), 7));
    feed(&mut rig, "\r\n... )");
    assert_eq!(buf(&rig), ("echo (2\n)".into(), 9));
    // Cursor inside a stripped prompt: at its line start.
    feed(&mut rig, "\x1b[2G");
    assert_eq!(buf(&rig), ("echo (2\n)".into(), 8));
    feed(&mut rig, "\x1b[A\x1b[3G");
    assert_eq!(buf(&rig), ("echo (2".into(), 0));
}

#[test]
fn pwsh_list_view_rows_not_in_buffer() {
    // PSReadLine pads the input row to the full width, so its ListView rows can look like a wrap continuation.
    let screen = format!(
        "{}git ch{}<-/4> > cherry-pick\x1b[1;11H",
        prompt("PS> "),
        " ".repeat(10)
    );
    let mut rig = setup(20, 6, Shell::Pwsh);
    feed(&mut rig, &screen);
    assert_ne!(buf(&rig).0, "git ch");
    let mut rig = setup(20, 6, Shell::Pwsh);
    feed(
        &mut rig,
        &format!("{}{screen}", osc("PwshListView=1")),
    );
    assert!(rig.tracker.pwsh_list_view());
    assert_eq!(buf(&rig), ("git ch".into(), 6));
    feed(&mut rig, &osc("PwshListView=0"));
    assert!(!rig.tracker.pwsh_list_view());
    feed(
        &mut rig,
        &format!("{}{}", osc("PwshListView=1"), osc("PreExec")),
    );
    assert!(
        !rig.tracker.pwsh_list_view(),
        "a command (maybe a nested shell) does not inherit it"
    );
}

#[test]
fn extract_prompt_scrolled_by_output() {
    let mut rig = setup(10, 3, Shell::Bash);
    feed(&mut rig, "1\r\n2\r\n3\r\n");
    let cmd = "echo hello world again";
    feed(&mut rig, &format!("{}{cmd}", prompt("$ ")));
    // The prompt row has scrolled into history; tags moved with it.
    assert!(rig.screen.history_size() > 0);
    assert_eq!(buf(&rig), (cmd.into(), cmd.len()));
    // Command output below a finished command does not leak into the next buffer.
    feed(
        &mut rig,
        &format!("\r\n{}out\r\n{}x", osc("PreExec"), prompt("$ ")),
    );
    assert_eq!(buf(&rig), ("x".into(), 1));
}

#[test]
fn windows_deferral_fifo() {
    let mut rig = tracker_rig(40, 5, Shell::Pwsh, true);
    feed(&mut rig, &format!("{SP}PS> {EP}{}", nc()));
    assert_eq!(rig.tracker.cycle(), None);
    assert_eq!(
        rig.tracker.deferred(),
        &[DeferredMarker::EndPrompt, DeferredMarker::NewCmd]
    );
    // A batch with markers keeps deferring.
    feed(&mut rig, &osc("Dir=/x"));
    assert_eq!(rig.tracker.cycle(), None);
    // A marker-free batch applies the queue in order, at the current cursor.
    feed(&mut rig, "\x1b[?25h");
    assert!(rig.tracker.deferred().is_empty());
    assert_eq!(
        (rig.tracker.cycle(), &rig.events[..]),
        (Some(1), &[ShellEvent::NewCommand(None)][..])
    );
    feed(&mut rig, "ls");
    assert_eq!(buf(&rig), ("ls".into(), 2));
}

// --- Characterisation of the tracker at the start of Phase 9: scripted chunks, frozen text fixtures. ----------

/// The hyperlink runs of the model, as ` row<r>:<from>..<to>=<uri>`.
fn tags(screen: &AlacrittyTerminalScreen) -> String {
    use alacritty_terminal::grid::Dimensions;
    use alacritty_terminal::index::{Column, Line};
    let m = screen.model();
    let mut out = String::new();
    for r in 0..m.screen_lines() {
        let mut run: Option<(usize, usize, String)> = None;
        for c in 0..=m.columns() {
            let uri = (c < m.columns())
                .then(|| {
                    m.grid()[Line(r as i32)][Column(c)]
                        .hyperlink()
                        .map(|h| h.uri().to_string())
                })
                .flatten();
            match (&mut run, uri) {
                (Some((_, end, u)), Some(n)) if *u == n => *end = c + 1,
                (_, n) => {
                    if let Some((a, b, u)) = run.take() {
                        out += &format!(" row{r}:{a}..{b}={u}");
                    }
                    run = n.map(|u| (c, c + 1, u));
                }
            }
        }
    }
    out
}

fn describe_event(e: &ShellEvent) -> String {
    match e {
        ShellEvent::NewCommand(None) => "NewCmd(None)".into(),
        ShellEvent::NewCommand(Some(h)) => {
            let p = h.to_primitives();
            format!(
                "NewCmd(command={:?} shell={:?} pid={:?} session_id={:?} cwd={:?} exit_code={:?} hostname={:?} end_time_set={})",
                p.command,
                p.shell,
                p.pid,
                p.session_id,
                p.cwd,
                p.exit_code,
                p.hostname,
                p.end_time.is_some()
            )
        }
        ShellEvent::CommandEnded => "CommandEnded".into(),
        ShellEvent::PreExec => "PreExec".into(),
    }
}

/// Feeds `chunks` one by one and logs, per chunk: the filtered bytes for the terminal, the events, the cycle
/// state, the extract tuple and the hyperlink runs of the model.
fn transcript<S: AsRef<str>>(rig: &mut Rig, chunks: &[S]) -> String {
    let mut log = String::new();
    for (i, chunk) in chunks.iter().enumerate() {
        let chunk = chunk.as_ref();
        let out = feed(rig, chunk);
        let events: Vec<String> = std::mem::take(&mut rig.events)
            .iter()
            .map(describe_event)
            .collect();
        let extract = rig
            .edit_buffer()
            .map(|e| (e.buffer().to_string(), e.cursor(), e.shell_ghost(), e.at_prompt()));
        log += &format!(
            "#{i} in={chunk:?}\n  out={}\n  events={events:?}\n  at_prompt={} preexec={} cycle={:?} cwd={:?}\n  extract={extract:?}\n  tags={}\n",
            out.escape_ascii(),
            rig.tracker.at_prompt(),
            rig.tracker.preexec(),
            rig.tracker.cycle(),
            rig.tracker.cwd(),
            tags(&rig.screen)
        );
    }
    log
}

#[test]
fn should_filter_and_track_a_bash_cycle_exactly_as_at_the_phase_9_start() {
    let long = "x".repeat(40);
    let chunks = [
        format!("{}{}{}", osc("Shell=bash"), osc("PID=7"), osc("Dir=/work")),
        prompt("$ "),
        "ls -la ".to_string(),
        format!("{long}\x1b[2K"),
        "\r\n".to_string(),
        format!(
            "{}{}hello\x1b]0;title\x07 world\r\n",
            osc(&format!("OSCLock={SID}")),
            osc("PreExec")
        ),
        "\x1b]697;NoHist".to_string(),
        "ory\x07".to_string(),
        precmd(&["ExitCode=3"], "$ "),
        "\x1b[?2004h\x1b[>4;1m\x1b[?9001h\x1b[=1;1u\x1b[u\x1b[>c\x1b[1;5r\x1b[r".to_string(),
    ];
    let mut rig = tracker_rig(30, 8, Shell::Bash, false);

    let log = transcript(&mut rig, &chunks);

    assert_matches_fixture("terminal_session/characterisation/tracker_bash.log", &log);
}

#[test]
fn should_defer_windows_markers_in_order_exactly_as_at_the_phase_9_start() {
    let chunks = [
        format!("{SP}PS> {EP}{}", nc()),
        osc("Dir=/x"),
        "\x1b[?25h".to_string(),
        "ls".to_string(),
        format!("\r\n{}", osc("PreExec")),
        format!("{}{}", osc("NoHistory"), "out\r\n"),
        format!("{}{SP}PS> {EP}{}", osc(&format!("OSCUnlock={SID}")), nc()),
        "\x1b[?25h".to_string(),
        "sleep".to_string(),
        format!("\r\n{}", osc("PreExec")),
        format!("{EP}{}{}", nc(), osc("PreExec")),
        "\x1b[?25h".to_string(),
    ];
    let mut rig = tracker_rig(30, 5, Shell::Pwsh, true);

    let log = transcript(&mut rig, &chunks);

    assert_matches_fixture("terminal_session/characterisation/tracker_defer.log", &log);
}

#[test]
fn should_track_zsh_colours_and_a_nested_shell_exactly_as_at_the_phase_9_start() {
    let chunks = [
        osc("Shell=zsh"),
        osc("ZshAutosuggestionColor=fg=8"),
        prompt("% "),
        "ec".to_string(),
        "\x1b[90mho hi\x1b[0m\x1b[6D".to_string(),
        "\r\n".to_string(),
        format!("{}{}", osc(&format!("OSCLock={SID}")), osc("PreExec")),
        format!("{}{}", osc("PID=99"), prompt("inner> ")),
        "exit\r\n".to_string(),
        format!("{}{}", osc(&format!("OSCUnlock={SID}")), prompt("% ")),
    ];
    let mut rig = tracker_rig(40, 6, Shell::Zsh, false);

    let log = transcript(&mut rig, &chunks);

    assert_matches_fixture("terminal_session/characterisation/tracker_zsh.log", &log);
}
