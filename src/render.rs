//! Themes, layout and frame bytes for the in-terminal overlay: menu, description bar, pop-out, ghost text, and
//! exact restoration of the covered cells from the model (SPEC §7.4–7.7, §8.2). OWNER: ui agent.
use crate::icons::{self, Graphics, Image};
use crate::menu::{max_rows, Menu};
use crate::settings::domain::services::settings_catalog::{AUTOCOMPLETE_THEME, AUTOCOMPLETE_WIDTH};
use crate::settings::domain::value_objects::settings::Settings;
use crate::shared::infrastructure::data_directory_from_env::data_directory_from_env;
use crate::completion::domain::value_objects::suggestion::Suggestion;
use crate::types::Model;
use crate::specs::domain::value_objects::suggestion_kind::SuggestionKind;
use alacritty_terminal::grid::{Dimensions, Row};
use alacritty_terminal::index::{Column, Line};
use alacritty_terminal::term::cell::{Cell, Flags};
use alacritty_terminal::term::TermMode;
use alacritty_terminal::vte::ansi::{Color, NamedColor};
use serde_json::Value;
use std::collections::HashSet;
use std::fmt::Write as _;
use std::ops::Range;
use unicode_width::UnicodeWidthChar;

type Rgb = (u8, u8, u8);

/// Eight theme colours as RGB (SPEC §7.7).
#[derive(Clone, Debug, PartialEq)]
pub struct Theme {
    pub main_bg: (u8, u8, u8),
    pub main_text: (u8, u8, u8),
    pub matching_bg: (u8, u8, u8),
    pub selected_bg: (u8, u8, u8),
    pub selected_text: (u8, u8, u8),
    pub selected_matching_bg: (u8, u8, u8),
    pub description_text: (u8, u8, u8),
    pub description_border: (u8, u8, u8),
}

/// base16 filler for custom themes (`themes.ts:46-83`, dark variant).
const SHADES: [&str; 8] = [
    "#181818", "#282828", "#383838", "#585858", "#b8b8b8", "#d8d8d8", "#e8e8e8", "#f8f8f8",
];
const ACCENTS: [&str; 8] = [
    "#ab4642", "#dc9656", "#f7ca88", "#a1b56c", "#86c1b9", "#7cafc2", "#ba8baf", "#a16946",
];

impl Theme {
    pub fn dark() -> Theme {
        Theme {
            main_bg: (48, 48, 48),
            main_text: (180, 180, 180),
            matching_bg: (95, 89, 56),
            selected_bg: (30, 90, 199),
            selected_text: (253, 253, 253),
            selected_matching_bg: (106, 142, 218),
            description_text: (180, 180, 180),
            description_border: (65, 65, 65),
        }
    }

    pub fn light() -> Theme {
        Theme {
            main_bg: (254, 254, 254),
            main_text: (7, 7, 7),
            matching_bg: (255, 239, 152),
            selected_bg: (41, 105, 218),
            selected_text: (253, 253, 253),
            selected_matching_bg: (106, 142, 218),
            description_text: (7, 7, 7),
            description_border: (199, 199, 199),
        }
    }

    /// From `autocomplete.theme` (dark/light/system/custom file). `terminal_bg` = OSC 11 reply (for `system`).
    pub fn from_settings(settings: &Settings, terminal_bg: Option<(u8, u8, u8)>) -> Theme {
        match settings.string(AUTOCOMPLETE_THEME).unwrap_or_default() {
            "" | "dark" => Theme::dark(),
            "light" => Theme::light(),
            "system" => match terminal_bg {
                Some((r, g, b))
                    if (0.299 * r as f64 + 0.587 * g as f64 + 0.114 * b as f64) / 255.0 >= 0.5 =>
                {
                    Theme::light()
                }
                _ => Theme::dark(),
            },
            name => data_directory_from_env()
                .themes_dir()
                .ok()
                .and_then(|dir| std::fs::read_to_string(dir.join(format!("{name}.json"))).ok())
                .and_then(|s| serde_json::from_str::<Value>(&s).ok())
                .map_or_else(Theme::dark, |v| Theme::from_json(&v)),
        }
    }

    /// Custom theme file (SPEC §7.7): `version:"1.0"` + `theme{}`, else base16 shades/accents. Missing or
    /// unparsable colours come from dark / the dark filler.
    pub fn from_json(v: &Value) -> Theme {
        let col = |p: &str| v.pointer(p).and_then(Value::as_str).and_then(parse_color);
        let d = Theme::dark();
        if v.get("version").and_then(Value::as_str) == Some("1.0")
            && v.get("theme").is_some_and(Value::is_object)
        {
            return Theme {
                main_bg: col("/theme/backgroundColor").unwrap_or(d.main_bg),
                main_text: col("/theme/textColor").unwrap_or(d.main_text),
                matching_bg: col("/theme/matchBackgroundColor").unwrap_or(d.matching_bg),
                selected_bg: col("/theme/selection/backgroundColor").unwrap_or(d.selected_bg),
                selected_text: col("/theme/selection/textColor").unwrap_or(d.selected_text),
                selected_matching_bg: col("/theme/selection/matchBackgroundColor")
                    .unwrap_or(d.selected_matching_bg),
                description_text: col("/theme/description/textColor").unwrap_or(d.description_text),
                description_border: col("/theme/description/borderColor")
                    .unwrap_or(d.description_border),
            };
        }
        let filler = |list: &[&str; 8], i: usize| parse_color(list[i]).unwrap_or_default();
        let shade = |i: usize| col(&format!("/shade{i}")).unwrap_or_else(|| filler(&SHADES, i));
        let accent = |i: usize| col(&format!("/accent{i}")).unwrap_or_else(|| filler(&ACCENTS, i));
        Theme {
            main_bg: shade(0),
            main_text: shade(6),
            matching_bg: shade(1),
            selected_bg: shade(2),
            selected_text: shade(7),
            selected_matching_bg: accent(6),
            description_text: shade(5),
            description_border: shade(1),
        }
    }
}

/// `#rgb`, `#rrggbb`, `#rrggbbaa` (alpha premultiplied against black), `rgb(r, g, b)` with optional spaces.
pub fn parse_color(s: &str) -> Option<(u8, u8, u8)> {
    let s = s.trim();
    if let Some(h) = s.strip_prefix('#') {
        let hex = |i: usize, n: usize| h.get(i..i + n).and_then(|d| u8::from_str_radix(d, 16).ok());
        return match h.len() {
            3 => Some((hex(0, 1)? * 17, hex(1, 1)? * 17, hex(2, 1)? * 17)),
            6 | 8 => {
                let a = if h.len() == 8 { hex(6, 2)? as u32 } else { 255 };
                let m = |c: u8| ((c as u32 * a + 127) / 255) as u8;
                Some((m(hex(0, 2)?), m(hex(2, 2)?), m(hex(4, 2)?)))
            }
            _ => None,
        };
    }
    let mut it = s
        .strip_prefix("rgb(")?
        .strip_suffix(')')?
        .split(',')
        .map(|p| p.trim().parse::<u8>().ok());
    let rgb = (it.next()??, it.next()??, it.next()??);
    it.next().is_none().then_some(rgb)
}

/// Nearest xterm-256 index (6×6×6 cube or 24-step grey ramp).
pub fn xterm256((r, g, b): (u8, u8, u8)) -> u8 {
    const LV: [u8; 6] = [0, 95, 135, 175, 215, 255];
    let near = |c: u8| {
        (0..6)
            .min_by_key(|&i| (LV[i] as i32 - c as i32).abs())
            .unwrap_or(0)
    };
    let (ri, gi, bi) = (near(r), near(g), near(b));
    let grey = ((((r as i32 + g as i32 + b as i32) / 3) - 8 + 5) / 10).clamp(0, 23);
    let gv = (8 + 10 * grey) as u8;
    let dist = |(x, y, z): Rgb| {
        [(x, r), (y, g), (z, b)]
            .iter()
            .map(|&(p, q)| (p as i32 - q as i32).pow(2))
            .sum::<i32>()
    };
    if dist((gv, gv, gv)) < dist((LV[ri], LV[gi], LV[bi])) {
        232 + grey as u8
    } else {
        (16 + 36 * ri + 6 * gi + bi) as u8
    }
}

/// Inputs to a frame besides the model and menu.
pub struct FrameInput<'a> {
    pub settings: &'a Settings,
    /// Ghost text to draw at the cursor (already decided by the wrapper), if any.
    pub ghost: Option<&'a str>,
    /// Spinner frame counter (advanced every 80 ms by the wrapper while loading).
    pub spinner: usize,
    /// Wrapper-level suppression (preexec, resize repaint pending, insertion lock …).
    pub suppress: bool,
    /// Non-full scroll region active (tracked by the wrapper; alacritty keeps it private).
    pub scroll_region: bool,
}

pub struct Renderer {
    theme: Theme,
    truecolor: bool,
    /// Restores every span the last frame covered (computed from the model at draw time).
    clear: Vec<u8>,
    /// Last frame emitted: an identical frame over a still-drawn one is skipped (render key, SPEC §7.5.3).
    /// ponytail: the key is the frame bytes themselves (a few rows per frame, cheap to build and compare).
    last: Vec<u8>,
    /// How row icons are drawn (`icons::detect`); with direct placements the clear removes them too.
    images: Graphics,
    /// Image ids the terminal already holds: each is transmitted once, placements come every frame.
    sent: HashSet<u32>,
}

const SPINNER: [char; 10] = ['⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠧', '⠇', '⠏'];
const POP_W: usize = 30;
const HEAD: &str = "\x1b[?2026h\x1b[?25l";

/// One covered rect row: `s` (SGR + text) is exactly `w` columns at (`row`, `x`); `pad` = box pen for the cells
/// added when the span is widened over half of a wide char.
/// A frame, the clear that undoes it, and the images the terminal does not hold yet (id, transmission).
type Composed = (Vec<u8>, Vec<u8>, Vec<(u32, String)>);

struct Patch {
    row: usize,
    x: usize,
    w: usize,
    s: String,
    pad: String,
    /// Image covering the icon slot, placed at column `x + 1` (list rows only).
    img: Option<Image>,
}

#[derive(Default)]
struct Run {
    s: String,
    w: usize,
}

impl Run {
    fn put(&mut self, pen: &str, text: &str) {
        self.s.push_str(pen);
        self.s.push_str(text);
        self.w += width(text);
    }

    fn pad_to(&mut self, pen: &str, w: usize) {
        if self.w < w {
            self.put(pen, &" ".repeat(w - self.w));
        }
    }
}

impl Renderer {
    /// `truecolor` = `COLORTERM ∈ {truecolor, 24bit}` or `WT_SESSION` set.
    pub fn new(theme: Theme, truecolor: bool) -> Renderer {
        Renderer {
            theme,
            truecolor,
            clear: Vec::new(),
            last: Vec::new(),
            images: Graphics::Off,
            sent: HashSet::new(),
        }
    }

    /// Row icons drawn as kitty graphics images (Ghostty, kitty, also behind tmux); ASCII when `Off`.
    pub fn with_images(mut self, g: Graphics) -> Renderer {
        self.images = g;
        self
    }

    pub fn set_theme(&mut self, theme: Theme) {
        self.theme = theme;
        self.last.clear();
    }

    /// Bytes restoring every cell span the last frame covered; empties the cache (SPEC §7.5.3).
    pub fn take_clear(&mut self) -> Vec<u8> {
        self.last.clear();
        std::mem::take(&mut self.clear)
    }

    /// Forget the cached clear without emitting it (after resize: the terminal reflowed the old frame).
    pub fn drop_clear(&mut self) {
        self.clear.clear();
        self.last.clear();
    }

    /// Frame for the current state (SPEC §7.5.2). Empty when nothing changed or nothing is drawn. Stores the
    /// matching clear for the next `take_clear`. After PTY output call `take_clear` first (the screen then equals
    /// the model); for state-only changes call `frame` alone: a still-drawn frame is cleared in the same bytes,
    /// or nothing is emitted when the new frame is identical.
    pub fn frame(&mut self, model: &Model, menu: &Menu, input: &FrameInput) -> Vec<u8> {
        let Some((frame, clear, transmit)) = self.compose(model, menu, input) else {
            return self.take_clear();
        };
        if frame == self.last && !self.clear.is_empty() {
            return Vec::new();
        }
        let mut out = self.take_clear();
        for (id, bytes) in transmit {
            self.sent.insert(id);
            out.extend_from_slice(bytes.as_bytes());
        }
        out.extend_from_slice(&frame);
        self.clear = clear;
        self.last = frame;
        out
    }

    /// Full repaint of the model (after resize, SPEC §7.6). Row by row with EL 2: ED 2 would push the screen into
    /// the scrollback on many terminals (alacritty, VTE, kitty), one copy per repaint.
    pub fn full_repaint(&mut self, model: &Model) -> Vec<u8> {
        self.drop_clear();
        let grid = model.grid();
        let mut s = String::from(HEAD);
        if self.images == Graphics::Direct {
            s += icons::CLEAR_PLACEMENTS;
        }
        for y in 0..grid.screen_lines() {
            let row = &grid[Line(y as i32)];
            cup(&mut s, y, 0);
            s += "\x1b[0m\x1b[2K"; // EL erases in the current pen (BCE)
            if let Some(last) = (0..grid.columns()).rev().find(|&x| !blank(&row[Column(x)])) {
                restore_cells(&mut s, row, 0..last + 1);
            }
        }
        s += &tail(model);
        s.into_bytes()
    }

    /// (frame, clear), or None when nothing is drawn (guards SPEC §7.5.1, nothing to show).
    fn compose(&self, model: &Model, menu: &Menu, input: &FrameInput) -> Option<Composed> {
        let grid = model.grid();
        let (rows, cols) = (grid.screen_lines(), grid.columns());
        let guarded = TermMode::ALT_SCREEN | TermMode::ORIGIN | TermMode::INSERT;
        if input.suppress
            || input.scroll_region
            || model.mode().intersects(guarded)
            || grid.cursor.input_needs_wrap
            || cols < 10
            || rows < 3
        {
            return None;
        }
        let (cy, cx) = (
            grid.cursor.point.line.0 as usize,
            grid.cursor.point.column.0,
        );
        let mut patches = Vec::new();
        if menu.shown(input.settings) {
            self.menu_patches(menu, input, (rows, cols), (cy, cx), &mut patches);
        }
        if let Some(g) = input.ghost {
            patches.extend(ghost_patch(&grid[Line(cy as i32)], cols, (cy, cx), g));
        }
        if patches.is_empty() {
            return None;
        }
        let (mut frame, mut clear) = (String::from(HEAD), String::from(HEAD));
        if self.images == Graphics::Direct {
            clear += icons::CLEAR_PLACEMENTS;
        }
        // Images the terminal does not hold yet; `frame` marks them sent when it emits them.
        let mut transmit: Vec<(u32, String)> = Vec::new();
        for (k, p) in patches.iter().enumerate() {
            let row = &grid[Line(p.row as i32)];
            // A covered half of a wide char is blanked by the terminal: cover (and restore) the whole char.
            let left = p.x > 0 && row[Column(p.x)].flags.contains(Flags::WIDE_CHAR_SPACER);
            let right =
                p.x + p.w < cols && row[Column(p.x + p.w - 1)].flags.contains(Flags::WIDE_CHAR);
            let xs = p.x - usize::from(left);
            cup(&mut frame, p.row, xs);
            if left {
                frame += &p.pad;
                frame.push(' ');
            }
            frame += &p.s;
            if right {
                frame += &p.pad;
                frame.push(' ');
            }
            if let Some(img) = &p.img {
                if !self.sent.contains(&img.id) && transmit.iter().all(|(id, _)| *id != img.id) {
                    transmit.push((img.id, icons::transmit(img, self.images)));
                }
                // Behind tmux the placeholders in `p.s` show the image.
                if self.images == Graphics::Direct {
                    cup(&mut frame, p.row, p.x + 1);
                    frame += &icons::place(img, k + 1);
                }
            }
            cup(&mut clear, p.row, xs);
            restore_cells(&mut clear, row, xs..p.x + p.w + usize::from(right));
        }
        let tail = tail(model);
        frame += &tail;
        clear += &tail;
        Some((frame.into_bytes(), clear.into_bytes(), transmit))
    }

    fn menu_patches(
        &self,
        menu: &Menu,
        input: &FrameInput,
        size: (usize, usize),
        cur: (usize, usize),
        out: &mut Vec<Patch>,
    ) {
        let s = input.settings;
        let items = menu.items();
        let sel = menu.selected().min(items.len().saturating_sub(1));
        let query = match items.get(sel) {
            Some(it) => it.query(),
            None => menu.last().map_or("", |c| c.search_term()),
        };
        let menu_w = s.f64(AUTOCOMPLETE_WIDTH).max(10.0) as usize;
        let Some(l) = layout(size, cur, items.len(), max_rows(s), menu_w, width(query)) else {
            return;
        };
        // Keep the selection inside the (possibly shrunk) window.
        let mut top = menu.scroll_top().min(sel);
        if sel >= top + l.n {
            top = sel + 1 - l.n;
        }
        top = top.min(items.len() - l.n);
        for (i, idx) in (top..top + l.n).enumerate() {
            let (s, pad, img) = self.list_row(&items[idx], idx == sel, l.w);
            out.push(Patch {
                row: l.list_top + i,
                x: l.x0,
                w: l.w,
                s,
                pad,
                img,
            });
        }
        let pop_x = match () {
            _ if items.is_empty() || !menu.popout(s) => None,
            _ if l.x0 + l.w + POP_W <= size.1 => Some(l.x0 + l.w),
            _ if l.x0 >= POP_W => Some(l.x0 - POP_W),
            _ => None,
        };
        if let Some(px) = pop_x {
            // ponytail: no spinner while the pop-out replaces the description bar.
            let pad = self.pen(self.theme.description_text, self.theme.main_bg, "");
            for (i, s) in self
                .popout_rows(menu, l.n + 1, &pad)
                .into_iter()
                .enumerate()
            {
                out.push(Patch {
                    row: l.list_top + i,
                    x: px,
                    w: POP_W,
                    s,
                    pad: pad.clone(),
                    img: None,
                });
            }
        } else {
            let spin = menu
                .loading()
                .then(|| SPINNER[input.spinner % SPINNER.len()]);
            let (s, pad) = self.desc_row(menu, l.w, spin);
            out.push(Patch {
                row: l.desc_row,
                x: l.x0,
                w: l.w,
                s,
                pad,
                img: None,
            });
        }
    }

    fn rgb(&self, base: u8, (r, g, b): Rgb) -> String {
        if self.truecolor {
            format!("{base};2;{r};{g};{b}")
        } else {
            format!("{base};5;{}", xterm256((r, g, b)))
        }
    }

    /// `ESC[0;<fg>;<bg><extra>m`
    fn pen(&self, fg: Rgb, bg: Rgb, extra: &str) -> String {
        format!("\x1b[0;{};{}{extra}m", self.rgb(38, fg), self.rgb(48, bg))
    }

    /// `' '` + icon slot (two columns) + `' '` + title (match highlight) + (`' '` + dim args hint when room) + pad
    /// (SPEC §7.4.2). The image, if any, goes over the slot.
    fn list_row(&self, it: &Suggestion, selected: bool, w: usize) -> (String, String, Option<Image>) {
        let t = &self.theme;
        let (fg, bg, mbg) = if selected {
            (t.selected_text, t.selected_bg, t.selected_matching_bg)
        } else {
            (t.main_text, t.main_bg, t.matching_bg)
        };
        let (base, hl) = (self.pen(fg, bg, ""), self.pen(fg, mbg, ""));
        let mut r = Run::default();
        let slot = icons::slot(it, self.images);
        r.put(&base, " ");
        if slot.badge {
            r.put(&self.pen((255, 255, 255), bg, ""), &slot.text);
        } else if let (Graphics::Tmux, Some(img)) = (self.images, &slot.image) {
            r.put(&self.pen(fg, bg, &icons::color(img.id)), &slot.text);
        } else {
            r.put(&base, &slot.text);
        }
        r.put(&base, " ");
        let avail = w.saturating_sub(5);
        let (title, hint) = (clean(&it.title()), clean(it.args_hint()));
        let show_hint = !hint.is_empty() && width(&title) + 1 + width(&hint) <= avail;
        let shown = fit(&title, avail);
        let kept = if shown == title {
            usize::MAX
        } else {
            shown.chars().count().saturating_sub(1)
        };
        let mut on = false;
        for (i, c) in shown.chars().enumerate() {
            let h = i < kept && it.highlight().contains(&i);
            if h != on {
                r.s.push_str(if h { &hl } else { &base });
                on = h;
            }
            r.s.push(c);
            r.w += cw(c);
        }
        if on {
            r.s.push_str(&base);
        }
        if show_hint {
            r.put(&base, " ");
            r.put(&self.pen(fg, bg, ";2"), &hint);
        }
        r.pad_to(&base, w);
        (r.s, base, slot.image)
    }

    /// Description bar / argument hint: `' '` + bold `name: ` + italic description, `No description` in grey,
    /// right-aligned spinner (SPEC §7.4.2).
    fn desc_row(&self, menu: &Menu, w: usize, spin: Option<char>) -> (String, String) {
        let t = &self.theme;
        let base = self.pen(t.description_text, t.main_bg, "");
        let (name, desc) = describe(menu);
        let end = w.saturating_sub(if spin.is_some() { 2 } else { 1 });
        let mut r = Run::default();
        r.put(&base, " ");
        if name.is_empty() && desc.is_empty() {
            r.put(
                &self.pen((140, 140, 140), t.main_bg, ""),
                &fit("No description", end.saturating_sub(r.w)),
            );
        } else {
            if !name.is_empty() {
                let label = if desc.is_empty() {
                    name
                } else {
                    format!("{name}: ")
                };
                r.put(
                    &self.pen(t.description_text, t.main_bg, ";1"),
                    &fit(&label, end.saturating_sub(r.w)),
                );
            }
            r.put(
                &self.pen(t.description_text, t.main_bg, ";3"),
                &fit(&desc, end.saturating_sub(r.w)),
            );
        }
        r.pad_to(&base, end);
        if let Some(c) = spin {
            r.put(&base, c.encode_utf8(&mut [0; 4]));
        }
        r.pad_to(&base, w);
        (r.s, base)
    }

    /// Pop-out panel (SPEC §7.4.3): `h` rows of the description wrapped to the panel width.
    fn popout_rows(&self, menu: &Menu, h: usize, pen: &str) -> Vec<String> {
        let (name, desc) = describe(menu);
        let text = [name, desc]
            .into_iter()
            .filter(|s| !s.is_empty())
            .collect::<Vec<_>>()
            .join(": ");
        let text = if text.is_empty() {
            "No description".to_string()
        } else {
            text
        };
        let mut lines = wrap(&text, POP_W - 2, h);
        lines.resize(h, String::new());
        (lines.iter())
            .map(|l| {
                let mut r = Run::default();
                r.put(pen, " ");
                r.put(pen, l);
                r.pad_to(pen, POP_W);
                r.s
            })
            .collect()
    }
}

/// (bold label, description): the selected item's, `file`/`folder`, else the current argument's. Sanitized.
fn describe(menu: &Menu) -> (String, String) {
    let sel = menu.items().get(menu.selected());
    let own = sel
        .and_then(|s| s.description())
        .filter(|d| !d.trim().is_empty())
        .map(String::from)
        .or_else(|| match sel.map(|s| s.kind()) {
            Some(SuggestionKind::File) => Some("file".into()),
            Some(SuggestionKind::Folder) => Some("folder".into()),
            _ => None,
        });
    match (own, menu.arg_hint()) {
        (Some(d), _) => (String::new(), clean(&d)),
        (None, Some(a)) => (clean(a.name()), clean(a.description())),
        (None, None) => (String::new(), String::new()),
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Side {
    Below,
    Above,
}

#[derive(Debug, PartialEq, Eq)]
struct Layout {
    side: Side,
    /// List rows (0 in argument-hint mode).
    n: usize,
    list_top: usize,
    desc_row: usize,
    x0: usize,
    w: usize,
}

/// SPEC §7.4.1 (D8: never scroll; below, above, shrink, else hide). `items == 0` = argument-hint mode (one row).
fn layout(
    (rows, cols): (usize, usize),
    (cy, cx): (usize, usize),
    items: usize,
    n_max: usize,
    menu_w: usize,
    query_w: usize,
) -> Option<Layout> {
    let w = menu_w.min(cols);
    let mut n = n_max.min(items);
    let mut h = n + 1;
    let (below, above) = (rows - 1 - cy, cy);
    let side = match () {
        _ if below >= h => Side::Below,
        _ if above >= h => Side::Above,
        _ if below >= above => Side::Below,
        _ => Side::Above,
    };
    let room = if side == Side::Below { below } else { above };
    if room < h {
        if items == 0 || room < 2 {
            return None;
        }
        n = room - 1;
        h = room;
    }
    // Rows start with `' '` + icon (2 columns) + `' '`: 4 columns before the title.
    let x0 = (cx as isize - query_w as isize - 4).clamp(0, (cols - w) as isize) as usize;
    let (list_top, desc_row) = match side {
        Side::Below => (cy + 1, cy + n + 1),
        Side::Above => (cy - h, cy - 1),
    };
    Some(Layout {
        side,
        n,
        list_top,
        desc_row,
        x0,
        w,
    })
}

/// Ghost text at the cursor in bright black, clipped at the first non-blank cell to the right and at the right
/// edge (SPEC §8.2).
fn ghost_patch(
    row: &Row<Cell>,
    cols: usize,
    (cy, cx): (usize, usize),
    ghost: &str,
) -> Option<Patch> {
    let end = (cx..cols)
        .find(|&x| !blank(&row[Column(x)]))
        .unwrap_or(cols);
    let (mut s, mut w) = (String::new(), 0);
    for c in clean(ghost).chars() {
        if w == 0 && cw(c) == 0 {
            continue; // would combine with the cell left of the span
        }
        if w + cw(c) > end - cx {
            break;
        }
        s.push(c);
        w += cw(c);
    }
    (w > 0).then(|| Patch {
        row: cy,
        x: cx,
        w,
        s: format!("\x1b[0m\x1b[90m{s}"),
        pad: "\x1b[0m".into(),
        img: None,
    })
}

/// Cursor back to the model position with the model's pen and cursor visibility, end of synchronized output. A
/// pending wrap is re-created by re-printing the last cell.
fn tail(model: &Model) -> String {
    let grid = model.grid();
    let p = grid.cursor.point;
    let (cy, cx) = (p.line.0 as usize, p.column.0);
    let mut s = String::new();
    if grid.cursor.input_needs_wrap {
        let row = &grid[p.line];
        let x = cx - usize::from(cx > 0 && row[Column(cx)].flags.contains(Flags::WIDE_CHAR_SPACER));
        cup(&mut s, cy, x);
        restore_cells(&mut s, row, x..cx + 1);
    } else {
        cup(&mut s, cy, cx);
    }
    s += &sgr(&grid.cursor.template);
    if model.mode().contains(TermMode::SHOW_CURSOR) {
        s += "\x1b[?25h";
    }
    s += "\x1b[?2026l";
    s
}

fn cup(s: &mut String, row: usize, col: usize) {
    let _ = write!(s, "\x1b[{};{}H", row + 1, col + 1);
}

/// Re-emit model cells exactly (SPEC §7.5.2 `restore_cells`). Hyperlinks are not emitted.
/// ponytail: a covered last-column cell loses the terminal's soft-wrap bit (only copy/paste joining notices);
/// restoring it would need a real wrap, which can scroll.
fn restore_cells(out: &mut String, row: &Row<Cell>, xs: Range<usize>) {
    let mut pen = String::new();
    for x in xs {
        let cell = &row[Column(x)];
        if cell.flags.contains(Flags::WIDE_CHAR_SPACER) {
            continue;
        }
        let s = sgr(cell);
        if s != pen {
            out.push_str(&s);
            pen = s;
        }
        out.push(if cell.c == '\0' { ' ' } else { cell.c });
        out.extend(cell.zerowidth().unwrap_or_default());
    }
}

/// `ESC[0;…m` for a model cell: exact colours (no 256 downgrade), attributes, underline style and colour.
fn sgr(cell: &Cell) -> String {
    let mut s = String::from("\x1b[0");
    color(&mut s, cell.fg, 30, 38);
    color(&mut s, cell.bg, 40, 48);
    const ATTRS: [(Flags, &str); 11] = [
        (Flags::BOLD, "1"),
        (Flags::DIM, "2"),
        (Flags::ITALIC, "3"),
        (Flags::UNDERLINE, "4"),
        (Flags::DOUBLE_UNDERLINE, "4:2"),
        (Flags::UNDERCURL, "4:3"),
        (Flags::DOTTED_UNDERLINE, "4:4"),
        (Flags::DASHED_UNDERLINE, "4:5"),
        (Flags::INVERSE, "7"),
        (Flags::HIDDEN, "8"),
        (Flags::STRIKEOUT, "9"),
    ];
    for (f, code) in ATTRS {
        if cell.flags.contains(f) {
            s.push(';');
            s.push_str(code);
        }
    }
    let _ = match cell.underline_color() {
        Some(Color::Spec(c)) => write!(s, ";58;2;{};{};{}", c.r, c.g, c.b),
        Some(Color::Indexed(i)) => write!(s, ";58;5;{i}"),
        Some(Color::Named(n)) if (n as usize) < 16 => write!(s, ";58;5;{}", n as usize),
        _ => Ok(()),
    };
    s.push('m');
    s
}

/// Named 0–7 → base+n, bright → base+60+n, Dim* → normal (+ dim for fg), Indexed → 38;5, Spec → 38;2.
fn color(s: &mut String, c: Color, base: usize, ext: u8) {
    let _ = match c {
        Color::Spec(c) => write!(s, ";{ext};2;{};{};{}", c.r, c.g, c.b),
        Color::Indexed(i) => write!(s, ";{ext};5;{i}"),
        Color::Named(n) => {
            let dim = NamedColor::DimBlack as usize;
            match n as usize {
                i @ 0..=7 => write!(s, ";{}", base + i),
                i @ 8..=15 => write!(s, ";{}", base + 60 + i - 8),
                i if (dim..dim + 8).contains(&i) => write!(
                    s,
                    ";{}{}",
                    base + i - dim,
                    if base == 30 { ";2" } else { "" }
                ),
                _ if n == NamedColor::DimForeground && base == 30 => write!(s, ";2"),
                _ => Ok(()),
            }
        }
    };
}

/// Blank for clipping / repaint: a space with the default background and no visible attribute.
fn blank(c: &Cell) -> bool {
    matches!(c.c, ' ' | '\0')
        && c.bg == Color::Named(NamedColor::Background)
        && !c
            .flags
            .intersects(Flags::INVERSE | Flags::ALL_UNDERLINES | Flags::STRIKEOUT)
        && c.zerowidth().is_none_or(|z| z.is_empty())
}

/// Control chars (newlines, tabs, ESC) would break the frame: show them as spaces.
fn clean(s: &str) -> String {
    s.chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .collect()
}

/// Display width of a char, as the model computes it (per char, `unicode-width`).
fn cw(c: char) -> usize {
    c.width().unwrap_or(0)
}

fn width(s: &str) -> usize {
    s.chars().map(cw).sum()
}

/// Fit into `w` columns; overflow replaced by `…`.
fn fit(s: &str, w: usize) -> String {
    if width(s) <= w {
        return s.to_string();
    }
    let (mut out, mut used) = (String::new(), 0);
    for c in s.chars() {
        if used + cw(c) > w.saturating_sub(1) {
            break;
        }
        out.push(c);
        used += cw(c);
    }
    if w > 0 {
        out.push('…');
    }
    out
}

/// Greedy word wrap by display width (long words hard-broken), at most `max` lines, `…` on overflow.
fn wrap(text: &str, w: usize, max: usize) -> Vec<String> {
    let mut lines = vec![String::new()];
    let mut lw = 0;
    for word in text.split_whitespace() {
        if lw > 0 && lw + 1 + width(word) > w {
            lines.push(String::new());
            lw = 0;
        }
        if lw > 0 {
            lines.last_mut().unwrap().push(' ');
            lw += 1;
        }
        for c in word.chars() {
            if lw + cw(c) > w {
                lines.push(String::new());
                lw = 0;
            }
            lines.last_mut().unwrap().push(c);
            lw += cw(c);
        }
    }
    if lines.len() > max {
        lines.truncate(max);
        if let Some(l) = lines.last_mut() {
            *l = fit(&format!("{l}…"), w);
        }
    }
    lines
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::menu::tests::{comp, item, settings};
    use alacritty_terminal::event::VoidListener;
    use alacritty_terminal::term::Config;
    use alacritty_terminal::vte::ansi::Processor;
    use serde_json::json;

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
        m.on_completion(comp("c", 1, items), s);
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
        let mut r = Renderer::new(Theme::dark(), truecolor);
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
        m.handle(crate::keys::Action::NavigateDown, &s);
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
        m.on_completion(c, &s);
        assert!(m.shown(&s) && !m.intercepting(&s));
        let a = model(40, 10, &[b"$ c"]);
        let mut r = Renderer::new(Theme::light(), true);
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
    fn dedupe_guards_and_clear() {
        let s = Settings::default();
        let m = menu(subs(), &s);
        let a = model(40, 12, &[b"$ c"]);
        let mut r = Renderer::new(Theme::dark(), true);
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
        let mut r = Renderer::new(Theme::dark(), true);
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
    fn layout_decisions() {
        // (rows, cols), (cy, cx), items, n_max, width, query width
        let l = layout((24, 40), (2, 10), 5, 7, 20, 0).unwrap();
        assert_eq!(
            l,
            Layout {
                side: Side::Below,
                n: 5,
                list_top: 3,
                desc_row: 8,
                x0: 6,
                w: 20
            }
        );
        let l = layout((24, 40), (22, 10), 9, 7, 20, 0).unwrap();
        assert_eq!(
            l,
            Layout {
                side: Side::Above,
                n: 7,
                list_top: 14,
                desc_row: 21,
                x0: 6,
                w: 20
            }
        );
        // Neither side fits: shrink on the bigger side (above here).
        let l = layout((10, 40), (5, 10), 9, 7, 20, 0).unwrap();
        assert_eq!(
            l,
            Layout {
                side: Side::Above,
                n: 4,
                list_top: 0,
                desc_row: 4,
                x0: 6,
                w: 20
            }
        );
        let l = layout((10, 40), (4, 10), 9, 7, 20, 0).unwrap();
        assert_eq!(
            (l.side, l.n, l.list_top, l.desc_row),
            (Side::Below, 4, 5, 9)
        );
        // No room for one row + description: hide. Argument hint needs one row.
        assert_eq!(layout((3, 40), (1, 10), 3, 7, 20, 0), None);
        let l = layout((3, 40), (1, 10), 0, 7, 20, 0).unwrap();
        assert_eq!(
            (l.side, l.n, l.list_top, l.desc_row),
            (Side::Below, 0, 2, 2)
        );
        let l = layout((3, 40), (2, 10), 0, 7, 20, 0).unwrap();
        assert_eq!((l.side, l.list_top, l.desc_row), (Side::Above, 1, 1));
        // x0 clamping (aligned under the typed token, kept on screen) and width clamping.
        // Title (after `' '` + icon + `' '`) starts in the column of the typed token: 12 - 3 = 5 + 4.
        assert_eq!(layout((24, 40), (2, 12), 3, 7, 20, 3).unwrap().x0, 5);
        assert_eq!(layout((24, 40), (2, 2), 3, 7, 20, 5).unwrap().x0, 0);
        assert_eq!(layout((24, 40), (2, 39), 3, 7, 20, 0).unwrap().x0, 20);
        let l = layout((24, 30), (2, 9), 3, 7, 40, 0).unwrap();
        assert_eq!(
            l,
            Layout {
                side: Side::Below,
                n: 3,
                list_top: 3,
                desc_row: 6,
                x0: 0,
                w: 30
            }
        );
    }

    #[test]
    fn truncation_and_wrap() {
        assert_eq!(fit("美国人美国人", 10), "美国人美…");
        assert_eq!(width(&fit("美国人美国人", 10)), 9);
        assert_eq!(fit("abc", 3), "abc");
        assert_eq!(fit("abcd", 3), "ab…");
        assert_eq!(fit("abcd", 0), "");
        assert_eq!(wrap("one two three", 7, 5), vec!["one two", "three"]);
        assert_eq!(wrap("abcdefghij", 4, 5), vec!["abcd", "efgh", "ij"]);
        assert_eq!(wrap("aa bb cc dd", 5, 1), vec!["aa b…"]);
        assert_eq!(wrap("aa bb cc dd", 6, 1), vec!["aa bb…"]);
        assert!(SPINNER.iter().all(|&c| cw(c) == 1) && cw('…') == 1);
    }

    #[test]
    fn row_contents() {
        let s = settings(r#"{"autocomplete.width": 20}"#);
        let r = Renderer::new(Theme::dark(), true);
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
        let mut r = Renderer::new(Theme::dark(), true).with_images(Graphics::Direct);
        // First frame sends each icon once, then places one image per row, under the text.
        let first = r.frame(&a, &m, &input(&s, None));
        let text = String::from_utf8_lossy(&first).into_owned();
        assert!(text.contains("\x1b_Ga=t,i="), "{text:?}");
        assert_eq!(text.matches("\x1b_Ga=p,").count(), 4, "{text:?}");
        let clear = r.take_clear();
        assert!(String::from_utf8_lossy(&clear).contains(icons::CLEAR_PLACEMENTS));
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
        let mut r = Renderer::new(Theme::dark(), true).with_images(Graphics::Tmux);
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
        assert!(!String::from_utf8_lossy(&clear).contains(icons::CLEAR_PLACEMENTS));
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

    #[test]
    fn colours_and_themes() {
        assert_eq!(parse_color("#fff"), Some((255, 255, 255)));
        assert_eq!(parse_color("#1a2B3c"), Some((26, 43, 60)));
        assert_eq!(parse_color("#ff000080"), Some((128, 0, 0)));
        assert_eq!(parse_color("rgb( 1, 2, 3 )"), Some((1, 2, 3)));
        assert_eq!(parse_color("rgb(106,142,218)"), Some((106, 142, 218)));
        for bad in [
            "rgb(1,2)",
            "rgb(1,2,3,4)",
            "red",
            "#12345",
            "#ggg",
            "rgb(300,0,0)",
        ] {
            assert_eq!(parse_color(bad), None, "{bad}");
        }
        assert_eq!(xterm256((0, 0, 0)), 16);
        assert_eq!(xterm256((255, 255, 255)), 231);
        assert_eq!(xterm256((128, 128, 128)), 244);
        assert_eq!(xterm256((95, 135, 175)), 67);
        assert_eq!(xterm256((30, 90, 199)), 26);
        assert_eq!(xterm256((48, 48, 48)), 236);

        let v1: Value = serde_json::from_str(
            r##"{"version":"1.0","theme":{"backgroundColor":"#000","selection":{"backgroundColor":"rgb(1, 2, 3)"},
                 "description":{"textColor":"nope"}}}"##,
        )
        .unwrap();
        let t = Theme::from_json(&v1);
        assert_eq!((t.main_bg, t.selected_bg), ((0, 0, 0), (1, 2, 3)));
        assert_eq!(
            (t.main_text, t.description_text),
            (Theme::dark().main_text, Theme::dark().description_text)
        );
        let b16: Value =
            serde_json::from_str(r##"{"shade0":"#101010","accent6":"#123456"}"##).unwrap();
        let t = Theme::from_json(&b16);
        assert_eq!(
            (
                t.main_bg,
                t.matching_bg,
                t.selected_matching_bg,
                t.main_text
            ),
            ((16, 16, 16), (40, 40, 40), (18, 52, 86), (232, 232, 232))
        );

        let theme = |name: &str, bg| {
            Theme::from_settings(
                &settings(&format!(r#"{{"autocomplete.theme": "{name}"}}"#)),
                bg,
            )
        };
        assert_eq!(theme("light", None), Theme::light());
        assert_eq!(theme("system", Some((250, 250, 250))), Theme::light());
        assert_eq!(theme("system", Some((10, 10, 30))), Theme::dark());
        assert_eq!(theme("system", None), Theme::dark());
        assert_eq!(
            Theme::from_settings(&Settings::default(), None),
            Theme::dark()
        );
    }

    /// The eight colours of a theme in field order: main_bg, main_text, matching_bg, selected_bg, selected_text,
    /// selected_matching_bg, description_text, description_border.
    fn roles(t: &Theme) -> [Rgb; 8] {
        [
            t.main_bg,
            t.main_text,
            t.matching_bg,
            t.selected_bg,
            t.selected_text,
            t.selected_matching_bg,
            t.description_text,
            t.description_border,
        ]
    }

    #[test]
    fn should_draw_a_json_value_that_is_not_a_fig_theme_with_the_base16_filler() {
        let filler = [
            (24, 24, 24),
            (232, 232, 232),
            (40, 40, 40),
            (56, 56, 56),
            (248, 248, 248),
            (186, 139, 175),
            (216, 216, 216),
            (40, 40, 40),
        ];
        let not_fig = [
            json!({}),
            json!([1]),
            json!("x"),
            json!(null),
            json!(5),
            json!({"version": "1.0"}),
            json!({"version": 1, "theme": {}}),
            json!({"version": "1.0", "theme": "x"}),
        ];

        for value in not_fig {
            assert_eq!(roles(&Theme::from_json(&value)), filler, "{value}");
        }
    }

    #[test]
    fn should_fall_back_per_colour_for_a_fig_theme_with_missing_or_unparsable_colours() {
        let fig = json!({
            "version": "1.0",
            "theme": {
                "backgroundColor": "#000",
                "matchBackgroundColor": "red",
                "selection": {"backgroundColor": 5, "textColor": null, "matchBackgroundColor": "rgb(1,2)"},
                "description": {"textColor": "#102030"}
            }
        });
        let dark = [
            (48, 48, 48),
            (180, 180, 180),
            (95, 89, 56),
            (30, 90, 199),
            (253, 253, 253),
            (106, 142, 218),
            (180, 180, 180),
            (65, 65, 65),
        ];

        let theme = Theme::from_json(&fig);

        assert_eq!(
            roles(&theme),
            [
                (0, 0, 0),
                dark[1],
                dark[2],
                dark[3],
                dark[4],
                dark[5],
                (16, 32, 48),
                dark[7]
            ]
        );
        // a Fig theme without a single colour is dark, not the base16 filler
        let empty = json!({"version": "1.0", "theme": {}});
        assert_eq!(roles(&Theme::from_json(&empty)), dark);
    }

    #[test]
    fn should_fall_back_per_colour_for_a_base16_theme_with_missing_or_unparsable_colours() {
        let base16 = json!({
            "shade0": "#101010",
            "shade1": "#0a0b0c",
            "shade5": 5,
            "shade6": null,
            "shade7": "red",
            "accent6": "rgb(1,2)",
            "shade3": "#ffffff",
            "accent0": "#ffffff"
        });

        let theme = Theme::from_json(&base16);

        assert_eq!(
            roles(&theme),
            [
                (16, 16, 16),
                (232, 232, 232),
                (10, 11, 12),
                (56, 56, 56),
                (248, 248, 248),
                (186, 139, 175),
                (216, 216, 216),
                (10, 11, 12)
            ]
        );
    }

    /// 64-bit FNV-1a: a stable hash that needs no crate and does not change between Rust versions.
    fn fnv1a(bytes: &[u8]) -> u64 {
        bytes.iter().fold(0xcbf2_9ce4_8422_2325, |h, b| {
            (h ^ u64::from(*b)).wrapping_mul(0x0000_0100_0000_01b3)
        })
    }

    /// Draws `frames` one after the other and hashes every frame, then every clear (taken after each frame).
    fn overlay_hashes(r: &mut Renderer, a: &Model, m: &Menu, frames: &[FrameInput]) -> (u64, u64) {
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
        let custom = || Theme::from_json(&base16);
        let mut got = Vec::new();

        // Below the cursor, wide characters at the edges of the span, truecolor, first row selected.
        let (a, m) = (model(40, 12, &[&below]), menu(subs(), &s));
        let mut r = Renderer::new(Theme::dark(), true);
        got.push(("below", overlay_hashes(&mut r, &a, &m, &[input(&s, None)])));

        // The light theme on a 256-colour terminal.
        let mut r = Renderer::new(Theme::light(), false);
        got.push((
            "light_256",
            overlay_hashes(&mut r, &a, &m, &[input(&s, None)]),
        ));

        // Scrolled: two rows high, third row selected, whose description has a newline and wide characters.
        let tall = settings(r#"{"autocomplete.width": 20, "autocomplete.height": 2}"#);
        let mut m = menu(subs(), &tall);
        m.handle(crate::keys::Action::NavigateDown, &tall);
        m.handle(crate::keys::Action::NavigateDown, &tall);
        let mut r = Renderer::new(Theme::dark(), true);
        got.push((
            "scrolled",
            overlay_hashes(&mut r, &a, &m, &[input(&tall, None)]),
        ));

        // Above the cursor on the bottom row, ghost text clipped at the right prompt, pop-out beside the list.
        let always =
            settings(r#"{"autocomplete.width": 20, "autocomplete.alwaysShowDescription": true}"#);
        let mut m = menu(subs(), &always);
        m.handle(crate::keys::Action::NavigateDown, &always);
        let mut out = String::from_utf8(screen("")).unwrap();
        out += "\r\n\r\n\r\n\r\n\r\n$ ec          \x1b[36m[rprompt]\x1b[0m\x1b[14;5H";
        let a = model(60, 14, &[out.as_bytes()]);
        let mut r = Renderer::new(Theme::dark(), true);
        let ghost = input(&always, Some("ho hello\tworld and more"));
        got.push((
            "above_ghost_popout",
            overlay_hashes(&mut r, &a, &m, &[ghost]),
        ));

        // An argument hint and no rows (custom theme), with a wide ghost clipped at the last column.
        let hint = Settings::default();
        let m = menu(Vec::new(), &hint);
        let a = model(40, 10, &[format!("{}$ c", " ".repeat(30)).as_bytes()]);
        let mut r = Renderer::new(custom(), true);
        let wide = input(&hint, Some("中文中文中文"));
        got.push((
            "arg_hint_wide_ghost",
            overlay_hashes(&mut r, &a, &m, &[wide]),
        ));

        // The custom theme over the list, second row selected.
        let mut m = menu(subs(), &s);
        m.handle(crate::keys::Action::NavigateDown, &s);
        let a = model(40, 12, &[&below]);
        let mut r = Renderer::new(custom(), true);
        got.push((
            "custom_base16",
            overlay_hashes(&mut r, &a, &m, &[input(&s, None)]),
        ));

        // Icons: sent once, then only placed (direct) or drawn as placeholders (tmux).
        let m = menu(subs(), &s);
        let a = model(40, 12, &[b"$ git c"]);
        for (name, g) in [
            ("icons_direct", Graphics::Direct),
            ("icons_tmux", Graphics::Tmux),
        ] {
            let mut r = Renderer::new(Theme::dark(), true).with_images(g);
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
        let mut r = Renderer::new(custom(), true);
        got.push((
            "long_description_bar",
            overlay_hashes(&mut r, &a, &m, &[input(&s, None)]),
        ));
        let m = menu(rows(), &always);
        let a = model(60, 12, &[b"$ git c"]);
        let mut r = Renderer::new(custom(), true);
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
}
