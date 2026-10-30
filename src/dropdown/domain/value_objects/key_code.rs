#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum KeyCode {
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
