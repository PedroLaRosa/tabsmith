use crate::dropdown::domain::entities::menu::Menu;
use crate::dropdown::domain::value_objects::theme::Theme;
use crate::terminal_session::application::frame_input::FrameInput;
use crate::terminal_session::application::ports::terminal_screen::TerminalScreen;
use crate::terminal_session::domain::value_objects::edit_buffer::EditBuffer;
use crate::terminal_session::domain::value_objects::edit_buffer_context::EditBufferContext;
use crate::terminal_session::domain::value_objects::screen_command::ScreenCommand;
use crate::terminal_session::domain::value_objects::screen_state::{ScreenState, ScreenStateParts};
use crate::terminal_session::infrastructure::adapters::cell_tagger;
use crate::terminal_session::infrastructure::adapters::edit_buffer_extractor;
use crate::terminal_session::infrastructure::adapters::headless_model::Model;
use crate::terminal_session::infrastructure::adapters::output_filter::OutputFilter;
use crate::terminal_session::infrastructure::adapters::overlay_renderer::OverlayRenderer;
use alacritty_terminal::event::VoidListener;
use alacritty_terminal::grid::Dimensions;
use alacritty_terminal::term::test::TermSize;
use alacritty_terminal::term::{Config, Term, TermMode};
use alacritty_terminal::vte::ansi::Processor;
use std::cell::RefCell;

/// Lines of scrollback the model keeps in production.
const SCROLLBACK_LINES: usize = 2000;

/// The headless model of the real terminal, the parser that feeds it, the filter in front of the real terminal and
/// the session's one overlay renderer, behind one `RefCell`: every port method takes a single borrow and calls no
/// other method of the port.
pub(crate) struct AlacrittyTerminalScreen {
    inner: RefCell<Inner>,
}

struct Inner {
    term: Model,
    processor: Processor,
    filter: OutputFilter,
    renderer: OverlayRenderer,
}

/// Feed bytes to the model only. Synchronized updates are applied at once: the model mirrors terminal state.
fn advance(processor: &mut Processor, term: &mut Model, bytes: &[u8]) {
    processor.advance(term, bytes);
    if processor.sync_timeout().sync_timeout().is_some() {
        processor.stop_sync(term);
    }
}

impl AlacrittyTerminalScreen {
    /// `renderer` is the session's one renderer; it lives as long as the screen.
    pub(crate) fn new(
        columns: usize,
        lines: usize,
        renderer: OverlayRenderer,
        lf_to_ind: bool,
    ) -> Self {
        let config = Config {
            scrolling_history: SCROLLBACK_LINES,
            ..Config::default()
        };
        Self::with_config(config, columns, lines, renderer, lf_to_ind)
    }

    fn with_config(
        config: Config,
        columns: usize,
        lines: usize,
        renderer: OverlayRenderer,
        lf_to_ind: bool,
    ) -> Self {
        let mut filter = OutputFilter::new(lines);
        filter.lf_to_ind = lf_to_ind;
        AlacrittyTerminalScreen {
            inner: RefCell::new(Inner {
                term: Term::new(config, &TermSize::new(columns, lines), VoidListener),
                processor: Processor::new(),
                filter,
                renderer,
            }),
        }
    }

    /// Start-up only (the cursor-position sync): the model, through the same long-lived parser, and nothing for the
    /// real terminal.
    pub(crate) fn advance_model(&self, bytes: &[u8]) {
        let Inner {
            term, processor, ..
        } = &mut *self.inner.borrow_mut();
        advance(processor, term, bytes);
    }

    /// The environment of the tests: the default scrollback, the dark theme, no truecolor, no LF rewriting.
    #[cfg(test)]
    pub(crate) fn for_tests(columns: usize, lines: usize) -> Self {
        let renderer = OverlayRenderer::new(Theme::dark(), false);
        Self::with_config(Config::default(), columns, lines, renderer, false)
    }

    #[cfg(test)]
    pub(crate) fn history_size(&self) -> usize {
        self.inner.borrow().term.grid().history_size()
    }

    #[cfg(test)]
    pub(crate) fn model(&self) -> std::cell::Ref<'_, Model> {
        std::cell::Ref::map(self.inner.borrow(), |inner| &inner.term)
    }

    /// Test hook: fail inside a borrowed section, as an unexpected panic in the overlay would.
    #[cfg(test)]
    pub(crate) fn panic_while_borrowed(&self) {
        let _inner = self.inner.borrow_mut();
        panic!("test: panic inside a borrowed section");
    }
}

impl TerminalScreen for AlacrittyTerminalScreen {
    fn pass_through(&self, bytes: &[u8], out: &mut Vec<u8>) {
        let Inner {
            term,
            processor,
            filter,
            ..
        } = &mut *self.inner.borrow_mut();
        advance(processor, term, bytes);
        filter.feed(bytes, out);
    }

    fn pass_window_name_through(&self, name: &[u8], out: &mut Vec<u8>) {
        self.inner.borrow_mut().filter.feed(name, out);
    }

    fn apply(&self, command: ScreenCommand) {
        cell_tagger::apply(&mut self.inner.borrow_mut().term, command);
    }

    fn cursor_column(&self) -> usize {
        self.inner.borrow().term.grid().cursor.point.column.0
    }

    fn edit_buffer(&self, context: &EditBufferContext<'_>) -> Option<EditBuffer> {
        edit_buffer_extractor::extract(&self.inner.borrow().term, context)
    }

    fn state(&self) -> ScreenState {
        let Inner { term, filter, .. } = &*self.inner.borrow();
        let grid = term.grid();
        ScreenState::new(ScreenStateParts {
            alt_screen: term.mode().contains(TermMode::ALT_SCREEN),
            origin_mode: term.mode().contains(TermMode::ORIGIN),
            insert_mode: term.mode().contains(TermMode::INSERT),
            pending_wrap: grid.cursor.input_needs_wrap,
            columns: grid.columns(),
            lines: grid.screen_lines(),
            scroll_region: filter.scroll_region(),
            in_string: filter.in_string(),
        })
    }

    fn resize(&self, columns: usize, lines: usize) {
        let Inner {
            term,
            filter,
            renderer,
            ..
        } = &mut *self.inner.borrow_mut();
        term.resize(TermSize::new(columns, lines));
        filter.resize(lines);
        renderer.drop_clear(); // the terminal reflowed the old frame unpredictably
    }

    fn draw_overlay(&self, menu: &Menu, input: &FrameInput<'_>) -> Vec<u8> {
        let Inner { term, renderer, .. } = &mut *self.inner.borrow_mut();
        renderer.frame(term, menu, input)
    }

    fn take_clear(&self) -> Vec<u8> {
        self.inner.borrow_mut().renderer.take_clear()
    }

    fn full_repaint(&self) -> Vec<u8> {
        let Inner { term, renderer, .. } = &mut *self.inner.borrow_mut();
        renderer.full_repaint(term)
    }

    fn set_theme(&self, theme: Theme) {
        self.inner.borrow_mut().renderer.set_theme(theme);
    }
}

#[cfg(test)]
mod tests;
