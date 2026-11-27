//! The colour of each row icon, so a terminal without images can still show it as a coloured tile.

use crate::modules::dropdown::domain::value_objects::rgb::Rgb;

/// Icon name → its colour: the fill of the PNG's square.
const PALETTE: &[(&str, Rgb)] = &[("git", (0xff, 0x40, 0x1e))];

/// The colour of the icon called `name`, if it has one.
pub(crate) fn icon_color(name: &str) -> Option<Rgb> {
    PALETTE
        .iter()
        .find(|(icon, _)| *icon == name)
        .map(|(_, colour)| *colour)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn should_give_the_git_icon_the_red_of_its_square() {
        let colour = icon_color("git");

        assert_eq!(colour, Some((0xff, 0x40, 0x1e)));
    }
}
