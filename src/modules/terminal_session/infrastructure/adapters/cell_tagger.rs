use crate::modules::terminal_session::domain::value_objects::cell_tag::CellTag;
use crate::modules::terminal_session::domain::value_objects::screen_command::ScreenCommand;
use crate::modules::terminal_session::infrastructure::adapters::headless_model::Model;
use alacritty_terminal::grid::Dimensions;
use alacritty_terminal::index::Column;
use alacritty_terminal::term::cell::Flags;
use alacritty_terminal::vte::ansi::{Handler, Hyperlink};

/// Execute one screen command on the model, the way the wrapper received it.
pub(crate) fn apply(model: &mut Model, command: ScreenCommand) {
    match command {
        ScreenCommand::Tag(cell_tag) => tag(model, Some(cell_tag.uri())),
        ScreenCommand::ClearTag => tag(model, None),
        ScreenCommand::MarkLineStart { redrawn_cycle } => mark_line_start(
            model,
            redrawn_cycle.map(|n| CellTag::Cycle(n).uri()).as_deref(),
        ),
    }
}

/// Inject a cell tag into the model only (OSC 8 hyperlink on the cursor template; SPEC D3).
fn tag(model: &mut Model, uri: Option<String>) {
    model.set_hyperlink(uri.map(|uri| Hyperlink { id: None, uri }));
}

/// NewCmd at the cursor: the line starts here. Before it on its row is prompt: untagged cells too (a prompt drawn
/// without markers: pwsh re-wraps a foreign prompt at ReadLine) and, on a redraw, the open cycle's (`from`; readline
/// redraws only a multi-line prompt's last line, without its StartPrompt). A redraw of the prompt alone keeps the
/// line's cells after it (bash readline: CUF over a vi-mode switch, DCH at the end of an i-search): `from` cells at
/// and after the cursor (its line and wrap continuations) get the cursor template's tag.
fn mark_line_start(model: &mut Model, from: Option<&str>) {
    let prompt = Some(alacritty_terminal::term::cell::Hyperlink::new(
        None::<String>,
        CellTag::PROMPT_URI.into(),
    ));
    let link = model.grid().cursor.template.hyperlink();
    let (cols, bottom) = (model.grid().columns(), model.grid().bottommost_line());
    let (mut line, mut split) = (
        model.grid().cursor.point.line,
        model.grid().cursor.point.column.0,
    );
    loop {
        let row = &mut model.grid_mut()[line];
        for c in 0..cols {
            let cell = &mut row[Column(c)];
            let old = cell.hyperlink();
            let ours = from.is_some_and(|f| old.as_ref().is_some_and(|h| h.uri() == f));
            if c < split && (ours || old.is_none()) {
                cell.set_hyperlink(prompt.clone());
            } else if ours {
                cell.set_hyperlink(link.clone());
            }
        }
        if from.is_none()
            || line >= bottom
            || !row[Column(cols - 1)].flags.contains(Flags::WRAPLINE)
        {
            break;
        }
        (line, split) = (line + 1, 0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alacritty_terminal::event::VoidListener;
    use alacritty_terminal::index::Line;
    use alacritty_terminal::term::test::TermSize;
    use alacritty_terminal::term::{Config, Term};
    use alacritty_terminal::vte::ansi::Processor;

    fn model(columns: usize) -> Model {
        Term::new(Config::default(), &TermSize::new(columns, 3), VoidListener)
    }

    fn write(model: &mut Model, text: &str) {
        let mut processor: Processor = Processor::new();
        processor.advance(model, text.as_bytes());
    }

    /// The hyperlink uri of every cell of the row, `-` for an untagged cell.
    fn row_tags(model: &Model, row: i32) -> Vec<String> {
        (0..model.columns())
            .map(|column| {
                model.grid()[Line(row)][Column(column)]
                    .hyperlink()
                    .map_or("-".to_string(), |link| link.uri().to_string())
            })
            .collect()
    }

    fn uris(expected: &[&str]) -> Vec<String> {
        expected.iter().map(|uri| uri.to_string()).collect()
    }

    #[test]
    fn should_tag_the_cells_written_after_a_tag_command() {
        let mut model = model(4);
        apply(&mut model, ScreenCommand::Tag(CellTag::Cycle(1)));

        write(&mut model, "ab");

        assert_eq!(
            row_tags(&model, 0),
            ["tabsmith:c1", "tabsmith:c1", "-", "-"]
        );
    }

    #[test]
    fn should_stop_tagging_after_a_clear_tag_command() {
        let mut model = model(4);
        apply(&mut model, ScreenCommand::Tag(CellTag::Prompt));
        write(&mut model, "a");

        apply(&mut model, ScreenCommand::ClearTag);
        write(&mut model, "b");

        assert_eq!(row_tags(&model, 0), [CellTag::PROMPT_URI, "-", "-", "-"]);
    }

    #[test]
    fn should_turn_the_untagged_cells_before_the_cursor_into_the_prompt_on_mark_line_start() {
        let mut model = model(6);
        write(&mut model, "$ abc\x1b[3D");

        apply(
            &mut model,
            ScreenCommand::MarkLineStart {
                redrawn_cycle: None,
            },
        );

        assert_eq!(
            row_tags(&model, 0),
            [CellTag::PROMPT_URI, CellTag::PROMPT_URI, "-", "-", "-", "-"]
        );
    }

    #[test]
    fn should_retag_the_redrawn_cycle_cells_from_the_cursor_on_mark_line_start() {
        let mut model = model(6);
        apply(&mut model, ScreenCommand::Tag(CellTag::Cycle(3)));
        write(&mut model, "$ cmd\x1b[3D");
        apply(&mut model, ScreenCommand::Tag(CellTag::Cycle(4)));

        apply(
            &mut model,
            ScreenCommand::MarkLineStart {
                redrawn_cycle: Some(3),
            },
        );

        assert_eq!(
            row_tags(&model, 0),
            [
                CellTag::PROMPT_URI,
                CellTag::PROMPT_URI,
                "tabsmith:c4",
                "tabsmith:c4",
                "tabsmith:c4",
                "-"
            ]
        );
    }

    #[test]
    fn should_follow_wrapped_rows_only_when_a_redrawn_cycle_is_given() {
        let tagged_wrapped_line = || {
            let mut model = model(4);
            apply(&mut model, ScreenCommand::Tag(CellTag::Cycle(3)));
            write(&mut model, "abcdef\x1b[1;3H");
            apply(&mut model, ScreenCommand::Tag(CellTag::Cycle(4)));
            model
        };
        let mut redrawn = tagged_wrapped_line();
        let mut not_redrawn = tagged_wrapped_line();

        apply(
            &mut redrawn,
            ScreenCommand::MarkLineStart {
                redrawn_cycle: Some(3),
            },
        );
        apply(
            &mut not_redrawn,
            ScreenCommand::MarkLineStart {
                redrawn_cycle: None,
            },
        );

        assert_eq!(
            (row_tags(&redrawn, 1), row_tags(&not_redrawn, 1)),
            (
                uris(&["tabsmith:c4", "tabsmith:c4", "-", "-"]),
                uris(&["tabsmith:c3", "tabsmith:c3", "-", "-"])
            )
        );
    }
}
