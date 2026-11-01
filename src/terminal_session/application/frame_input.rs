use crate::settings::domain::value_objects::settings::Settings;
use std::time::Instant;

/// Inputs to a frame besides the model and menu.
pub(crate) struct FrameInput<'a> {
    pub(crate) settings: &'a Settings,
    /// Ghost text to draw at the cursor (already decided by the wrapper), if any.
    pub(crate) ghost: Option<&'a str>,
    /// Spinner frame counter (advanced every 80 ms by the wrapper while loading).
    pub(crate) spinner: usize,
    /// Wrapper-level suppression (preexec, resize repaint pending, insertion lock …).
    pub(crate) suppress: bool,
    /// Non-full scroll region active (tracked by the wrapper; alacritty keeps it private).
    pub(crate) scroll_region: bool,
    /// The instant the spinner is judged at (the menu shows it after 200 ms of loading).
    pub(crate) now: Instant,
}
