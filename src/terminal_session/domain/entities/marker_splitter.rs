use crate::terminal_session::domain::value_objects::output_piece::OutputPiece;

/// An unterminated OSC longer than this is flushed as plain bytes (SPEC §5.1.1).
const OSC_MAX: usize = 64 * 1024;

#[derive(Clone, Copy, Default, PartialEq)]
enum State {
    #[default]
    Ground,
    Esc,
    Osc,
    OscEsc,
    Name,
    NameEsc,
}

/// Splits PTY output into plain bytes, OSC 697 markers and window names at the exact byte offset; stateful across
/// chunks.
#[derive(Default)]
pub(crate) struct MarkerSplitter {
    state: State,
    osc: Vec<u8>,
    name: Vec<u8>,
}

impl MarkerSplitter {
    pub(crate) fn feed(&mut self, data: &[u8]) -> Vec<OutputPiece> {
        let mut pieces = Vec::new();
        let mut bytes = Vec::with_capacity(data.len());
        for &b in data {
            self.step(b, &mut bytes, &mut pieces);
        }
        if !bytes.is_empty() {
            pieces.push(OutputPiece::Bytes(bytes));
        }
        pieces
    }

    fn step(&mut self, b: u8, bytes: &mut Vec<u8>, pieces: &mut Vec<OutputPiece>) {
        match self.state {
            State::Ground if b == 0x1b => self.state = State::Esc,
            State::Ground => bytes.push(b),
            State::Esc if b == b']' => {
                self.state = State::Osc;
                self.osc.clear();
            }
            State::Esc if b == b'k' => {
                self.state = State::Name;
                self.name = b"\x1bk".to_vec();
            }
            State::Esc => {
                bytes.push(0x1b);
                self.state = State::Ground;
                self.step(b, bytes, pieces);
            }
            State::Osc => match b {
                0x07 => self.finish(b"\x07", bytes, pieces),
                0x1b => self.state = State::OscEsc,
                _ => {
                    self.osc.push(b);
                    if self.osc.len() > OSC_MAX {
                        self.abandon(bytes);
                    }
                }
            },
            State::OscEsc if b == b'\\' => self.finish(b"\x1b\\", bytes, pieces),
            State::OscEsc => {
                // `ESC x` inside an OSC: give up on it and re-process the ESC.
                self.abandon(bytes);
                self.state = State::Esc;
                self.step(b, bytes, pieces);
            }
            // Ended as tmux ends it: ST, not BEL; CAN/SUB cancel it.
            State::Name if b == 0x1b => self.state = State::NameEsc,
            State::Name if matches!(b, 0x18 | 0x1a) => {
                self.name.push(b);
                self.end_name(bytes, pieces);
            }
            State::Name => {
                self.name.push(b);
                if self.name.len() > OSC_MAX {
                    // Still a name for tmux, so still not for the model. ponytail: a frame may now land inside it;
                    // nothing names a window this long.
                    let name = std::mem::take(&mut self.name);
                    emit(OutputPiece::WindowName(name), bytes, pieces);
                }
            }
            State::NameEsc if b == b'\\' => {
                self.name.extend_from_slice(b"\x1b\\");
                self.end_name(bytes, pieces);
            }
            State::NameEsc => {
                // `ESC x` ends the name for tmux too (dropped), and the ESC is re-processed.
                self.end_name(bytes, pieces);
                self.state = State::Esc;
                self.step(b, bytes, pieces);
            }
        }
    }

    fn end_name(&mut self, bytes: &mut Vec<u8>, pieces: &mut Vec<OutputPiece>) {
        self.state = State::Ground;
        let name = std::mem::take(&mut self.name);
        emit(OutputPiece::WindowName(name), bytes, pieces);
    }

    /// Emit the collected OSC prefix as plain bytes.
    fn abandon(&mut self, bytes: &mut Vec<u8>) {
        bytes.extend_from_slice(b"\x1b]");
        bytes.append(&mut self.osc);
        self.state = State::Ground;
    }

    fn finish(&mut self, terminator: &[u8], bytes: &mut Vec<u8>, pieces: &mut Vec<OutputPiece>) {
        self.state = State::Ground;
        if self.osc.starts_with(b"697;") {
            let marker = String::from_utf8_lossy(&self.osc[4..]).into_owned();
            emit(OutputPiece::Marker(marker), bytes, pieces);
            self.osc.clear();
        } else {
            bytes.extend_from_slice(b"\x1b]");
            bytes.append(&mut self.osc);
            bytes.extend_from_slice(terminator);
        }
    }
}

/// `piece` at the current byte offset: the plain bytes collected before it go first.
fn emit(piece: OutputPiece, bytes: &mut Vec<u8>, pieces: &mut Vec<OutputPiece>) {
    if !bytes.is_empty() {
        pieces.push(OutputPiece::Bytes(std::mem::take(bytes)));
    }
    pieces.push(piece);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn split_all(chunks: &[&[u8]]) -> Vec<OutputPiece> {
        let mut s = MarkerSplitter::default();
        chunks.iter().flat_map(|c| s.feed(c)).collect()
    }

    fn bytes_of(pieces: &[OutputPiece]) -> Vec<u8> {
        pieces
            .iter()
            .flat_map(|p| {
                if let OutputPiece::Bytes(b) = p {
                    b.clone()
                } else {
                    vec![]
                }
            })
            .collect()
    }

    fn markers_of(pieces: &[OutputPiece]) -> Vec<String> {
        pieces
            .iter()
            .filter_map(|p| {
                if let OutputPiece::Marker(m) = p {
                    Some(m.clone())
                } else {
                    None
                }
            })
            .collect()
    }

    #[test]
    fn splitter_chunk_boundaries() {
        let whole = b"ab\x1b]697;Dir=/a;b\x07cd";
        for cut in 0..=whole.len() {
            let p = split_all(&[&whole[..cut], &whole[cut..]]);
            assert_eq!(bytes_of(&p), b"abcd", "cut {cut}");
            assert_eq!(markers_of(&p), ["Dir=/a;b"], "cut {cut}");
        }
        // Marker sits at the exact byte offset.
        let p = split_all(&[b"x\x1b]697;NewCmd\x07y"]);
        assert_eq!(
            p,
            [
                OutputPiece::Bytes(b"x".to_vec()),
                OutputPiece::Marker("NewCmd".into()),
                OutputPiece::Bytes(b"y".to_vec())
            ]
        );
    }

    #[test]
    fn splitter_bel_st_and_passthrough() {
        let p = split_all(&[b"\x1b]697;A\x07\x1b]697;B\x1b\\"]);
        assert_eq!(markers_of(&p), ["A", "B"]);
        // Non-697 OSCs and other escapes pass through byte-exact (also split across chunks).
        let raw: &[u8] = b"\x1b]0;title\x07\x1b]8;;http://x\x1b\\l\x1b]8;;\x1b\\\x1b[31m";
        assert_eq!(bytes_of(&split_all(&[raw])), raw);
        assert_eq!(
            bytes_of(&split_all(&[&raw[..3], &raw[3..12], &raw[12..]])),
            raw
        );
        // ESC + other byte inside an OSC abandons it and re-processes the ESC.
        let p = split_all(&[b"\x1b]697;Di\x1b[1m\x1b]697;X\x07"]);
        assert_eq!(bytes_of(&p), b"\x1b]697;Di\x1b[1m");
        assert_eq!(markers_of(&p), ["X"]);
        // Trailing lone ESC is held until the next chunk.
        let mut s = MarkerSplitter::default();
        assert!(s
            .feed(b"a\x1b")
            .iter()
            .all(|p| p == &OutputPiece::Bytes(b"a".to_vec())));
        assert_eq!(s.feed(b"[m"), [OutputPiece::Bytes(b"\x1b[m".to_vec())]);
    }

    #[test]
    fn splitter_holds_a_window_name_whole_across_chunks() {
        let whole = b"ab\x1bkvim\x1b\\cd";
        let name = OutputPiece::WindowName(b"\x1bkvim\x1b\\".to_vec());

        for cut in 0..=whole.len() {
            let p = split_all(&[&whole[..cut], &whole[cut..]]);

            assert_eq!(bytes_of(&p), b"abcd", "cut {cut}");
            assert!(p.contains(&name), "cut {cut}");
        }
    }

    #[test]
    fn splitter_ends_a_window_name_at_another_escape_like_tmux() {
        // tmux drops `foo` and runs the SGR: `ABC` on screen.
        let p = split_all(&[b"A\x1bkfoo\x1b[31mB\x1b[0mC"]);

        assert_eq!(
            p,
            [
                OutputPiece::Bytes(b"A".to_vec()),
                OutputPiece::WindowName(b"\x1bkfoo".to_vec()),
                OutputPiece::Bytes(b"\x1b[31mB\x1b[0mC".to_vec())
            ]
        );
    }

    #[test]
    fn splitter_cancels_a_window_name_at_can_or_sub_like_tmux() {
        // tmux drops the name and prints what follows: `DE` on screen.
        let p = split_all(&[b"D\x1bkfoo\x18E", b"\x1bkbar\x1aF"]);

        assert_eq!(bytes_of(&p), b"DEF");
    }

    #[test]
    fn splitter_flushes_huge_window_name_to_the_terminal_only() {
        let mut s = MarkerSplitter::default();
        let mut big = b"\x1bk".to_vec();
        big.extend(std::iter::repeat_n(b'x', OSC_MAX + 10));

        let p = s.feed(&big);

        assert!(bytes_of(&p).is_empty());
        assert!(matches!(&p[..], [OutputPiece::WindowName(n)] if n.len() > OSC_MAX));
    }

    #[test]
    fn splitter_flushes_huge_osc() {
        let mut s = MarkerSplitter::default();
        let mut big = b"\x1b]697;".to_vec();
        big.extend(std::iter::repeat_n(b'x', OSC_MAX + 10));
        big.push(0x07);
        let p = s.feed(&big);
        assert!(markers_of(&p).is_empty());
        assert_eq!(bytes_of(&p), big);
    }
}
