use crate::colour::Rgb;

/// The template's RGBA pixels with the brand colour multiplied in, exactly as tabsmith's `icon_assets::tinted` does it
/// at runtime, so a drawn icon and a tinted badge of the same colour have the same square.
fn tinted(template: &[u8], colour: Rgb) -> Vec<u8> {
    fn scale(a: u8, b: u8) -> u8 {
        ((a as u16 * b as u16) / 256) as u8
    }
    let Rgb(r, g, b) = colour;
    template
        .chunks_exact(4)
        .flat_map(|p| [scale(p[0], r), scale(p[1], g), scale(p[2], b), p[3]])
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::colour::Rgb;

    #[test]
    fn should_tint_the_template_as_tabsmith_tints_it_at_runtime() {
        let template_pixel = [255, 128, 0, 200];

        let tinted = tinted(&template_pixel, Rgb(0xF0, 0x50, 0x32));

        assert_eq!(tinted, [239, 40, 0, 200]);
    }
}
