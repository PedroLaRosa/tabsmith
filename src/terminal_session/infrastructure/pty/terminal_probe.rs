use crate::dropdown::domain::value_objects::rgb::Rgb;
use std::time::Duration;

pub(crate) const PROBE_TIMEOUT: Duration = Duration::from_millis(300);
/// Startup probe: background colour (`system` theme, also when chosen later) and cursor position (SPEC §4.4 step 5).
pub(crate) const PROBE: &[u8] = b"\x1b]11;?\x1b\\\x1b[6n";

/// Startup probe replies (SPEC §4.4 step 5): `(CPR (row, col), OSC 11 background, other bytes typed meanwhile)`.
pub(crate) fn parse_probe(buf: &[u8]) -> (Option<(u16, u16)>, Option<Rgb>, Vec<u8>) {
    let (mut cpr, mut bg, mut rest) = (None, None, Vec::new());
    let mut i = 0;
    while i < buf.len() {
        let s = &buf[i..];
        if let Some((r, c, n)) = parse_cpr(s) {
            cpr = Some((r, c));
            i += n;
        } else if let Some((rgb, n)) = parse_osc11(s) {
            bg = rgb.or(bg);
            i += n;
        } else {
            rest.push(buf[i]);
            i += 1;
        }
    }
    (cpr, bg, rest)
}

/// `ESC [ row ; col R` → (row, col, length).
fn parse_cpr(s: &[u8]) -> Option<(u16, u16, usize)> {
    let body = s.strip_prefix(b"\x1b[")?;
    let end = body
        .iter()
        .position(|b| !(b.is_ascii_digit() || *b == b';'))?;
    if body[end] != b'R' {
        return None;
    }
    let (r, c) = std::str::from_utf8(&body[..end]).ok()?.split_once(';')?;
    Some((r.parse().ok()?, c.parse().ok()?, end + 3))
}

/// `ESC ] 11 ; rgb:RRRR/GGGG/BBBB (BEL | ESC \)` → (colour, length).
fn parse_osc11(s: &[u8]) -> Option<(Option<Rgb>, usize)> {
    let body = s.strip_prefix(b"\x1b]11;")?;
    let end = body.iter().position(|b| *b == 0x07 || *b == 0x1b)?;
    let term_len = if body[end] == 0x07 {
        1
    } else if body.get(end + 1) == Some(&b'\\') {
        2
    } else {
        return None;
    };
    let text = std::str::from_utf8(&body[..end]).ok()?;
    let mut parts = text
        .strip_prefix("rgb:")
        .into_iter()
        .flat_map(|t| t.split('/'))
        .map(|h| {
            let v = u32::from_str_radix(h, 16)
                .ok()
                .filter(|_| (1..=4).contains(&h.len()))?;
            Some((v * 255 / ((1 << (4 * h.len())) - 1)) as u8)
        });
    let rgb = (|| Some((parts.next()??, parts.next()??, parts.next()??)))();
    Some((rgb, 5 + end + term_len))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn probe_replies() {
        let (cpr, bg, rest) = parse_probe(b"ab\x1b]11;rgb:ffff/8080/0000\x1b\\\x1b[12;40Rc");
        assert_eq!(
            (cpr, bg, rest.as_slice()),
            (Some((12, 40)), Some((255, 128, 0)), b"abc".as_slice())
        );
        let (cpr, bg, rest) = parse_probe(b"\x1b]11;rgb:f/0/f\x07\x1b[1;1R");
        assert_eq!(
            (cpr, bg, rest.len()),
            (Some((1, 1)), Some((255, 0, 255)), 0)
        );
        // Keys typed meanwhile (arrows, partial replies) are kept for the shell.
        let (cpr, _, rest) = parse_probe(b"\x1b[A\x1b[12;");
        assert_eq!((cpr, rest.as_slice()), (None, b"\x1b[A\x1b[12;".as_slice()));
        // The background is always asked for (a later switch to `system` needs it), before the CPR that ends it.
        assert!(PROBE.starts_with(b"\x1b]11;?") && PROBE.ends_with(b"\x1b[6n"));
    }
}
