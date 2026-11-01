use super::*;
use crate::settings::domain::value_objects::settings::Settings;
use std::time::{Duration, Instant};
use crate::completion::domain::test_fixtures::{comp, item, settings};
use crate::dropdown::infrastructure::adapters::theme_json_mapper::theme_palette_from_json;
use crate::specs::domain::value_objects::suggestion_kind::SuggestionKind;
use alacritty_terminal::event::VoidListener;
use alacritty_terminal::term::Config;
use alacritty_terminal::vte::ansi::Processor;

struct Size(usize, usize);

impl Dimensions for Size {
    fn total_lines(&self) -> usize {
        self.1
    }
    fn screen_lines(&self) -> usize {
        self.1
    }
    fn columns(&self) -> usize {
        self.0
    }
}

fn model(cols: usize, rows: usize, pieces: &[&[u8]]) -> Model {
    let mut t = Model::new(Config::default(), &Size(cols, rows), VoidListener);
    let mut p: Processor = Processor::new();
    for b in pieces {
        p.advance(&mut t, b);
    }
    t
}

/// Visible grids equal (chars, colours, visible flags, combining chars), plus cursor, pen and cursor mode.
fn assert_same(a: &Model, b: &Model) {
    let invisible = Flags::WRAPLINE | Flags::LEADING_WIDE_CHAR_SPACER;
    let (ga, gb) = (a.grid(), b.grid());
    for y in 0..ga.screen_lines() {
        for x in 0..ga.columns() {
            let (ca, cb) = (
                &ga[Line(y as i32)][Column(x)],
                &gb[Line(y as i32)][Column(x)],
            );
            let key = |c: &Cell| {
                (
                    c.c,
                    c.fg,
                    c.bg,
                    c.flags - invisible,
                    c.zerowidth().map(<[char]>::to_vec),
                )
            };
            assert_eq!(key(ca), key(cb), "cell row {y} col {x}");
        }
    }
    assert_eq!(ga.cursor.point, gb.cursor.point);
    assert_eq!(ga.cursor.input_needs_wrap, gb.cursor.input_needs_wrap);
    let pen = |c: &Cell| (c.fg, c.bg, c.flags);
    assert_eq!(pen(&ga.cursor.template), pen(&gb.cursor.template));
    assert_eq!(
        a.mode().contains(TermMode::SHOW_CURSOR),
        b.mode().contains(TermMode::SHOW_CURSOR)
    );
}

fn differs(a: &Model, b: &Model) -> bool {
    let (ga, gb) = (a.grid(), b.grid());
    (0..ga.screen_lines()).any(|y| {
        (0..ga.columns())
            .any(|x| ga[Line(y as i32)][Column(x)] != gb[Line(y as i32)][Column(x)])
    })
}

/// Visible menu over `items` (typed query "c").
fn menu(items: Vec<Suggestion>, s: &Settings) -> Menu {
    let mut m = Menu::new();
    m.on_completion(comp("c", 1, items), s, Instant::now());
    assert!(m.shown(s));
    m
}

fn subs() -> Vec<Suggestion> {
    let mut v: Vec<Suggestion> = ["checkout", "cherry-pick", "clone", "commit"]
        .iter()
        .map(|n| item(n, SuggestionKind::Subcommand, "c"))
        .collect();
    v[0] = v[0].clone().with_args_hint("<br>".into());
    v[1] = v[1].clone().with_names(vec!["cherry-pick".into(), "cp".into()]);
    v[2] = v[2].clone().with_description(Some("multi\nline 中文 description".into()));
    v
}

fn input<'a>(s: &'a Settings, ghost: Option<&'a str>) -> FrameInput<'a> {
    FrameInput {
        settings: s,
        ghost,
        spinner: 3,
        suppress: false,
        scroll_region: false,
        now: Instant::now(),
    }
}

/// Model A ← output; B ← output + frame + clear → same visible grid (SPEC §10 round-trip invariant).
fn round_trip(
    cols: usize,
    rows: usize,
    output: &[u8],
    m: &Menu,
    inp: &FrameInput,
    truecolor: bool,
) -> String {
    let a = model(cols, rows, &[output]);
    let mut r = OverlayRenderer::new(Theme::dark(), truecolor);
    let frame = r.frame(&a, m, inp);
    assert!(
        frame.starts_with(HEAD.as_bytes()) && frame.ends_with(b"\x1b[?2026l"),
        "{frame:?}"
    );
    assert!(
        differs(&a, &model(cols, rows, &[output, &frame])),
        "the frame draws something"
    );
    let clear = r.take_clear();
    assert_same(&a, &model(cols, rows, &[output, &frame, &clear]));
    String::from_utf8(frame).unwrap()
}

/// Coloured screen; rows 3..=7 have wide chars at cols 3-4 and 23-24 and assorted attributes.
fn screen(cursor: &str) -> Vec<u8> {
    let mut o = String::new();
    o += "\x1b[1;31mbold red\x1b[0m plain \x1b[38;2;1;2;3;48;2;200;100;50mtrue\x1b[0m\r\n";
    o += "\x1b[38;5;196;48;5;22mindexed\x1b[0m \x1b[95;104mbright\x1b[0m\r\n";
    o += "$ git c\x1b[K\r\n";
    o += &format!("abc中{}文zz\r\n", "y".repeat(18));
    o += &format!(
        "\x1b[48;2;10;20;30m  {}\x1b[44m中\x1b[0m{}\x1b[7m文\x1b[0m\r\n",
        "e\u{301}".repeat(17),
        ""
    );
    o += "    \x1b[3mit\x1b[4mul\x1b[9mst\x1b[7minv\x1b[2mdim\x1b[8mhid\x1b[0m\x1b[4:3mcurl\x1b[0m\x1b[58;2;1;2;3;4mulc\x1b[0m\r\n";
    o += "\x1b[44m        blue bg row            \x1b[0m\r\n";
    o += "abc中文中文中文中文中文中文中文中文\r\n";
    o += "last line";
    o += cursor;
    o.into_bytes()
}

#[test]
fn round_trip_below_wide_edges_truecolor() {
    let s = settings(r#"{"autocomplete.width": 20}"#);
    let m = menu(subs(), &s);
    // Cursor one past `$ git c` (row 2, col 8) with a green pen: x0 = 8 - 1 - 4 = 3, span 3..23.
    let out = screen("\x1b[3;9H\x1b[32m");
    let frame = round_trip(40, 12, &out, &m, &input(&s, None), true);
    assert!(
        frame.contains("\x1b[4;4H"),
        "span widened left over the wide char at 3-4: {frame:?}"
    );
    assert!(
        frame.contains("48;2;30;90;199"),
        "selected row in truecolor"
    );
    assert!(frame.contains("<br>"), "args hint when it fits");
    assert!(
        frame.contains("\x1b[3;9H\x1b[0;32m\x1b[?25h\x1b[?2026l"),
        "cursor + pen restore"
    );
    // 256-colour terminals get the same geometry with downgraded theme colours.
    let frame = round_trip(40, 12, &out, &m, &input(&s, None), false);
    assert!(frame.contains("48;5;26") && !frame.contains("48;2;30;90;199"));
}

#[test]
fn round_trip_above_ghost_popout() {
    let s =
        settings(r#"{"autocomplete.width": 20, "autocomplete.alwaysShowDescription": true}"#);
    let mut m = menu(subs(), &s);
    m.handle(crate::dropdown::domain::value_objects::key_action::KeyAction::NavigateDown, &s);
    // Cursor on the bottom row 13, after `$ ec`: menu above on rows 8..=12 (wide chars, colours), x0 = 0;
    // pop-out right of the list (0 + 20 + 30 <= 60).
    let mut out = String::from_utf8(screen("")).unwrap();
    out += "\r\n\r\n\r\n\r\n\r\n$ ec          \x1b[36m[rprompt]\x1b[0m\x1b[14;5H";
    let frame = round_trip(
        60,
        14,
        out.as_bytes(),
        &m,
        &input(&s, Some("ho hello\tworld and more")),
        true,
    );
    assert!(
        frame.contains("\x1b[14;5H\x1b[0m\x1b[90mho hello w\x1b"),
        "ghost clipped at the right prompt: {frame:?}"
    );
    assert!(
        frame.contains("\x1b[9;1H") && !frame.contains("\x1b[8;"),
        "block bottom-aligned to the cursor"
    );
    assert!(
        frame.contains("\x1b[9;21H") && frame.contains("about cherry-pick"),
        "pop-out with the selection"
    );
    assert!(
        !frame.contains("\x1b[13;1H"),
        "no description bar under the pop-out"
    );
}

#[test]
fn round_trip_arg_hint_spinner_and_wide_ghost() {
    let s = Settings::default();
    let mut c = comp("c", 1, vec![]);
    c = c.with_loading(true);
    let mut m = Menu::new();
    m.on_completion(c, &s, Instant::now());
    assert!(m.shown(&s) && !m.intercepting(&s));
    let a = model(40, 10, &[b"$ c"]);
    let mut r = OverlayRenderer::new(Theme::light(), true);
    let f = String::from_utf8(r.frame(&a, &m, &input(&s, None))).unwrap();
    assert!(
        f.contains("path: ") && !f.contains('⠸'),
        "no spinner before 200 ms"
    );
    // Ghost with wide chars clipped at the right edge (col 38 cannot hold half of 文).
    let out = format!("{}$ c", " ".repeat(30));
    round_trip(
        40,
        10,
        out.as_bytes(),
        &m,
        &input(&s, Some("中文中文中文")),
        true,
    );
}

#[test]
fn should_show_the_spinner_only_after_the_menu_has_been_loading_for_more_than_200_ms_at_the_given_instant() {
    let s = Settings::default();
    let t0 = Instant::now();
    let mut m = Menu::new();
    m.on_completion(comp("c", 1, vec![]).with_loading(true), &s, t0);
    let a = model(40, 10, &[b"$ c"]);
    let glyph = SPINNER[3 % SPINNER.len()];
    let drawn_at = |after_ms: u64| {
        let at = FrameInput {
            now: t0 + Duration::from_millis(after_ms),
            ..input(&s, None)
        };
        let mut r = OverlayRenderer::new(Theme::light(), true);
        String::from_utf8(r.frame(&a, &m, &at)).unwrap().contains(glyph)
    };

    let shown = (drawn_at(199), drawn_at(201));

    assert_eq!(shown, (false, true));
}

#[test]
fn dedupe_guards_and_clear() {
    let s = Settings::default();
    let m = menu(subs(), &s);
    let a = model(40, 12, &[b"$ c"]);
    let mut r = OverlayRenderer::new(Theme::dark(), true);
    let f1 = r.frame(&a, &m, &input(&s, None));
    assert!(!f1.is_empty());
    assert!(
        r.frame(&a, &m, &input(&s, None)).is_empty(),
        "identical frame skipped"
    );
    // State-only change: clear + new frame in one write.
    let f2 = r.frame(&a, &m, &input(&s, Some("heckout")));
    assert!(f2.starts_with(HEAD.as_bytes()) && f2.len() > f1.len());
    // Guards: nothing drawn, only the pending clear is emitted.
    let alt = model(40, 12, &[b"$ c\x1b[?1049h"]);
    let clear = r.frame(&alt, &m, &input(&s, None));
    assert!(!clear.is_empty() && r.take_clear().is_empty());
    assert!(r.frame(&alt, &m, &input(&s, None)).is_empty());
    let mut inp = input(&s, None);
    inp.suppress = true;
    assert!(r.frame(&a, &m, &inp).is_empty());
    let wrap = model(10, 5, &[b"0123456789"]);
    assert!(wrap.grid().cursor.input_needs_wrap);
    assert!(
        r.frame(&wrap, &m, &input(&s, Some("x"))).is_empty(),
        "pending wrap"
    );
    assert!(
        r.frame(&model(9, 5, &[b"$ c"]), &m, &input(&s, None))
            .is_empty(),
        "too small"
    );
    let ins = model(40, 12, &[b"$ c\x1b[4h"]);
    assert!(
        r.frame(&ins, &m, &input(&s, None)).is_empty(),
        "insert mode"
    );
    r.frame(&a, &m, &input(&s, None));
    r.drop_clear();
    assert!(r.take_clear().is_empty());
}

#[test]
fn full_repaint_round_trip() {
    let out = [
        screen("").as_slice(),
        b"\x1b[6;1H",
        "x".repeat(40).as_bytes(),
        b"\x1b[35;1m",
    ]
    .concat();
    let a = model(40, 12, &[&out]);
    assert!(a.grid().cursor.input_needs_wrap);
    let mut r = OverlayRenderer::new(Theme::dark(), true);
    let rep = r.full_repaint(&a);
    let b = model(
        40,
        12,
        &[
            b"garbage\r\n\x1b[41mmore garbage everywhere\x1b[0m\r\n\x1b[5;5Hx",
            &rep,
        ],
    );
    assert_same(&a, &b);
    // A repaint never adds to the scrollback (ED 2 scrolls the screen into history on alacritty and others).
    let out = b"1\r\n2\r\n3\r\n4\r\n5";
    let a = model(40, 10, &[out]);
    let rep = r.full_repaint(&a);
    let b = model(40, 10, &[out, &rep, &rep, &rep]);
    assert_eq!(b.grid().history_size(), 0);
    assert_same(&a, &b);
}

#[test]
fn truncation_and_wrap_spinner() {
    assert!(SPINNER.iter().all(|&c| cw(c) == 1));
}

#[test]
fn row_contents() {
    let s = settings(r#"{"autocomplete.width": 20}"#);
    let r = OverlayRenderer::new(Theme::dark(), true);
    let mut it = item("checkout-a-very-long-name", SuggestionKind::Subcommand, "ch");
    it = it.with_args_hint("<branch>".into());
    let (row, _, _) = r.list_row(&it, false, 20);
    let text: String = strip(&row);
    assert_eq!(
        text, "  > checkout-a-ver… ",
        "no room for the hint, title truncated"
    );
    assert!(
        row.contains(&format!("{}ch", r.pen((180, 180, 180), (95, 89, 56), ""))),
        "highlighted query"
    );
    let (row, _, _) = r.list_row(&item("co", SuggestionKind::Option, "c"), true, 20);
    assert_eq!(strip(&row), "  - co              ");
    assert_eq!(width(&strip(&row)), 20);
    let m = menu(subs(), &s);
    let (row, _) = r.desc_row(&m, 20, Some('⠋'));
    assert_eq!(strip(&row), " about checkout   ⠋ ");
    assert_eq!(width(&strip(&row)), 20);
}

#[test]
fn images_fill_the_icon_slot() {
    let s = settings(r#"{"autocomplete.width": 20}"#);
    let m = menu(subs(), &s);
    let base = b"$ git c".to_vec();
    let a = model(40, 12, &[&base]);
    let mut r = OverlayRenderer::new(Theme::dark(), true).with_images(GraphicsSupport::Direct);
    // First frame sends each icon once, then places one image per row, under the text.
    let first = r.frame(&a, &m, &input(&s, None));
    let text = String::from_utf8_lossy(&first).into_owned();
    assert!(text.contains("\x1b_Ga=t,i="), "{text:?}");
    assert_eq!(text.matches("\x1b_Ga=p,").count(), 4, "{text:?}");
    let clear = r.take_clear();
    assert!(String::from_utf8_lossy(&clear).contains(kitty_graphics::CLEAR_PLACEMENTS));
    assert!(
        differs(&a, &model(40, 12, &[&base, &first])),
        "the frame draws something"
    );
    assert_same(&a, &model(40, 12, &[&base, &first, &clear]));
    // Next frame: the images are already in the terminal, so only placements go out again.
    let second = r.frame(&a, &m, &input(&s, Some("x")));
    let text = String::from_utf8_lossy(&second).into_owned();
    assert!(!text.contains("a=t,"), "{text:?}");
    assert_eq!(text.matches("\x1b_Ga=p,").count(), 4, "{text:?}");
}

#[test]
fn tmux_images_are_placeholders() {
    let s = settings(r#"{"autocomplete.width": 20}"#);
    let m = menu(subs(), &s);
    let base = b"$ git c".to_vec();
    let a = model(40, 12, &[&base]);
    let mut r = OverlayRenderer::new(Theme::dark(), true).with_images(GraphicsSupport::Tmux);
    // Images go through tmux's passthrough once; each slot is two placeholder cells, no placement.
    let first = r.frame(&a, &m, &input(&s, None));
    let text = String::from_utf8_lossy(&first).into_owned();
    assert!(
        text.contains("\x1bPtmux;\x1b\x1b_Ga=T,U=1,c=2,r=1,i="),
        "{text:?}"
    );
    assert!(!text.contains("\x1b_Ga=p,"), "{text:?}");
    assert_eq!(text.matches('\u{10eeee}').count(), 8, "{text:?}");
    let clear = r.take_clear();
    assert!(!String::from_utf8_lossy(&clear).contains(kitty_graphics::CLEAR_PLACEMENTS));
    assert_same(&a, &model(40, 12, &[&base, &first, &clear]));
    let second = r.frame(&a, &m, &input(&s, Some("x")));
    assert!(!String::from_utf8_lossy(&second).contains("Ptmux"));
}

/// Text of a styled row (SGR removed).
fn strip(s: &str) -> String {
    let mut out = String::new();
    let mut esc = false;
    for c in s.chars() {
        match (esc, c) {
            (false, '\x1b') => esc = true,
            (true, 'm') => esc = false,
            (true, _) => {}
            (false, c) => out.push(c),
        }
    }
    out
}

/// 64-bit FNV-1a: a stable hash that needs no crate and does not change between Rust versions.
fn fnv1a(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf2_9ce4_8422_2325, |h, b| {
        (h ^ u64::from(*b)).wrapping_mul(0x0000_0100_0000_01b3)
    })
}

/// Draws `frames` one after the other and hashes every frame, then every clear (taken after each frame).
fn overlay_hashes(r: &mut OverlayRenderer, a: &Model, m: &Menu, frames: &[FrameInput]) -> (u64, u64) {
    let (mut drawn, mut cleared) = (Vec::new(), Vec::new());
    for inp in frames {
        drawn.extend(r.frame(a, m, inp));
        cleared.extend(r.take_clear());
    }
    (fnv1a(&drawn), fnv1a(&cleared))
}

#[test]
fn should_draw_the_overlay_bytes_it_drew_at_the_phase_8_start() {
    // (scenario, hash of every frame, hash of every clear), generated once at the phase8-start tag. A refactor
    // may change how a scenario is built, never a value. No scenario depends on time (nothing is loading).
    const EXPECTED: &[(&str, u64, u64)] = &[
        ("below", 0xef119818acef5714, 0x3fd0e0c7d118ca9c),
        ("light_256", 0xbe62837ba0bae9f2, 0x3fd0e0c7d118ca9c),
        ("scrolled", 0x5b2dbcd97d614206, 0xb5974c8ab0e1a615),
        ("above_ghost_popout", 0xf2039310de2aeed9, 0x79cfb3d71e636be6),
        ("arg_hint_wide_ghost", 0x19087c0c1de42343, 0xc4bf1afcc5ae6c08),
        ("custom_base16", 0x3c68886e2010eab3, 0x3fd0e0c7d118ca9c),
        ("icons_direct", 0x63e8d0517291ef1c, 0x49c6d5dfc1ea8156),
        ("icons_tmux", 0x66355913b17a2222, 0x22e0c44f71632e9e),
        ("long_description_bar", 0x09e5bda8de7ac765, 0xe0ae15a704c54ab1),
        ("long_description_popout", 0xe79fca34ee4c38df, 0x55f9d31a52e19d65),
    ];
    let s = settings(r#"{"autocomplete.width": 20}"#);
    let below = screen("\x1b[3;9H\x1b[32m");
    // A custom base16 theme with one distinct colour per role the overlay draws (dark and light give the
    // list and description roles the same colour, so only this one tells them apart).
    let base16 = serde_json::json!({
        "shade0": "#101010", "shade1": "#202020", "shade2": "#303030", "shade5": "#a0a0a0",
        "shade6": "#b0b0b0", "shade7": "#c0c0c0", "accent6": "#123456"
    });
    let custom = || theme_palette_from_json(&base16).theme();
    let mut got = Vec::new();

    // Below the cursor, wide characters at the edges of the span, truecolor, first row selected.
    let (a, m) = (model(40, 12, &[&below]), menu(subs(), &s));
    let mut r = OverlayRenderer::new(Theme::dark(), true);
    got.push(("below", overlay_hashes(&mut r, &a, &m, &[input(&s, None)])));

    // The light theme on a 256-colour terminal.
    let mut r = OverlayRenderer::new(Theme::light(), false);
    got.push((
        "light_256",
        overlay_hashes(&mut r, &a, &m, &[input(&s, None)]),
    ));

    // Scrolled: two rows high, third row selected, whose description has a newline and wide characters.
    let tall = settings(r#"{"autocomplete.width": 20, "autocomplete.height": 2}"#);
    let mut m = menu(subs(), &tall);
    m.handle(crate::dropdown::domain::value_objects::key_action::KeyAction::NavigateDown, &tall);
    m.handle(crate::dropdown::domain::value_objects::key_action::KeyAction::NavigateDown, &tall);
    let mut r = OverlayRenderer::new(Theme::dark(), true);
    got.push((
        "scrolled",
        overlay_hashes(&mut r, &a, &m, &[input(&tall, None)]),
    ));

    // Above the cursor on the bottom row, ghost text clipped at the right prompt, pop-out beside the list.
    let always =
        settings(r#"{"autocomplete.width": 20, "autocomplete.alwaysShowDescription": true}"#);
    let mut m = menu(subs(), &always);
    m.handle(crate::dropdown::domain::value_objects::key_action::KeyAction::NavigateDown, &always);
    let mut out = String::from_utf8(screen("")).unwrap();
    out += "\r\n\r\n\r\n\r\n\r\n$ ec          \x1b[36m[rprompt]\x1b[0m\x1b[14;5H";
    let a = model(60, 14, &[out.as_bytes()]);
    let mut r = OverlayRenderer::new(Theme::dark(), true);
    let ghost = input(&always, Some("ho hello\tworld and more"));
    got.push((
        "above_ghost_popout",
        overlay_hashes(&mut r, &a, &m, &[ghost]),
    ));

    // An argument hint and no rows (custom theme), with a wide ghost clipped at the last column.
    let hint = Settings::default();
    let m = menu(Vec::new(), &hint);
    let a = model(40, 10, &[format!("{}$ c", " ".repeat(30)).as_bytes()]);
    let mut r = OverlayRenderer::new(custom(), true);
    let wide = input(&hint, Some("中文中文中文"));
    got.push((
        "arg_hint_wide_ghost",
        overlay_hashes(&mut r, &a, &m, &[wide]),
    ));

    // The custom theme over the list, second row selected.
    let mut m = menu(subs(), &s);
    m.handle(crate::dropdown::domain::value_objects::key_action::KeyAction::NavigateDown, &s);
    let a = model(40, 12, &[&below]);
    let mut r = OverlayRenderer::new(custom(), true);
    got.push((
        "custom_base16",
        overlay_hashes(&mut r, &a, &m, &[input(&s, None)]),
    ));

    // Icons: sent once, then only placed (direct) or drawn as placeholders (tmux).
    let m = menu(subs(), &s);
    let a = model(40, 12, &[b"$ git c"]);
    for (name, g) in [
        ("icons_direct", GraphicsSupport::Direct),
        ("icons_tmux", GraphicsSupport::Tmux),
    ] {
        let mut r = OverlayRenderer::new(Theme::dark(), true).with_images(g);
        let frames = [input(&s, None), input(&s, Some("x"))];
        got.push((name, overlay_hashes(&mut r, &a, &m, &frames)));
    }

    // A description longer than the bar (truncated with an ellipsis) and than the pop-out (wrapped).
    let long = "a very long description that goes on and on past the end of any row ".repeat(3);
    let rows = || {
        let mut v = subs();
        v[0] = v[0].clone().with_description(Some(long.clone()));
        v
    };
    let m = menu(rows(), &s);
    let a = model(40, 12, &[b"$ git c"]);
    let mut r = OverlayRenderer::new(custom(), true);
    got.push((
        "long_description_bar",
        overlay_hashes(&mut r, &a, &m, &[input(&s, None)]),
    ));
    let m = menu(rows(), &always);
    let a = model(60, 12, &[b"$ git c"]);
    let mut r = OverlayRenderer::new(custom(), true);
    got.push((
        "long_description_popout",
        overlay_hashes(&mut r, &a, &m, &[input(&always, None)]),
    ));

    assert_eq!(got.len(), EXPECTED.len());
    for ((name, (drawn, cleared)), (want_name, want_drawn, want_cleared)) in
        got.iter().zip(EXPECTED)
    {
        assert_eq!(name, want_name);
        assert_eq!(drawn, want_drawn, "{name}: the frame bytes changed");
        assert_eq!(cleared, want_cleared, "{name}: the clear bytes changed");
    }
}
