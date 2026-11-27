use crate::colour::Rgb;
use resvg::tiny_skia::{Pixmap, Transform};
use resvg::usvg::{self, Rect};

/// Width and height of every row icon, as the Amazon Q PNGs.
const SIZE: u32 = 32;
/// The glyph's longer side, in pixels of the 32×32 icon.
const GLYPH_SIDE: f32 = 22.0;
/// Where the middle of the glyph box lands.
const GLYPH_CENTRE: (f32, f32) = (16.0, 16.0);

/// A brand icon as PNG bytes (B3.5): the template (32×32 RGBA8) tinted with the brand colour, the glyph centred on it in
/// the colour's ink.
pub fn drawn(svg: &str, colour: Rgb, template: &[u8]) -> Result<Vec<u8>, String> {
    let square = tinted(template, colour);
    let icon = painted(&square, &glyph_coverage(svg)?, colour.ink());
    Ok(encoded(&icon, SIZE))
}

/// The transform that scales the glyph's box to `GLYPH_SIDE` on its longer side and centres it on `GLYPH_CENTRE`.
fn placement(glyph: Rect) -> Transform {
    let scale = GLYPH_SIDE / glyph.width().max(glyph.height());
    let x = GLYPH_CENTRE.0 - scale * (glyph.x() + glyph.width() / 2.0);
    let y = GLYPH_CENTRE.1 - scale * (glyph.y() + glyph.height() / 2.0);
    Transform::from_row(scale, 0.0, 0.0, scale, x, y)
}

/// How much of each of the 32×32 pixels the placed glyph covers, 0-255, antialiased by resvg.
fn glyph_coverage(svg: &str) -> Result<Vec<u8>, String> {
    let tree = usvg::Tree::from_str(svg, &usvg::Options::default()).map_err(|e| e.to_string())?;
    let mut pixmap = Pixmap::new(SIZE, SIZE).expect("non-zero size");
    resvg::render(&tree, placement(tree.root().abs_bounding_box()), &mut pixmap.as_mut());
    Ok(pixmap.pixels().iter().map(|pixel| pixel.alpha()).collect())
}

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

/// The glyph laid onto the square in straight alpha ("source-atop"): each pixel moves towards the ink by the glyph's
/// coverage (0-255) and keeps the square's alpha, so the glyph never leaves the square's silhouette.
fn painted(square: &[u8], coverage: &[u8], ink: Rgb) -> Vec<u8> {
    let Rgb(r, g, b) = ink;
    let mix = |under: u8, over: u8, c: u8| {
        ((u32::from(over) * u32::from(c) + u32::from(under) * (255 - u32::from(c)) + 127) / 255) as u8
    };
    square
        .chunks_exact(4)
        .zip(coverage)
        .flat_map(|(p, &c)| [mix(p[0], r, c), mix(p[1], g, c), mix(p[2], b, c), p[3]])
        .collect()
}

/// RGBA8 pixels as a PNG. The encoder writes no timestamp or other per-run data, so the same pixels always give the
/// same bytes (B3.6).
fn encoded(rgba: &[u8], width: u32) -> Vec<u8> {
    let height = rgba.len() as u32 / 4 / width;
    let mut bytes = Vec::new();
    let mut encoder = png::Encoder::new(&mut bytes, width, height);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    encoder.set_compression(png::Compression::High);
    let mut writer = encoder.write_header().expect("in-memory PNG header");
    writer.write_image_data(rgba).expect("in-memory PNG data");
    writer.finish().expect("in-memory PNG end");
    bytes
}

#[cfg(test)]
mod tests {
    use super::*;
    use resvg::tiny_skia::Point;

    #[test]
    fn should_tint_the_template_as_tabsmith_tints_it_at_runtime() {
        let template_pixel = [255, 128, 0, 200];

        let tinted = tinted(&template_pixel, Rgb(0xF0, 0x50, 0x32));

        assert_eq!(tinted, [239, 40, 0, 200]);
    }

    #[test]
    fn should_paint_the_glyph_onto_the_square_by_its_coverage() {
        let square = [200, 100, 50, 255, 200, 100, 50, 255, 200, 100, 50, 128];
        let coverage = [0, 255, 128];

        let icon = painted(&square, &coverage, Rgb(0, 0, 0));

        assert_eq!(icon, [200, 100, 50, 255, 0, 0, 0, 255, 100, 50, 25, 128]);
    }

    #[test]
    fn should_encode_the_same_pixels_to_the_same_bytes() {
        let pixels = [240, 80, 50, 255, 0, 0, 0, 0, 255, 255, 255, 128, 1, 2, 3, 4];

        let first = encoded(&pixels, 2);
        let second = encoded(&pixels, 2);

        assert!(!first.is_empty() && first == second);
    }

    #[test]
    fn should_encode_a_png_that_decodes_to_the_same_straight_alpha_pixels() {
        let pixels = [240, 80, 50, 255, 0, 0, 0, 0, 255, 255, 255, 128, 1, 2, 3, 4];

        let png = encoded(&pixels, 2);

        assert_eq!(decoded(&png), pixels);
    }

    #[test]
    fn should_scale_the_glyph_box_to_the_glyph_side_and_centre_it_on_the_square() {
        let wide_glyph = Rect::from_xywh(2.0, 6.0, 20.0, 10.0).unwrap();

        let mut corners = [Point::from_xy(2.0, 6.0), Point::from_xy(22.0, 16.0)];
        placement(wide_glyph).map_points(&mut corners);

        let half = GLYPH_SIDE / 2.0;
        let (x, y) = GLYPH_CENTRE;
        let expected = [(x - half, y - half / 2.0), (x + half, y + half / 2.0)];
        let rounded = |p: &Point| ((p.x * 1000.0).round() / 1000.0, (p.y * 1000.0).round() / 1000.0);
        assert_eq!(corners.each_ref().map(rounded), expected);
    }

    #[test]
    fn should_cover_the_middle_of_the_square_and_leave_its_corner_bare() {
        let square_glyph = r#"<svg viewBox="0 0 24 24" xmlns="http://www.w3.org/2000/svg"><path d="M2 2h20v20H2z"/></svg>"#;

        let coverage = glyph_coverage(square_glyph).unwrap();

        let at = |x: usize, y: usize| coverage[y * 32 + x];
        assert_eq!((coverage.len(), at(16, 16), at(1, 1)), (32 * 32, 255, 0));
    }

    #[test]
    fn should_draw_the_glyph_in_its_ink_on_the_square_tinted_with_the_brand_colour() {
        let square_glyph = r#"<svg viewBox="0 0 24 24" xmlns="http://www.w3.org/2000/svg"><path d="M2 2h20v20H2z"/></svg>"#;
        let white_template = [255; 32 * 32 * 4];

        let png = drawn(square_glyph, Rgb(0xF6, 0x92, 0x20), &white_template).unwrap();

        let pixels = decoded(&png);
        let at = |x: usize, y: usize| &pixels[(y * 32 + x) * 4..][..4];
        assert_eq!((at(16, 16), at(1, 1)), (&[0, 0, 0, 255][..], &[245, 145, 31, 255][..]));
    }

    fn decoded(png: &[u8]) -> Vec<u8> {
        let mut reader = png::Decoder::new(std::io::Cursor::new(png)).read_info().unwrap();
        let mut pixels = vec![0; reader.output_buffer_size().unwrap()];
        reader.next_frame(&mut pixels).unwrap();
        pixels
    }
}
