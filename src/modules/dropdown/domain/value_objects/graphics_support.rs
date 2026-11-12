/// How row icons reach the terminal.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum GraphicsSupport {
    /// ASCII icons.
    Off,
    /// Kitty graphics placements under the text (kitty, Ghostty).
    Direct,
    /// Kitty or Ghostty behind tmux: images go through tmux's passthrough as virtual placements and the slot holds
    /// kitty Unicode placeholders, which tmux keeps, redraws and clears like any text.
    Tmux,
}
