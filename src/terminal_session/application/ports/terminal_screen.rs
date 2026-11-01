use crate::dropdown::domain::entities::menu::Menu;
use crate::dropdown::domain::value_objects::theme::Theme;
use crate::terminal_session::application::frame_input::FrameInput;
use crate::terminal_session::domain::value_objects::edit_buffer::EditBuffer;
use crate::terminal_session::domain::value_objects::edit_buffer_context::EditBufferContext;
use crate::terminal_session::domain::value_objects::screen_command::ScreenCommand;
use crate::terminal_session::domain::value_objects::screen_state::ScreenState;

/// The wrapper's mirror of the real terminal: the headless model it feeds with the shell's output, the filter in
/// front of the real terminal, and the overlay drawn over both. One instance per session, single thread
/// (`Rc<dyn _>`); every method takes `&self` (the adapter keeps its state behind a `RefCell`). None of them fails.
/// A method never calls another one of the port.
pub(crate) trait TerminalScreen {
    /// Plain shell output at its place in the stream: the model first (stopping any synchronized update, so the
    /// model never lags), then the filter, which APPENDS what may go to the real terminal to `out`.
    fn pass_through(&self, bytes: &[u8], out: &mut Vec<u8>);

    /// A screen window name (`ESC k .. ST`): for the real terminal only (tmux names its window), never the model.
    fn pass_window_name_through(&self, name: &[u8], out: &mut Vec<u8>);

    /// Cell tagging, model only; nothing reaches the real terminal. Commands apply in the order given, each before
    /// the next byte is advanced.
    fn apply(&self, command: ScreenCommand);

    /// Column of the cursor in the model (0-based); a pending wrap is NOT counted as past the last column here.
    fn cursor_column(&self) -> usize;

    /// The command line in the model (SPEC 5.6); `None` while `context` has no open cycle. The caller filters on
    /// `at_prompt()` and the alternate screen as it does today.
    fn edit_buffer(&self, context: &EditBufferContext<'_>) -> Option<EditBuffer>;

    /// A fresh snapshot of what the render guards need; never cache it across a `pass_through`.
    fn state(&self) -> ScreenState;

    /// Terminal resized: the model reflows, the filter forgets its scroll region, the cached clear is dropped (the
    /// real terminal reflowed the old frame unpredictably). The three always go together, in this order.
    fn resize(&self, columns: usize, lines: usize);

    /// The overlay for the current state (SPEC 7.5.2): the clear of a still-drawn frame, the frame, or nothing when
    /// identical to the last one.
    fn draw_overlay(&self, menu: &Menu, input: &FrameInput<'_>) -> Vec<u8>;

    /// Bytes that restore every cell the last frame covered; empties the cache. After shell output take it LATE (a
    /// panic before it leaves the clear cached for the driver's guard).
    fn take_clear(&self) -> Vec<u8>;

    /// Repaint of the whole model after a resize settled (drops the cached clear first).
    fn full_repaint(&self) -> Vec<u8>;

    /// UNCONDITIONAL (also for an equal theme): forgets the last frame so the next draw is not skipped.
    fn set_theme(&self, theme: Theme);
}
