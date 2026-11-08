use crate::modules::shared::domain::value_objects::shell::Shell;
use crate::modules::terminal_session::domain::value_objects::cell_tag::CellTag;
use crate::modules::terminal_session::domain::value_objects::edit_buffer::EditBuffer;
use crate::modules::terminal_session::domain::value_objects::edit_buffer_context::EditBufferContext;
use crate::modules::terminal_session::domain::value_objects::shell_suggestion_color::{
    rgb_to_256, ShellSuggestionColor,
};
use crate::modules::terminal_session::domain::value_objects::text_emphasis::TextEmphasis;
use crate::modules::terminal_session::infrastructure::adapters::headless_model::Model;
use alacritty_terminal::grid::Dimensions;
use alacritty_terminal::index::{Column, Line};
use alacritty_terminal::term::cell::{Cell, Flags};
use alacritty_terminal::vte::ansi::Color;

fn cell_col(c: Color) -> Option<u8> {
    match c {
        Color::Indexed(i) => Some(i),
        Color::Spec(rgb) => Some(rgb_to_256(rgb.r, rgb.g, rgb.b)),
        Color::Named(n) => (n as usize).try_into().ok().filter(|i: &u8| *i < 16),
    }
}

fn matches_cell(z: &ShellSuggestionColor, cell: &Cell) -> bool {
    z.matches(cell_col(cell.fg), cell_col(cell.bg))
}

fn cell_emphasis(flags: Flags) -> TextEmphasis {
    TextEmphasis::new(flags.contains(Flags::DIM), flags.contains(Flags::ITALIC))
}

/// What `extract` collects while it scans the cells; the `EditBuffer` is built from it once, at the end.
#[derive(Default)]
struct Scraped {
    buffer: String,
    /// Byte offset into `buffer`.
    cursor: usize,
    /// The shell already draws its own suggestion after the cursor (SPEC §8.1).
    shell_ghost: bool,
    /// The cursor is on the command line (see `extract`): false on the rows an inline TUI draws below it (fzf
    /// `--height`, fish's pager search field), whose cells carry the open cycle's tag too.
    at_prompt: bool,
}

/// Suggestion-coloured cell (SPEC §5.1.3; pwsh: the reported prediction colour, else dim or italic).
fn suggestion(context: &EditBufferContext, cell: &Cell) -> bool {
    match context.shell() {
        Some(Shell::Zsh) => context.zsh_color().is_some_and(|c| matches_cell(c, cell)),
        Some(Shell::Fish) => context.fish_color().is_some_and(|c| matches_cell(c, cell)),
        Some(Shell::Pwsh) => match context.pwsh_color() {
            Some(c) => c.matches(cell_col(cell.fg), cell_col(cell.bg), cell_emphasis(cell.flags)),
            None => cell_emphasis(cell.flags).is_dim_or_italic(),
        },
        _ => false,
    }
}

/// Edit buffer + cursor from the tagged cells around the cursor (SPEC §5.6).
pub(crate) fn extract(model: &Model, context: &EditBufferContext<'_>) -> Option<EditBuffer> {
    let n = context.cycle();
    let tag = CellTag::Cycle(n).uri();
    let grid = model.grid();
    let cols = grid.columns();
    let cur = &grid.cursor;
    // Pending wrap = the cursor sits past the last column.
    let (cl, cc) = (
        cur.point.line.0,
        if cur.input_needs_wrap {
            cols
        } else {
            cur.point.column.0
        },
    );
    let row = |r: i32| &grid[Line(r)][..Column(cols)];
    let has = |c: &Cell, t: &str| c.hyperlink().is_some_and(|h| h.uri() == t);
    // Row r continues on r + 1: auto-wrap, or this cycle's text fills it to the last column and goes on at the
    // next row's first column after the shell's own CRLF, which sets no WRAPLINE (fish rewriting a wrapped line
    // after an edit or re-highlight on an earlier row). Not a prompt cell there: a right prompt at the margin
    // (zsh ZLE_RPROMPT_INDENT=0, fish) would join an inline TUI's rows below (fzf --layout=reverse) to the line.
    // ponytail: a real newline right after a line exactly as wide as the terminal reads as a soft wrap.
    let bottom = grid.bottommost_line().0;
    let wraps = |r: i32| {
        let last = &row(r)[cols - 1];
        last.flags.contains(Flags::WRAPLINE)
            || (r < bottom && has(last, &tag) && has(&row(r + 1)[0], &tag))
    };
    let tagged = |r: i32| row(r).iter().any(|c| has(c, &tag));

    let mut start = cl;
    while start > grid.topmost_line().0
        && cl - start < 100
        && (wraps(start - 1) || tagged(start - 1))
    {
        start -= 1;
    }
    // Below the cursor only wrap continuations and PS2 rows count (not zsh completion listings, not PSReadLine's
    // ListView, whose padded rows can look like wrap continuations).
    let pwsh = context.shell() == Some(Shell::Pwsh);
    let mut end = cl;
    while !(pwsh && context.pwsh_list_view())
        && end < grid.bottommost_line().0
        && (wraps(end) || (has(&row(end + 1)[0], CellTag::PROMPT_URI) && tagged(end + 1)))
    {
        end += 1;
    }

    // At the prompt = the cursor's line (rows joined by auto-wrap) starts on a prompt: a prompt cell (PS1's last
    // line, PS2, a prompt drawn without markers: NewCmd tags it) or PSReadLine's ContinuationPrompt. A NewCmd at
    // column 0 (PS1 ending in a newline) leaves its line no prompt cell: there, a prompt row right above and
    // nothing of this cycle drawn below (an inline TUI's query row can sit on that very row: fzf Ctrl-T, reverse
    // layout, from an empty line). An inline TUI's rows below the line (fzf --height, fish's pager search field)
    // are none of these; the buffer walk above still joins them, for pre_exec's scrape (a transient prompt is
    // redrawn without markers).
    // ponytail: continuation lines without a prompt (zle Alt-Enter, fish), an empty PS1, a markerless prompt
    // ending in a newline, and a newline-ended PS1's line while zsh lists completions below it read as not at
    // the prompt; recording the NewCmd row with scroll tracking is the upgrade.
    let mut top = cl;
    while top > grid.topmost_line().0 && cl - top < 100 && wraps(top - 1) {
        top -= 1;
    }
    let prompt_row = |r: i32| row(r).iter().any(|c| has(c, CellTag::PROMPT_URI));
    // A prompt filling its row to the margin (zsh then moves on with its own CRLF): like a PS1 ending in a newline.
    let full_prompt =
        |r: i32| r >= grid.topmost_line().0 && row(r).iter().all(|c| has(c, CellTag::PROMPT_URI));
    let cont = context.pwsh_continuation().chars().count();
    let below = end < grid.bottommost_line().0 && tagged(end + 1);
    let at_prompt = (top..=cl).any(prompt_row)
        || (pwsh
            && cont > 0
            && row(top)
                .iter()
                .take(cont)
                .map(|c| c.c)
                .eq(context.pwsh_continuation().chars()))
        || ((context.line_starts_at_column_zero() || full_prompt(top - 1))
            && top > grid.topmost_line().0
            && prompt_row(top - 1)
            && !below);
    let mut e = Scraped {
        at_prompt,
        ..Scraped::default()
    };
    let mut cursor = None;
    let (mut pad, mut line_has_cmd) = (0, false);
    // Blanks between command cells and the cursor are buffer text (zsh skips them with CUF on a redraw).
    let mut at_cursor = |e: &mut Scraped, pad: &mut usize, line_has_cmd: bool| {
        if line_has_cmd {
            e.buffer.extend(std::iter::repeat_n(' ', *pad));
            *pad = 0;
        }
        cursor = Some(e.buffer.len());
    };
    for r in start..=end {
        if r == start || !wraps(r - 1) {
            // ponytail: untagged cells before the first command cell of a line (prompt) are not padding.
            pad = 0;
            line_has_cmd = false;
        }
        let mut row_has_cmd = false;
        for (c, cell) in row(r).iter().enumerate() {
            if (r, c) == (cl, cc) {
                at_cursor(&mut e, &mut pad, line_has_cmd);
            }
            if cell
                .flags
                .intersects(Flags::WIDE_CHAR_SPACER | Flags::LEADING_WIDE_CHAR_SPACER)
            {
                continue;
            }
            let after = (r, c) >= (cl, cc);
            let cmd = has(cell, &tag);
            if cmd && !(after && suggestion(context, cell)) {
                if line_has_cmd {
                    e.buffer.extend(std::iter::repeat_n(' ', pad));
                }
                pad = 0;
                line_has_cmd = true;
                row_has_cmd = true;
                e.buffer.push(cell.c);
                e.buffer.extend(cell.zerowidth().into_iter().flatten());
            } else {
                pad += 1;
                e.shell_ghost |= cmd;
            }
        }
        if r == cl && cc >= cols {
            at_cursor(&mut e, &mut pad, line_has_cmd);
        }
        if r < end && !wraps(r) && (row_has_cmd || !e.buffer.is_empty()) {
            e.buffer.push('\n');
        }
    }
    e.cursor = cursor.unwrap_or(e.buffer.len());
    if pwsh {
        strip_pwsh_prompts(context, &mut e);
    }
    // Blank command cells after the cursor are drawing leftovers.
    let keep = e.buffer.trim_end().len().max(e.cursor);
    e.buffer.truncate(keep);
    Some(EditBuffer::new(e.buffer, e.cursor, e.shell_ghost, e.at_prompt))
}

/// PSReadLine draws its ContinuationPrompt (each line after a real `\n`) and a re-rendered PromptText (first
/// line) after EndPrompt, so they carry the command tag: drop them, keeping the cursor on the same character.
/// ponytail: a first line the user typed starting with the PromptText loses it too.
fn strip_pwsh_prompts(context: &EditBufferContext, e: &mut Scraped) {
    let (mut out, mut off, mut shift) = (String::with_capacity(e.buffer.len()), 0, 0);
    for (i, line) in e.buffer.split('\n').enumerate() {
        let p = if i == 0 {
            context
                .pwsh_prompt_text()
                .iter()
                .find(|p| line.starts_with(p.as_str()))
                .map_or(0, |p| p.len())
        } else if !context.pwsh_continuation().is_empty()
            && line.starts_with(context.pwsh_continuation())
        {
            context.pwsh_continuation().len()
        } else {
            0
        };
        if i > 0 {
            out.push('\n');
        }
        out.push_str(&line[p..]);
        shift += p.min(e.cursor.saturating_sub(off));
        off += line.len() + 1;
    }
    e.buffer = out;
    e.cursor -= shift;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::shared::domain::value_objects::shell::Shell;
    use crate::modules::terminal_session::domain::value_objects::cell_tag::CellTag;
    use crate::modules::terminal_session::domain::value_objects::edit_buffer_context::{
        EditBufferContext, EditBufferContextParts,
    };
    use crate::modules::terminal_session::domain::value_objects::pwsh_prediction_color::PwshPredictionColor;
    use crate::modules::terminal_session::domain::value_objects::screen_command::ScreenCommand;
    use crate::modules::terminal_session::domain::value_objects::shell_suggestion_color::ShellSuggestionColor;
    use crate::modules::terminal_session::infrastructure::adapters::cell_tagger;
    use crate::modules::terminal_session::infrastructure::adapters::headless_model::Model;
    use alacritty_terminal::event::VoidListener;
    use alacritty_terminal::index::{Column, Line};
    use alacritty_terminal::term::test::TermSize;
    use alacritty_terminal::term::{Config, Term};
    use alacritty_terminal::vte::ansi::{Color, NamedColor, Processor};

    /// A model whose cells are written under the tag of command cycle 1.
    fn tagged_model(columns: usize) -> Model {
        let mut model = Term::new(Config::default(), &TermSize::new(columns, 3), VoidListener);
        cell_tagger::apply(&mut model, ScreenCommand::Tag(CellTag::Cycle(1)));
        model
    }

    fn write(model: &mut Model, text: &str) {
        let mut processor: Processor = Processor::new();
        processor.advance(model, text.as_bytes());
    }

    fn parts<'a>(shell: Shell) -> EditBufferContextParts<'a> {
        EditBufferContextParts {
            cycle: 1,
            shell: Some(shell),
            line_col0: false,
            pwsh_list_view: false,
            pwsh_continuation: ">> ",
            pwsh_prompt_text: &[],
            zsh_color: None,
            fish_color: None,
            pwsh_color: None,
        }
    }

    /// What `extract` read: the buffer, the cursor offset and whether the shell draws a suggestion of its own.
    fn scrape(model: &Model, parts: EditBufferContextParts<'_>) -> (String, usize, bool) {
        let scraped = extract(model, &EditBufferContext::new(parts)).expect("a cycle is open");
        (
            scraped.buffer().to_string(),
            scraped.cursor(),
            scraped.shell_ghost(),
        )
    }

    #[test]
    fn should_treat_a_cell_with_only_one_of_the_required_flags_as_typed_text() {
        let reported = PwshPredictionColor::parse("[2;3;38;5;8m").unwrap();
        let mut model = tagged_model(10);
        write(&mut model, "ab\x1b[2;38;5;8mc\x1b[0m\x1b[3;38;5;8md\x1b[0m\x1b[2D");

        let scraped = scrape(
            &model,
            EditBufferContextParts {
                pwsh_color: Some(&reported),
                ..parts(Shell::Pwsh)
            },
        );

        assert_eq!(scraped, ("abcd".to_string(), 2, false));
    }

    #[test]
    fn should_treat_a_dim_or_italic_cell_as_the_shell_suggestion_without_a_reported_prediction_colour() {
        let mut model = tagged_model(10);
        write(&mut model, "ab\x1b[2mc\x1b[0m\x1b[3md\x1b[0m\x1b[2D");

        let scraped = scrape(&model, parts(Shell::Pwsh));

        assert_eq!(scraped, ("ab".to_string(), 2, true));
    }

    #[test]
    fn should_map_named_colours_above_fifteen_to_the_default_colour() {
        let default_colour = ShellSuggestionColor::parse_zsh("fg=default").unwrap();
        let mut model = tagged_model(10);
        write(&mut model, "\x1b[31mab\x1b[0mc\x1b[1D");
        model.grid_mut()[Line(0)][Column(2)].fg = Color::Named(NamedColor::BrightForeground);

        let scraped = scrape(
            &model,
            EditBufferContextParts {
                zsh_color: Some(&default_colour),
                ..parts(Shell::Zsh)
            },
        );

        assert_eq!(scraped, ("ab".to_string(), 2, true));
    }

    #[test]
    fn should_match_indexed_and_truecolour_cells_like_the_parsed_colour() {
        let grey = ShellSuggestionColor::parse_zsh("fg=245,fg=#aaaaaa").unwrap();
        let mut model = tagged_model(10);
        write(
            &mut model,
            "ab\x1b[38;5;245mc\x1b[38;2;170;170;170md\x1b[0m\x1b[2D",
        );

        let scraped = scrape(
            &model,
            EditBufferContextParts {
                zsh_color: Some(&grey),
                ..parts(Shell::Zsh)
            },
        );

        assert_eq!(scraped, ("ab".to_string(), 2, true));
    }

    #[test]
    fn should_count_the_pending_wrap_cursor_as_past_the_last_column() {
        let mut model = tagged_model(4);
        write(&mut model, "abcd");

        let scraped = scrape(&model, parts(Shell::Bash));

        assert_eq!(scraped, ("abcd".to_string(), 4, false));
    }

    #[test]
    fn suggestion_colors_cell_matching() {
        let z = ShellSuggestionColor::parse_zsh("fg=8,bold").unwrap();
        // Cell colours: named, indexed and truecolor all map to xterm-256.
        let mut cell = Cell {
            fg: Color::Named(alacritty_terminal::vte::ansi::NamedColor::BrightBlack),
            ..Cell::default()
        };
        assert!(matches_cell(&z, &cell));
        cell.fg = Color::Spec(alacritty_terminal::vte::ansi::Rgb {
            r: 0xaa,
            g: 0xaa,
            b: 0xaa,
        });
        assert!(matches_cell(
            &ShellSuggestionColor::parse_zsh("fg=#aaaaaa").unwrap(),
            &cell
        ));
        assert!(!matches_cell(&z, &cell));
    }
}
