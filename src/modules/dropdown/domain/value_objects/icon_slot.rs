use crate::modules::dropdown::domain::value_objects::icon_id::IconId;
use crate::modules::dropdown::domain::value_objects::rgb::Rgb;

/// The icon slot of a row: `text` is two columns wide for every row kind (a spec badge is stored as given); `image` (kitty only) covers it, under the text;
/// without images, `tile` is the colour the two cells are filled with.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct IconSlot {
    text: String,
    image: Option<IconId>,
    /// Badge text is drawn white.
    badge: bool,
    tile: Option<Rgb>,
}

impl IconSlot {
    pub(crate) fn new(text: String, image: Option<IconId>, badge: bool) -> Self {
        Self {
            text,
            image,
            badge,
            tile: None,
        }
    }

    /// Two cells of `text` on a tile of colour `tile`, for terminals without images.
    pub(crate) fn tile(text: String, tile: Rgb) -> Self {
        Self {
            text,
            image: None,
            badge: false,
            tile: Some(tile),
        }
    }

    pub(crate) fn text(&self) -> &str {
        &self.text
    }

    pub(crate) fn image(&self) -> Option<IconId> {
        self.image
    }

    pub(crate) fn is_badge(&self) -> bool {
        self.badge
    }

    pub(crate) fn tile_color(&self) -> Option<Rgb> {
        self.tile
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn should_hold_the_text_image_and_badge_of_an_icon_slot() {
        let icon = IconId::Tinted(0x2e, 0xcc, 0x71);

        let badge = IconSlot::new("ab".to_string(), Some(icon), true);
        let plain = IconSlot::new("  ".to_string(), None, false);

        assert_eq!(
            (badge.text(), badge.image(), badge.is_badge()),
            ("ab", Some(icon), true)
        );
        assert_eq!(
            (plain.text(), plain.image(), plain.is_badge()),
            ("  ", None, false)
        );
    }

    #[test]
    fn should_hold_the_text_and_colour_of_a_tile() {
        let git_red = (0xff, 0x40, 0x1e);

        let tile = IconSlot::tile(" >".to_string(), git_red);

        assert_eq!((tile.text(), tile.image(), tile.tile_color()), (" >", None, Some(git_red)));
    }
}
