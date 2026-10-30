use crate::dropdown::domain::value_objects::rgb::Rgb;

/// `#rgb`, `#rrggbb`, `#rrggbbaa` (alpha premultiplied against black), `rgb(r, g, b)` with optional spaces.
pub(crate) fn parse_color(s: &str) -> Option<(u8, u8, u8)> {
    let s = s.trim();
    if let Some(h) = s.strip_prefix('#') {
        let hex = |i: usize, n: usize| h.get(i..i + n).and_then(|d| u8::from_str_radix(d, 16).ok());
        return match h.len() {
            3 => Some((hex(0, 1)? * 17, hex(1, 1)? * 17, hex(2, 1)? * 17)),
            6 | 8 => {
                let a = if h.len() == 8 { hex(6, 2)? as u32 } else { 255 };
                let m = |c: u8| ((c as u32 * a + 127) / 255) as u8;
                Some((m(hex(0, 2)?), m(hex(2, 2)?), m(hex(4, 2)?)))
            }
            _ => None,
        };
    }
    let mut it = s
        .strip_prefix("rgb(")?
        .strip_suffix(')')?
        .split(',')
        .map(|p| p.trim().parse::<u8>().ok());
    let rgb = (it.next()??, it.next()??, it.next()??);
    it.next().is_none().then_some(rgb)
}

/// Nearest xterm-256 index (6×6×6 cube or 24-step grey ramp).
pub(crate) fn xterm256((r, g, b): (u8, u8, u8)) -> u8 {
    const LV: [u8; 6] = [0, 95, 135, 175, 215, 255];
    let near = |c: u8| {
        (0..6)
            .min_by_key(|&i| (LV[i] as i32 - c as i32).abs())
            .unwrap_or(0)
    };
    let (ri, gi, bi) = (near(r), near(g), near(b));
    let grey = ((((r as i32 + g as i32 + b as i32) / 3) - 8 + 5) / 10).clamp(0, 23);
    let gv = (8 + 10 * grey) as u8;
    let dist = |(x, y, z): Rgb| {
        [(x, r), (y, g), (z, b)]
            .iter()
            .map(|&(p, q)| (p as i32 - q as i32).pow(2))
            .sum::<i32>()
    };
    if dist((gv, gv, gv)) < dist((LV[ri], LV[gi], LV[bi])) {
        232 + grey as u8
    } else {
        (16 + 36 * ri + 6 * gi + bi) as u8
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn colours_and_themes() {
        assert_eq!(parse_color("#fff"), Some((255, 255, 255)));
        assert_eq!(parse_color("#1a2B3c"), Some((26, 43, 60)));
        assert_eq!(parse_color("#ff000080"), Some((128, 0, 0)));
        assert_eq!(parse_color("rgb( 1, 2, 3 )"), Some((1, 2, 3)));
        assert_eq!(parse_color("rgb(106,142,218)"), Some((106, 142, 218)));
        for bad in [
            "rgb(1,2)",
            "rgb(1,2,3,4)",
            "red",
            "#12345",
            "#ggg",
            "rgb(300,0,0)",
        ] {
            assert_eq!(parse_color(bad), None, "{bad}");
        }
        assert_eq!(xterm256((0, 0, 0)), 16);
        assert_eq!(xterm256((255, 255, 255)), 231);
        assert_eq!(xterm256((128, 128, 128)), 244);
        assert_eq!(xterm256((95, 135, 175)), 67);
        assert_eq!(xterm256((30, 90, 199)), 26);
        assert_eq!(xterm256((48, 48, 48)), 236);
    }
}
