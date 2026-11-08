#[derive(Debug, PartialEq)]
pub(crate) enum OutputPiece {
    Bytes(Vec<u8>),
    /// OSC 697 payload (everything after `697;`, terminator stripped).
    Marker(String),
    /// screen's window name (`ESC k … ESC \`), raw, for the terminal only: tmux takes it as the window name, the
    /// model (no `ESC k` in alacritty) would print its text.
    WindowName(Vec<u8>),
}
