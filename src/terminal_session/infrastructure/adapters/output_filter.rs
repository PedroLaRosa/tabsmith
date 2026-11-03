/// An unfinished CSI/ESC sequence longer than this passes through instead of being carried (no quadratic rescans).
const SEQ_MAX: usize = 4096;

/// Filters the bytes written to the real terminal: strips keyboard-protocol upgrades outside the alternate screen,
/// tracks DECSTBM (alacritty keeps its scroll region private) and holds back incomplete trailing escape sequences
/// and UTF-8 so an overlay frame appended to a batch can never split them.
pub(crate) struct OutputFilter {
    carry: Vec<u8>,
    rows: usize,
    alt: bool,
    partial_region: bool,
    /// Inside a DCS/SOS/PM/APC string body (`ESC P`/`X`/`^`/`_` … ST): no frame may be appended.
    in_string: bool,
    /// Inside an over-long CSI/ESC sequence already passed through: the bytes that continue it (params/intermediates).
    long_seq: Option<std::ops::RangeInclusive<u8>>,
    /// Windows console without `DISABLE_NEWLINE_AUTO_RETURN`: bare LF → `ESC D`.
    pub(crate) lf_to_ind: bool,
}

impl OutputFilter {
    pub(crate) fn new(rows: usize) -> OutputFilter {
        OutputFilter {
            carry: Vec::new(),
            rows,
            alt: false,
            partial_region: false,
            in_string: false,
            long_seq: None,
            lf_to_ind: false,
        }
    }

    /// Terminal resized: real terminals and the model reset the scroll region.
    pub(crate) fn resize(&mut self, rows: usize) {
        self.rows = rows;
        self.partial_region = false;
    }

    /// A non-full scroll region is active (render guard, SPEC §7.5.1).
    pub(crate) fn scroll_region(&self) -> bool {
        self.partial_region
    }

    /// A DCS/APC/PM/SOS string or an over-long CSI/ESC sequence is open on the terminal (render guard: a frame
    /// would land inside it).
    pub(crate) fn in_string(&self) -> bool {
        self.in_string || self.long_seq.is_some()
    }

    pub(crate) fn feed(&mut self, bytes: &[u8], out: &mut Vec<u8>) {
        let mut data = std::mem::take(&mut self.carry);
        data.extend_from_slice(bytes);
        let mut i = 0;
        while i < data.len() {
            if self.in_string {
                // The body passes through uncarried (sixel can be megabytes). Like the VT parser, any ESC (ST is
                // `ESC \`) or CAN/SUB ends the string; the ESC is then handled as a normal sequence.
                let end = data[i..]
                    .iter()
                    .position(|b| matches!(b, 0x1b | 0x18 | 0x1a))
                    .map_or(data.len(), |k| i + k);
                out.extend_from_slice(&data[i..end]);
                i = end;
                if i < data.len() {
                    self.in_string = false;
                    if data[i] != 0x1b {
                        out.push(data[i]);
                        i += 1;
                    }
                }
                continue;
            }
            if let Some(r) = &self.long_seq {
                let end = data[i..]
                    .iter()
                    .position(|b| !r.contains(b))
                    .map(|k| i + k + 1);
                if end.is_some() {
                    self.long_seq = None;
                }
                let end = end.unwrap_or(data.len());
                out.extend_from_slice(&data[i..end]);
                i = end;
                continue;
            }
            let b = data[i];
            if b == 0x1b {
                // Final byte (inclusive end): CSI params/intermediates then a final; ESC intermediates then a final.
                let skip = |from: usize, r: std::ops::RangeInclusive<u8>| {
                    (from..data.len())
                        .find(|&k| !r.contains(&data[k]))
                        .unwrap_or(data.len())
                };
                let j = match data.get(i + 1) {
                    None => data.len(),
                    Some(b'[') => skip(i + 2, 0x20..=0x3f),
                    Some(0x20..=0x2f) => skip(i + 1, 0x20..=0x2f),
                    Some(_) => i + 1,
                };
                if j >= data.len() {
                    if data.len() - i > SEQ_MAX {
                        // ponytail: no keyboard-protocol upgrade is this long, so it is never stripped.
                        self.long_seq = Some(if data[i + 1] == b'[' {
                            0x20..=0x3f
                        } else {
                            0x20..=0x2f
                        });
                        out.extend_from_slice(&data[i..]);
                        return self.hold_utf8(out);
                    }
                    self.carry = data[i..].to_vec();
                    return self.hold_utf8(out);
                }
                let seq = &data[i..=j];
                if seq == b"\x1bc" {
                    self.alt = false;
                    self.partial_region = false;
                }
                self.in_string = matches!(seq, [0x1b, b'P' | b'X' | b'^' | b'_']);
                if seq[1] != b'[' || self.keep_csi(&seq[2..seq.len() - 1], seq[seq.len() - 1]) {
                    out.extend_from_slice(seq);
                }
                i = j + 1;
                continue;
            }
            if b == b'\n' && self.lf_to_ind && (i == 0 || data[i - 1] != b'\r') {
                out.extend_from_slice(b"\x1bD");
            } else {
                out.push(b);
            }
            i += 1;
        }
        self.hold_utf8(out);
    }

    /// Track alt screen / DECSTBM; false = strip (keyboard-protocol upgrade outside the alternate screen).
    fn keep_csi(&mut self, body: &[u8], fin: u8) -> bool {
        let numeric = |s: &[u8]| s.iter().all(|c| c.is_ascii_digit() || *c == b';');
        let private = body.first() == Some(&b'?');
        if private
            && (fin == b'h' || fin == b'l')
            && body[1..]
                .split(|c| *c == b';')
                .any(|p| matches!(p, b"1049" | b"1047" | b"47"))
        {
            self.alt = fin == b'h';
        }
        if fin == b'r' && numeric(body) {
            let mut it = body
                .split(|c| *c == b';')
                .map(|p| std::str::from_utf8(p).ok()?.parse::<usize>().ok());
            let top = it.next().flatten().filter(|t| *t > 0).unwrap_or(1);
            let bottom = it.next().flatten().filter(|b| *b > 0).unwrap_or(self.rows);
            if top < bottom {
                self.partial_region = top > 1 || bottom < self.rows;
            }
        }
        if self.alt {
            return true;
        }
        let rest = body.get(1..).unwrap_or_default();
        let strip = match (body.first(), fin) {
            (Some(b'?'), b'h' | b'l') => rest == b"9001",
            (Some(b'?'), b'u') => rest.is_empty(),
            (Some(b'=' | b'>' | b'<'), b'u') => numeric(rest),
            (Some(b'>'), b'm') => numeric(rest),
            _ => false,
        };
        !strip
    }

    /// Move an incomplete UTF-8 sequence at the end of `out` into the carry (prepended to the next batch).
    fn hold_utf8(&mut self, out: &mut Vec<u8>) {
        let n = out.len();
        for k in 1..=3.min(n) {
            let c = out[n - k];
            if c & 0xc0 == 0x80 {
                continue;
            }
            let need = match c {
                0xf0.. => 4,
                0xe0.. => 3,
                0xc0.. => 2,
                _ => 1,
            };
            if need > k {
                let mut tail = out.split_off(n - k);
                tail.append(&mut self.carry);
                self.carry = tail;
            }
            return;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn filter(f: &mut OutputFilter, s: &[u8]) -> Vec<u8> {
        let mut out = Vec::new();
        f.feed(s, &mut out);
        out
    }

    #[test]
    fn output_filter_strips_keyboard_upgrades() {
        let mut f = OutputFilter::new(24);
        for seq in [
            "\x1b[?9001h",
            "\x1b[?9001l",
            "\x1b[?u",
            "\x1b[=1;1u",
            "\x1b[>1u",
            "\x1b[<1u",
            "\x1b[>4;2m",
            "\x1b[>4m",
        ] {
            assert_eq!(
                filter(&mut f, format!("a{seq}b").as_bytes()),
                b"ab",
                "{seq:?}"
            );
        }
        for seq in [
            "\x1b[u",
            "\x1b[>c",
            "\x1b[?2004h",
            "\x1b[38;2;219;177;49m",
            "\x1b(B",
            "\x1b7",
        ] {
            assert_eq!(filter(&mut f, seq.as_bytes()), seq.as_bytes(), "{seq:?}");
        }
        // Split across batches: held back, then stripped / emitted whole.
        assert_eq!(filter(&mut f, b"before\x1b[>4;"), b"before");
        assert_eq!(filter(&mut f, b"2mafter"), b"after");
        assert_eq!(filter(&mut f, b"x\x1b[3"), b"x");
        assert_eq!(filter(&mut f, b"1mz"), b"\x1b[31mz");
        // Inside the alternate screen the app keeps its keyboard protocol.
        assert_eq!(
            filter(&mut f, b"\x1b[?1049h\x1b[>1u"),
            b"\x1b[?1049h\x1b[>1u"
        );
        assert_eq!(
            filter(&mut f, b"\x1b[<u\x1b[?1049l\x1b[>1u"),
            b"\x1b[<u\x1b[?1049l"
        );
    }

    #[test]
    fn output_filter_tracks_dcs_and_apc_strings() {
        let mut f = OutputFilter::new(24);
        // Split DCS (XTGETTCAP): open across the batch boundary, closed by ST; bytes pass through unchanged.
        assert_eq!(filter(&mut f, b"a\x1bP+q"), b"a\x1bP+q");
        assert!(f.in_string());
        assert_eq!(filter(&mut f, b"696e"), b"696e");
        assert!(f.in_string());
        assert_eq!(filter(&mut f, b"646e\x1b\\b"), b"646e\x1b\\b");
        assert!(!f.in_string());
        // APC split right before its ST; a trailing ESC is held as usual.
        assert_eq!(filter(&mut f, b"\x1b_Gi=1;x\x1b"), b"\x1b_Gi=1;x");
        assert!(!f.in_string());
        assert_eq!(filter(&mut f, b"\\z"), b"\x1b\\z");
        // CAN aborts a string; a bare LF inside a body is not rewritten.
        f.lf_to_ind = true;
        assert_eq!(filter(&mut f, b"\x1bPq#0\n\x18c"), b"\x1bPq#0\n\x18c");
        assert!(!f.in_string());
        // Other ESC sequences do not open a string.
        assert_eq!(filter(&mut f, b"\x1b7\x1b(B"), b"\x1b7\x1b(B");
        assert!(!f.in_string());
    }

    #[test]
    fn output_filter_passes_overlong_sequence_through() {
        // A stray `ESC [` before a megabyte of digits: bounded carry (no quadratic rescans), bytes pass through,
        // and the render guard holds until the final byte.
        let mut f = OutputFilter::new(24);
        let mut out = filter(&mut f, b"x\x1b[");
        let digits = vec![b'1'; 16 * 1024];
        for _ in 0..64 {
            out.extend(filter(&mut f, &digits));
            assert!(f.carry.len() <= SEQ_MAX + digits.len());
        }
        assert!(f.in_string() && out.len() > 64 * digits.len() - SEQ_MAX - digits.len());
        out.extend(filter(&mut f, b"1mz\x1b[?9001h"));
        assert!(!f.in_string());
        assert_eq!(
            out,
            [&b"x\x1b["[..], &vec![b'1'; 64 * digits.len() + 1], b"mz"].concat()
        );
    }

    #[test]
    fn output_filter_tracks_scroll_region_and_utf8() {
        let mut f = OutputFilter::new(24);
        filter(&mut f, b"\x1b[2;20r");
        assert!(f.scroll_region());
        filter(&mut f, b"\x1b[r");
        assert!(!f.scroll_region());
        filter(&mut f, b"\x1b[1;24r");
        assert!(!f.scroll_region());
        filter(&mut f, b"\x1b[5;30r");
        assert!(f.scroll_region());
        f.resize(30);
        assert!(!f.scroll_region());
        // UTF-8 split across reads is never emitted partially.
        let e = "é€".as_bytes();
        assert_eq!(filter(&mut f, &e[..1]), b"");
        assert_eq!(filter(&mut f, &e[1..3]), "é".as_bytes());
        assert_eq!(filter(&mut f, &e[3..]), "€".as_bytes());
        // Bare LF → ESC D only when asked (Windows without DISABLE_NEWLINE_AUTO_RETURN).
        assert_eq!(filter(&mut f, b"a\nb\r\n"), b"a\nb\r\n");
        f.lf_to_ind = true;
        assert_eq!(filter(&mut f, b"a\nb\r\n\n"), b"a\x1bDb\r\n\x1bD");
    }
}
