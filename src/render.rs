//! Themes, layout and frame bytes for the in-terminal overlay: menu, description bar, pop-out, ghost text, and
//! exact restoration of the covered cells from the model (SPEC §7.4–7.7, §8.2). OWNER: ui agent.
use crate::menu::Menu;
use crate::settings::Settings;
use crate::types::Model;

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

impl Theme {
    pub fn dark() -> Theme {
        todo!("ui agent")
    }

    pub fn light() -> Theme {
        todo!("ui agent")
    }

    /// From `autocomplete.theme` (dark/light/system/custom file). `terminal_bg` = OSC 11 reply (for `system`).
    pub fn from_settings(settings: &Settings, terminal_bg: Option<(u8, u8, u8)>) -> Theme {
        let _ = (settings, terminal_bg);
        todo!("ui agent")
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
    // ui agent: theme, truecolor flag, cached clear bytes, last render key, …
    _private: (),
}

impl Renderer {
    /// `truecolor` = `COLORTERM ∈ {truecolor, 24bit}` or `WT_SESSION` set.
    pub fn new(theme: Theme, truecolor: bool) -> Renderer {
        let _ = (theme, truecolor);
        todo!("ui agent")
    }

    pub fn set_theme(&mut self, theme: Theme) {
        let _ = theme;
        todo!("ui agent")
    }

    /// Bytes restoring every cell span the last frame covered; empties the cache (SPEC §7.5.3).
    pub fn take_clear(&mut self) -> Vec<u8> {
        todo!("ui agent")
    }

    /// Forget the cached clear without emitting it (after resize: the terminal reflowed the old frame).
    pub fn drop_clear(&mut self) {
        todo!("ui agent")
    }

    /// Frame for the current state (SPEC §7.5.2), assuming the screen equals the model (call `take_clear` first).
    /// Empty when nothing is drawn. Stores the matching clear for the next `take_clear`.
    pub fn frame(&mut self, model: &Model, menu: &Menu, input: &FrameInput) -> Vec<u8> {
        let _ = (model, menu, input);
        todo!("ui agent")
    }

    /// Full repaint of the model (after resize, SPEC §7.6).
    pub fn full_repaint(&mut self, model: &Model) -> Vec<u8> {
        let _ = model;
        todo!("ui agent")
    }
}
