/// A brand colour, as simple-icons writes it: six hex digits, `F69220`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rgb(pub u8, pub u8, pub u8);

impl Rgb {
    pub fn from_hex(hex: &str) -> Result<Rgb, String> {
        let channel = |at: usize| u8::from_str_radix(&hex[at..at + 2], 16).map_err(|e| e.to_string());
        Ok(Rgb(channel(0)?, channel(2)?, channel(4)?))
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
}
