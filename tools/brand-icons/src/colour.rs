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
}
