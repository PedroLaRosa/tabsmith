/// Bytes-level description of an insertion (SPEC §6.8, §7.3). Counts are in chars.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct Insertion {
    /// Chars to delete before the cursor (sent as 0x7f each).
    pub(crate) delete: usize,
    /// Text to type (no trailing "\n"; `{cursor}` already removed).
    pub(crate) text: String,
    /// Chars to move the cursor left after typing (`ESC [ D` each).
    pub(crate) cursor_back: usize,
    /// Send "\r" afterwards.
    pub(crate) execute: bool,
}
