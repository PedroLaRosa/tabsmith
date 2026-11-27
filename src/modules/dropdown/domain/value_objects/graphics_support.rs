/// How row icons reach the terminal.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum GraphicsSupport {
    /// No images: the row type's character on a tile of the icon's colour.
    Off,
    /// Kitty graphics placements under the text (kitty, Ghostty).
    Direct,
    /// Kitty or Ghostty behind tmux: images go through tmux's passthrough as virtual placements and the slot holds
    /// kitty Unicode placeholders, which tmux keeps, redraws and clears like any text.
    Tmux,
}
