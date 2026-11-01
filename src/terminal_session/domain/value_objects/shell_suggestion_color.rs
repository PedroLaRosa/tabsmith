/// A colour as an xterm-256 index; `None` = the terminal's default colour.
type Col = Option<u8>;

/// Colour the shell draws its own suggestions in. Any listed colour matches; an empty list matches every colour.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct ShellSuggestionColor {
    fg: Vec<Option<u8>>,
    bg: Vec<Option<u8>>,
}

impl ShellSuggestionColor {
    pub(crate) fn new(fg: Vec<Option<u8>>, bg: Vec<Option<u8>>) -> ShellSuggestionColor {
        ShellSuggestionColor { fg, bg }
    }

    pub(crate) fn fg(&self) -> &[Option<u8>] {
        &self.fg
    }

    pub(crate) fn bg(&self) -> &[Option<u8>] {
        &self.bg
    }

    /// zsh highlight style: `fg=8`, `fg=#aaaaaa,bg=…`, `fg=245,bold`, `fg=yellow`.
    pub(crate) fn parse_zsh(spec: &str) -> Option<ShellSuggestionColor> {
        let mut c = ShellSuggestionColor::default();
        for part in spec.split(',').map(str::trim) {
            if let Some(v) = part.strip_prefix("fg=") {
                c.fg.extend(parse_color(v, false));
            } else if let Some(v) = part.strip_prefix("bg=") {
                c.bg.extend(parse_color(v, false));
            }
        }
        (!c.fg.is_empty() || !c.bg.is_empty()).then_some(c)
    }

    /// fish `set_color` arguments: every non-option token is a colour (`555 brblack`, `--bold 666`).
    pub(crate) fn parse_fish(spec: &str) -> Option<ShellSuggestionColor> {
        let mut c = ShellSuggestionColor::default();
        let mut tokens = spec.split_whitespace();
        while let Some(t) = tokens.next() {
            if t == "-b" || t == "--background" {
                tokens.next(); // ponytail: background colours ignored (fish autosuggestions never set one)
            } else if !t.starts_with('-') {
                c.fg.extend(parse_color(t, true));
            }
        }
        (!c.fg.is_empty()).then_some(c)
    }

    pub(crate) fn matches(&self, fg: Option<u8>, bg: Option<u8>) -> bool {
        (self.fg.is_empty() || self.fg.contains(&fg))
            && (self.bg.is_empty() || self.bg.contains(&bg))
    }
}

/// One colour token: names (+ fish `br*` variants), zsh `0..255`, hex `#rgb`/`#rrggbb` (fish: `#` optional).
fn parse_color(v: &str, fish: bool) -> Option<Col> {
    const NAMES: [&str; 8] = [
        "black", "red", "green", "yellow", "blue", "magenta", "cyan", "white",
    ];
    let v = v.trim().to_ascii_lowercase();
    if v == "default" || v == "normal" {
        return Some(None);
    }
    if let Some(i) = NAMES.iter().position(|n| *n == v) {
        return Some(Some(i as u8));
    }
    if let Some(i) = v
        .strip_prefix("br")
        .and_then(|n| NAMES.iter().position(|x| *x == n))
    {
        return Some(Some(i as u8 + 8));
    }
    if !fish {
        if let Ok(i) = v.parse::<u8>() {
            return Some(Some(i));
        }
    }
    let hex = match v.strip_prefix('#') {
        Some(h) => h,
        None if fish => v.as_str(),
        None => return None,
    };
    let digit =
        |i: usize, len: usize| u8::from_str_radix(hex.get(i * len..(i + 1) * len)?, 16).ok();
    let (r, g, b) = match hex.len() {
        3 => (digit(0, 1)? * 17, digit(1, 1)? * 17, digit(2, 1)? * 17),
        6 => (digit(0, 2)?, digit(1, 2)?, digit(2, 2)?),
        _ => return None,
    };
    Some(Some(rgb_to_256(r, g, b)))
}

/// Nearest xterm-256 index (6×6×6 cube or 24 greys).
pub(crate) fn rgb_to_256(r: u8, g: u8, b: u8) -> u8 {
    const LEVELS: [i32; 6] = [0, 95, 135, 175, 215, 255];
    let near = |v: u8| {
        (0..6)
            .min_by_key(|&i| (LEVELS[i] - v as i32).abs())
            .unwrap_or(0)
    };
    let (ri, gi, bi) = (near(r), near(g), near(b));
    let dist = |x: i32, y: i32, z: i32| {
        (x - r as i32).pow(2) + (y - g as i32).pow(2) + (z - b as i32).pow(2)
    };
    let grey = ((r as i32 + g as i32 + b as i32) / 3 - 3).clamp(0, 230) / 10; // levels 8, 18, …, 238
    let gv = 8 + 10 * grey;
    if dist(gv, gv, gv) < dist(LEVELS[ri], LEVELS[gi], LEVELS[bi]) {
        232 + grey as u8
    } else {
        (16 + 36 * ri + 6 * gi + bi) as u8
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn suggestion_colors() {
        let z = ShellSuggestionColor::parse_zsh("fg=8,bold").unwrap();
        assert_eq!(z, ShellSuggestionColor::new(vec![Some(8)], vec![]));
        assert_eq!(
            ShellSuggestionColor::parse_zsh("fg=yellow").unwrap().fg(),
            [Some(3)]
        );
        assert_eq!(
            ShellSuggestionColor::parse_zsh("fg=245").unwrap().fg(),
            [Some(245)]
        );
        assert_eq!(
            ShellSuggestionColor::parse_zsh("fg=#aaaaaa,bg=default")
                .unwrap()
                .bg(),
            [None]
        );
        assert_eq!(ShellSuggestionColor::parse_zsh("bold"), None);
        let f = ShellSuggestionColor::parse_fish("555 brblack --bold").unwrap();
        assert_eq!(f.fg(), [Some(rgb_to_256(0x55, 0x55, 0x55)), Some(8)]);
        assert_eq!(
            ShellSuggestionColor::parse_fish("-b red 666").unwrap().fg(),
            [Some(rgb_to_256(0x66, 0x66, 0x66))]
        );
        assert_eq!(
            (
                rgb_to_256(0, 0, 0),
                rgb_to_256(255, 0, 0),
                rgb_to_256(128, 128, 128)
            ),
            (16, 196, 244)
        );
    }

    #[test]
    fn should_match_any_listed_colour_and_every_colour_when_a_list_is_empty() {
        let listed = ShellSuggestionColor::new(vec![Some(8), None], vec![Some(16)]);
        let unrestricted = ShellSuggestionColor::default();

        assert!(listed.matches(Some(8), Some(16)));
        assert!(listed.matches(None, Some(16)));
        assert!(!listed.matches(Some(9), Some(16)));
        assert!(!listed.matches(Some(8), None));
        assert!(unrestricted.matches(Some(200), None));
    }
}
