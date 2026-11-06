use crate::dropdown::domain::value_objects::icon_id::IconId;
use crate::dropdown::domain::value_objects::rgb::Rgb;

/// The icon slot of a row: `text` is two columns wide for every row kind (a spec badge is stored as given); `image` (kitty only) covers it, under the text.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct IconSlot {
    text: String,
    image: Option<IconId>,
    /// Badge text is drawn white.
    badge: bool,
    /// Without images: the slot's colour, so the icon reads as the coloured square its PNG is.
    background: Option<Rgb>,
}

impl IconSlot {
    pub(crate) fn new(text: String, image: Option<IconId>, badge: bool) -> Self {
        Self {
            text,
            image,
            badge,
            background: None,
        }
    }

    /// `text` in white on a `background` square, for terminals that cannot show the PNG.
    pub(crate) fn chip(text: String, background: Rgb) -> Self {
        Self {
            text,
            image: None,
            badge: true,
            background: Some(background),
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

    pub(crate) fn background(&self) -> Option<Rgb> {
        self.background
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
}
