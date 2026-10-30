/// Which row icon to show, as an id: the pixels belong to terminal_session's icon assets, never to the domain. Under tmux the
/// slot text also carries the id, in its placeholder cells.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum IconId {
    /// The PNG at this position of `ICON_NAMES`. Not validated: only the icon resolver builds one, from a position in that list.
    Png(usize),
    /// The template PNG multiplied by this colour.
    Tinted(u8, u8, u8),
}

impl IconId {
    /// The kitty image id, also the foreground colour of the tmux placeholders and the key of the images already sent: the
    /// PNG's position plus one, or `0x01RRGGBB` for a tint.
    pub(crate) fn value(self) -> u32 {
        match self {
            IconId::Png(index) => index as u32 + 1,
            IconId::Tinted(r, g, b) => {
                0x0100_0000 | u32::from(r) << 16 | u32::from(g) << 8 | u32::from(b)
            }
        }
    }
}

/// Row icon names plus the local-only `database`, `file`, `folder` and `symlink`. Order = image ids (1-based).
pub(crate) const ICON_NAMES: &[&str] = &[
    "alert",
    "android",
    "apple",
    "asterisk",
    "aws",
    "azure",
    "box",
    "carrot",
    "characters",
    "command",
    "commandkey",
    "commit",
    "cpu",
    "database",
    "discord",
    "docker",
    "file",
    "firebase",
    "flag",
    "folder",
    "gcloud",
    "gear",
    "git",
    "github",
    "gitlab",
    "gradle",
    "heroku",
    "invite",
    "kubernetes",
    "netlify",
    "node",
    "npm",
    "okteto",
    "option",
    "package",
    "slack",
    "string",
    "symlink",
    "template",
    "twitter",
    "vercel",
    "yarn",
];

#[cfg(test)]
mod tests {
    use super::*;

    /// The ids `slot()` gave at the phase 8 start: a PNG's position plus one (git 23, option 34, template 39), the top byte set for a tint.
    #[test]
    fn should_number_png_icons_from_one_and_tinted_icons_with_the_top_byte_set() {
        // Arrange
        let icons = [
            IconId::Png(0),
            IconId::Png(38),
            IconId::Png(33),
            IconId::Png(22),
            IconId::Tinted(0x2e, 0xcc, 0x71),
        ];

        // Act
        let values = icons.map(IconId::value);

        // Assert
        assert_eq!(values, [1, 39, 34, 23, 0x012e_cc71]);
    }
}
