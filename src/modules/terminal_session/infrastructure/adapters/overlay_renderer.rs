//! Frame bytes for the in-terminal overlay: menu, description bar, pop-out, ghost text, and
//! exact restoration of the covered cells from the model (SPEC §7.4–7.7, §8.2).
use crate::modules::dropdown::domain::services::color_parser::xterm256;
use crate::modules::dropdown::domain::services::icon_resolver;
use crate::modules::dropdown::domain::services::menu_describer::describe;
use crate::modules::dropdown::domain::services::menu_layout_calculator::layout;
use crate::modules::dropdown::domain::services::text_fitting::{clean, cw, fit, width, wrap};
use crate::modules::dropdown::domain::value_objects::graphics_support::GraphicsSupport;
use crate::modules::dropdown::domain::value_objects::icon_id::{self, IconId};
use crate::modules::dropdown::domain::value_objects::rgb::Rgb;
use crate::modules::dropdown::domain::value_objects::theme::Theme;
use crate::modules::terminal_session::infrastructure::adapters::{icon_assets, kitty_graphics};
use crate::modules::dropdown::domain::entities::menu::{max_rows, Menu};
use crate::modules::settings::domain::services::settings_catalog::AUTOCOMPLETE_WIDTH;
use crate::modules::completion::domain::value_objects::suggestion::Suggestion;
use crate::modules::terminal_session::application::frame_input::FrameInput;
use crate::modules::terminal_session::infrastructure::adapters::headless_model::Model;
use alacritty_terminal::grid::{Dimensions, Row};
use alacritty_terminal::index::{Column, Line};
use alacritty_terminal::term::cell::{Cell, Flags};
use alacritty_terminal::term::TermMode;
use alacritty_terminal::vte::ansi::{Color, NamedColor};
use std::collections::HashSet;
use std::fmt::Write as _;
use std::ops::Range;

pub(crate) struct OverlayRenderer {
    theme: Theme,
    truecolor: bool,
    /// Restores every span the last frame covered (computed from the model at draw time).
    clear: Vec<u8>,
    /// Last frame emitted: an identical frame over a still-drawn one is skipped (render key, SPEC §7.5.3).
    /// ponytail: the key is the frame bytes themselves (a few rows per frame, cheap to build and compare).
    last: Vec<u8>,
    /// How row icons are drawn (`kitty_graphics::detect`); with direct placements the clear removes them too.
    images: GraphicsSupport,
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
    img: Option<IconId>,
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

impl OverlayRenderer {
    /// `truecolor` = `COLORTERM ∈ {truecolor, 24bit}` or `WT_SESSION` set.
    pub(crate) fn new(theme: Theme, truecolor: bool) -> OverlayRenderer {
        OverlayRenderer {
            theme,
            truecolor,
            clear: Vec::new(),
            last: Vec::new(),
            images: GraphicsSupport::Off,
            sent: HashSet::new(),
        }
    }

    /// Row icons drawn as kitty graphics images (Ghostty, kitty, also behind tmux); ASCII when `Off`.
    pub(crate) fn with_images(mut self, g: GraphicsSupport) -> OverlayRenderer {
        self.images = g;
        self
    }

    pub(crate) fn set_theme(&mut self, theme: Theme) {
        self.theme = theme;
        self.last.clear();
    }

    /// Bytes restoring every cell span the last frame covered; empties the cache (SPEC §7.5.3).
    pub(crate) fn take_clear(&mut self) -> Vec<u8> {
        self.last.clear();
        std::mem::take(&mut self.clear)
    }

    /// Forget the cached clear without emitting it (after resize: the terminal reflowed the old frame).
    pub(crate) fn drop_clear(&mut self) {
        self.clear.clear();
        self.last.clear();
    }

    /// Frame for the current state (SPEC §7.5.2). Empty when nothing changed or nothing is drawn. Stores the
    /// matching clear for the next `take_clear`. After PTY output call `take_clear` first (the screen then equals
    /// the model); for state-only changes call `frame` alone: a still-drawn frame is cleared in the same bytes,
    /// or nothing is emitted when the new frame is identical.
    pub(crate) fn frame(&mut self, model: &Model, menu: &Menu, input: &FrameInput) -> Vec<u8> {
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
    pub(crate) fn full_repaint(&mut self, model: &Model) -> Vec<u8> {
        self.drop_clear();
        let grid = model.grid();
        let mut s = String::from(HEAD);
        if self.images == GraphicsSupport::Direct {
            s += kitty_graphics::CLEAR_PLACEMENTS;
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
        if self.images == GraphicsSupport::Direct {
            clear += kitty_graphics::CLEAR_PLACEMENTS;
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
                if !self.sent.contains(&img.value())
                    && transmit.iter().all(|(id, _)| *id != img.value())
                {
                    let image = icon_assets::kitty_image(*img);
                    transmit.push((img.value(), kitty_graphics::transmit(&image, self.images)));
                }
                // Behind tmux the placeholders in `p.s` show the image.
                if self.images == GraphicsSupport::Direct {
                    cup(&mut frame, p.row, p.x + 1);
                    frame += &kitty_graphics::place(*img, k + 1);
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
        if sel >= top + l.rows() {
            top = sel + 1 - l.rows();
        }
        top = top.min(items.len() - l.rows());
        for (i, idx) in (top..top + l.rows()).enumerate() {
            let (s, pad, img) = self.list_row(&items[idx], idx == sel, l.width());
            out.push(Patch {
                row: l.list_top() + i,
                x: l.x0(),
                w: l.width(),
                s,
                pad,
                img,
            });
        }
        let pop_x = match () {
            _ if items.is_empty() || !menu.popout(s) => None,
            _ if l.x0() + l.width() + POP_W <= size.1 => Some(l.x0() + l.width()),
            _ if l.x0() >= POP_W => Some(l.x0() - POP_W),
            _ => None,
        };
        if let Some(px) = pop_x {
            // ponytail: no spinner while the pop-out replaces the description bar.
            let pad = self.pen(self.theme.description_text(), self.theme.main_bg(), "");
            for (i, s) in self
                .popout_rows(menu, l.rows() + 1, &pad)
                .into_iter()
                .enumerate()
            {
                out.push(Patch {
                    row: l.list_top() + i,
                    x: px,
                    w: POP_W,
                    s,
                    pad: pad.clone(),
                    img: None,
                });
            }
        } else {
            let spin = menu
                .loading(input.now)
                .then(|| SPINNER[input.spinner % SPINNER.len()]);
            let (s, pad) = self.desc_row(menu, l.width(), spin);
            out.push(Patch {
                row: l.desc_row(),
                x: l.x0(),
                w: l.width(),
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
    fn list_row(&self, it: &Suggestion, selected: bool, w: usize) -> (String, String, Option<IconId>) {
        let t = &self.theme;
        let (fg, bg, mbg) = if selected {
            (t.selected_text(), t.selected_bg(), t.selected_matching_bg())
        } else {
            (t.main_text(), t.main_bg(), t.matching_bg())
        };
        let (base, hl) = (self.pen(fg, bg, ""), self.pen(fg, mbg, ""));
        let mut r = Run::default();
        let slot = icon_resolver::slot(it, self.images);
        r.put(&base, " ");
        if slot.is_badge() {
            r.put(&self.pen((255, 255, 255), bg, ""), slot.text());
        } else if let (GraphicsSupport::Tmux, Some(img)) = (self.images, slot.image()) {
            r.put(&self.pen(fg, bg, &icon_id::color(img.value())), slot.text());
        } else {
            r.put(&base, slot.text());
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
        (r.s, base, slot.image())
    }

    /// Description bar / argument hint: `' '` + bold `name: ` + italic description, `No description` in grey,
    /// right-aligned spinner (SPEC §7.4.2).
    fn desc_row(&self, menu: &Menu, w: usize, spin: Option<char>) -> (String, String) {
        let t = &self.theme;
        let base = self.pen(t.description_text(), t.main_bg(), "");
        let (name, desc) = describe(menu);
        let end = w.saturating_sub(if spin.is_some() { 2 } else { 1 });
        let mut r = Run::default();
        r.put(&base, " ");
        if name.is_empty() && desc.is_empty() {
            r.put(
                &self.pen((140, 140, 140), t.main_bg(), ""),
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
                    &self.pen(t.description_text(), t.main_bg(), ";1"),
                    &fit(&label, end.saturating_sub(r.w)),
                );
            }
            r.put(
                &self.pen(t.description_text(), t.main_bg(), ";3"),
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

#[cfg(test)]
mod tests;
