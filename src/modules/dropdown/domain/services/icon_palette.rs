//! The colour of each row icon, so a terminal without images can still show it as a coloured tile.

use crate::modules::dropdown::domain::value_objects::rgb::Rgb;

/// Icon name → its colour: the fill of the PNG's square. `file` and `folder` have no square: slate and blue.
const PALETTE: &[(&str, Rgb)] = &[
    ("alert", (0xff, 0xc5, 0x28)),
    ("android", (0xa4, 0xc6, 0x39)),
    ("apple", (0xa6, 0xb1, 0xb7)),
    ("asterisk", (0x39, 0x41, 0xd1)),
    ("aws", (0xff, 0x9c, 0x07)),
    ("azure", (0x00, 0x89, 0xd6)),
    ("box", (0x64, 0x9b, 0xc9)),
    ("carrot", (0xd1, 0x39, 0x56)),
    ("characters", (0x1e, 0x88, 0xff)),
    ("command", (0x88, 0x39, 0xd1)),
    ("commandkey", (0xd1, 0x39, 0xaf)),
    ("commit", (0xff, 0x40, 0x1e)),
    ("cpu", (0x2e, 0x32, 0x2d)),
    ("database", (0xd1, 0x6b, 0x39)),
    ("discord", (0x72, 0x89, 0xda)),
    ("docker", (0x00, 0x93, 0xd7)),
    ("file", (0x64, 0x74, 0x8b)),
    ("firebase", (0xff, 0xff, 0xff)),
    ("flag", (0x39, 0x41, 0xd1)),
    ("folder", (0x3b, 0x82, 0xf6)),
    ("gcloud", (0xff, 0xff, 0xff)),
    ("gear", (0x3b, 0x3b, 0x3b)),
    ("git", (0xff, 0x40, 0x1e)),
    ("github", (0x16, 0x16, 0x14)),
    ("gitlab", (0xff, 0xff, 0xff)),
    ("gradle", (0x03, 0x30, 0x3a)),
    ("heroku", (0x8d, 0x6e, 0xb2)),
    ("invite", (0x28, 0xa6, 0xff)),
    ("kubernetes", (0x32, 0x6c, 0xe5)),
    ("netlify", (0x4d, 0x9a, 0xbf)),
    ("node", (0xff, 0x40, 0x1e)),
    ("npm", (0xcb, 0x00, 0x00)),
    ("okteto", (0x00, 0xd1, 0xc9)),
    ("option", (0x5a, 0xcc, 0x5b)),
    ("package", (0xd1, 0x39, 0x39)),
    ("slack", (0xff, 0xff, 0xff)),
    ("string", (0x39, 0xcf, 0xd1)),
    ("symlink", (0xff, 0xff, 0xff)),
    ("template", (0xff, 0xff, 0xff)),
    ("twitter", (0x1f, 0x90, 0xd6)),
    ("vercel", (0x00, 0x00, 0x00)),
    ("yarn", (0x2c, 0x8e, 0xbb)),
];

/// The colour of the icon called `name`, if it has one.
pub(crate) fn icon_color(name: &str) -> Option<Rgb> {
    PALETTE
        .iter()
        .find(|(icon, _)| *icon == name)
        .map(|(_, colour)| *colour)
}

/// The colour of a character written on a `tile`: black on a light tile, white on a dark one, by the tile's perceived
/// brightness (ITU-R BT.601 weights, 0–255).
pub(crate) fn ink_on((r, g, b): Rgb) -> Rgb {
    let brightness = (299 * u32::from(r) + 587 * u32::from(g) + 114 * u32::from(b)) / 1000;
    if brightness > 160 {
        (0, 0, 0)
    } else {
        (0xff, 0xff, 0xff)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::dropdown::domain::value_objects::icon_id::ICON_NAMES;

    #[test]
    fn should_give_the_git_icon_the_red_of_its_square() {
        let colour = icon_color("git");

        assert_eq!(colour, Some((0xff, 0x40, 0x1e)));
    }

    #[test]
    fn should_write_black_on_a_light_tile() {
        let pnpm_orange = (0xf6, 0x92, 0x20);

        let ink = ink_on(pnpm_orange);

        assert_eq!(ink, (0, 0, 0));
    }

    #[test]
    fn should_write_white_on_a_dark_tile() {
        let git_red = (0xff, 0x40, 0x1e);

        let ink = ink_on(git_red);

        assert_eq!(ink, (0xff, 0xff, 0xff));
    }

    #[test]
    fn should_switch_to_black_ink_only_above_a_brightness_of_160() {
        let greys = [(160, 160, 160), (161, 161, 161)];

        let inks = greys.map(ink_on);

        assert_eq!(inks, [(0xff, 0xff, 0xff), (0, 0, 0)]);
    }

    #[test]
    fn should_give_every_icon_a_colour() {
        let uncoloured: Vec<_> = ICON_NAMES.iter().filter(|name| icon_color(name).is_none()).collect();

        assert_eq!(uncoloured, Vec::<&&str>::new());
    }
}
