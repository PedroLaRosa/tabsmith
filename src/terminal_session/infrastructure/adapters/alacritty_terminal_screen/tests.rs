use super::*;
use crate::completion::domain::test_fixtures::{comp, item};
use crate::dropdown::domain::entities::menu::Menu;
use crate::settings::domain::value_objects::settings::Settings;
use crate::specs::domain::value_objects::suggestion_kind::SuggestionKind;
use crate::terminal_session::domain::value_objects::cell_tag::CellTag;
use alacritty_terminal::index::{Column, Line};
use alacritty_terminal::vte::ansi::{Color, NamedColor};
use std::panic::{catch_unwind, AssertUnwindSafe};
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
