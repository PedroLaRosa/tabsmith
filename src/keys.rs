//! Stdin key decoding, keystroke grammar and the keybinding table (SPEC §7.2.1–7.2.3).
//! OWNER: ui agent. Used by the wrapper (pty.rs) to decide interception.
use crate::settings::Settings;
use std::ops::Range;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum KeyCode {
    Char(char),
    Enter,
    Tab,
    Backspace,
    Esc,
    Up,
    Down,
    Left,
    Right,
    Home,
    End,
    PageUp,
    PageDown,
    Delete,
    Insert,
    F(u8),
    /// Bracketed paste `ESC[200~ … ESC[201~` (never intercepted).
    Paste,
    /// Cursor position report `ESC [ r ; c R` (forwarded untouched).
    CursorReport,
    /// Anything else (forwarded untouched).
    Unknown,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub struct Mods {
    pub ctrl: bool,
    pub alt: bool,
    pub shift: bool,
    pub meta: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Key {
    pub code: KeyCode,
    pub mods: Mods,
}

/// A decoded key and the byte range of the chunk it came from.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KeyEvent {
    pub key: Key,
    pub bytes: Range<usize>,
}

/// Split a stdin chunk into keys (SPEC §7.2.1). Every byte belongs to exactly one event.
pub fn decode(chunk: &[u8]) -> Vec<KeyEvent> {
    let _ = chunk;
    todo!("ui agent")
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Action {
    InsertSelected,
    InsertCommonPrefix,
    InsertCommonPrefixOrNavigateDown,
    InsertCommonPrefixOrInsertSelected,
    InsertSelectedAndExecute,
    Execute,
    NavigateUp,
    NavigateDown,
    HideAutocomplete,
    ShowAutocomplete,
    ToggleAutocomplete,
    ToggleHistoryMode,
    ToggleDescription,
    ShowDescription,
    HideDescription,
    ToggleFuzzySearch,
    /// 1..=10
    SelectSuggestion(u8),
}

impl Action {
    /// Kiro action id (`insertSelected`, `selectSuggestion3`, …).
    pub fn from_id(id: &str) -> Option<Action> {
        let _ = id;
        todo!("ui agent")
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Binding {
    Action(Action),
    /// `"ignore"`: never intercept this key.
    Ignore,
    /// Unknown action id: swallow the key, do nothing (Kiro).
    Unknown,
}

/// Parse a keystroke like `ctrl+k`, `shift+tab`, `+`, `ctrl++`, `f12` (SPEC §7.2.2).
pub fn parse_keystroke(s: &str) -> Option<Key> {
    let _ = s;
    todo!("ui agent")
}

/// Default bindings (fig.json set, SPEC §7.2.3) overlaid with `autocomplete.keybindings.*`.
pub struct Keymap {
    bindings: Vec<(Key, Binding)>,
}

impl Keymap {
    pub fn from_settings(settings: &Settings) -> Keymap {
        let _ = settings;
        todo!("ui agent")
    }

    /// Binding for a decoded key (handles case-insensitive ctrl/alt letters and `ESC O x` arrows).
    pub fn lookup(&self, key: &Key) -> Option<Binding> {
        let _ = key;
        let _ = &self.bindings;
        todo!("ui agent")
    }
}
