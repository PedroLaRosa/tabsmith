//! Dropdown UI state: visibility machine, selection/scroll, actions, common prefix (SPEC §7.1–7.3).
//! Pure: no I/O. OWNER: ui agent. Driven by the wrapper (pty.rs); read by render.rs.
use crate::keys::Action;
use crate::settings::Settings;
use crate::types::{Completion, Insertion, Item};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Vis {
    Visible,
    HiddenUntilKeypress,
    HiddenUntilShown,
    HiddenByInsertion,
}

/// What the wrapper must do after an action.
#[derive(Clone, Debug, PartialEq)]
pub enum Effect {
    /// Write an insertion to the PTY (with buffer reconciliation + insertion lock, SPEC §7.3).
    /// `full` = a full insertion (visibility HiddenByInsertion already applied); `record` = (cmd, name) for recency.
    Insert { ins: Insertion, full: bool, record: Option<(String, String)> },
    /// Write raw bytes to the PTY (e.g. "\r" for `execute`).
    Write(Vec<u8>),
    /// Ring the bell (common-prefix failure "shake").
    Bell,
    /// Re-request completions for the current buffer (history mode toggled).
    Rerequest,
    /// Toggle fuzzy search in the engine, then re-request.
    ToggleFuzzy,
}

pub struct Menu {
    // ui agent: vis, items, selected, sticky, scroll_top, last completion, last_inserted, just_inserted,
    // history_mode, popout, loading_since, …
    _private: (),
}

impl Menu {
    pub fn new() -> Menu {
        todo!("ui agent")
    }

    /// Phase-Parse / phase-Generators reply for the latest request (SPEC §7.1).
    pub fn on_completion(&mut self, c: Completion, settings: &Settings) {
        let _ = (c, settings);
        todo!("ui agent")
    }

    /// Error reset: no items, HiddenUntilKeypress, history mode off (SPEC §7.1).
    pub fn error_reset(&mut self) {
        todo!("ui agent")
    }

    /// Menu drawn? (`Visible` and items or arg hint, and not `autocomplete.disable`.)
    pub fn shown(&self, settings: &Settings) -> bool {
        let _ = settings;
        todo!("ui agent")
    }

    /// Keys are intercepted (shown and items non-empty, SPEC §7.2.4).
    pub fn intercepting(&self, settings: &Settings) -> bool {
        let _ = settings;
        todo!("ui agent")
    }

    pub fn has_items(&self) -> bool {
        todo!("ui agent")
    }

    /// Run an action (SPEC §7.2.5). `tab_preamble` = the key was Tab with `onlyShowOnTab` while not visible.
    pub fn handle(&mut self, action: Action, settings: &Settings) -> Vec<Effect> {
        let _ = (action, settings);
        todo!("ui agent")
    }

    /// `onlyShowOnTab` Tab while hidden: 1 item → insert it, else show (SPEC §7.2.5 preamble).
    pub fn show_on_tab(&mut self, settings: &Settings) -> Vec<Effect> {
        let _ = settings;
        todo!("ui agent")
    }

    /// Ctrl-C / Ctrl-D / new prompt etc. handled by the wrapper via `error_reset`; this marks a backspace whose
    /// echo is pending (keeps a visible menu up until the echo arrives).
    pub fn note_backspace(&mut self) {
        todo!("ui agent")
    }

    pub fn history_mode(&self) -> bool {
        todo!("ui agent")
    }

    /// Latest completion applied (tokens, current arg, buffer it was computed from).
    pub fn last(&self) -> Option<&Completion> {
        todo!("ui agent")
    }

    pub fn items(&self) -> &[Item] {
        todo!("ui agent")
    }

    pub fn selected(&self) -> usize {
        todo!("ui agent")
    }

    pub fn scroll_top(&self) -> usize {
        todo!("ui agent")
    }

    pub fn vis(&self) -> Vis {
        todo!("ui agent")
    }

    /// Description pop-out open (or `alwaysShowDescription`).
    pub fn popout(&self, settings: &Settings) -> bool {
        let _ = settings;
        todo!("ui agent")
    }

    /// Some generator has been loading for > 200 ms (spinner).
    pub fn loading(&self) -> bool {
        todo!("ui agent")
    }
}
