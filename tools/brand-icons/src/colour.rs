/// A brand colour, as simple-icons writes it: six hex digits, `F69220`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rgb(pub u8, pub u8, pub u8);

impl Rgb {
    pub fn from_hex(hex: &str) -> Result<Rgb, String> {
        if hex.len() != 6 || !hex.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err(format!("{hex:?} is not six hex digits"));
        }
        let channel = |at: usize| u8::from_str_radix(&hex[at..at + 2], 16).expect("checked hex digits");
        Ok(Rgb(channel(0), channel(2), channel(4)))
    }

    /// The glyph colour on this brand colour (B2.2): black when the colour is light, white otherwise.
    pub fn ink(self) -> Rgb {
        let Rgb(r, g, b) = self;
        let luma = (299 * u32::from(r) + 587 * u32::from(g) + 114 * u32::from(b)) / 1000;
        if luma > 160 {
            Rgb(0, 0, 0)
        } else {
            Rgb(255, 255, 255)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn should_read_a_brand_colour_from_six_hex_digits() {
        let hex = "F69220";

        let colour = Rgb::from_hex(hex);

        assert_eq!(colour, Ok(Rgb(0xF6, 0x92, 0x20)));
    }

    #[test]
    fn should_reject_a_brand_colour_that_is_not_six_hex_digits() {
        let not_colours = ["F692", "F69220A", "#F6922", "F6922G"];

        let parsed: Vec<bool> = not_colours.iter().map(|hex| Rgb::from_hex(hex).is_err()).collect();

        assert_eq!(parsed, [true, true, true, true]);
    }

    #[test]
    fn should_ink_the_glyph_black_on_a_light_brand_colour() {
        let pnpm_orange = Rgb(0xF6, 0x92, 0x20);

        let ink = pnpm_orange.ink();

        assert_eq!(ink, Rgb(0, 0, 0));
    }

    #[test]
    fn should_ink_the_glyph_white_on_a_dark_brand_colour() {
        let git_red = Rgb(0xF0, 0x50, 0x32);

        let ink = git_red.ink();

        assert_eq!(ink, Rgb(255, 255, 255));
    }
}
