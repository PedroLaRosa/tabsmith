//! Stdin key decoding, keystroke grammar and the keybinding table (SPEC §7.2.1–7.2.3).
//! OWNER: ui agent. Used by the wrapper (pty.rs) to decide interception.
use crate::settings::Settings;
use std::ops::Range;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum KeyCode {
    Char(char),
    Enter,
    Tab,
    Backspace,
    Esc,
    Up,
    Down,
    Left,
    Right,
    Home,
    End,
    PageUp,
    PageDown,
    Delete,
    Insert,
    F(u8),
    /// Bracketed paste `ESC[200~ … ESC[201~` (never intercepted).
    Paste,
    /// Cursor position report `ESC [ r ; c R` (forwarded untouched).
    CursorReport,
    /// Anything else (forwarded untouched).
    Unknown,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub struct Mods {
    pub ctrl: bool,
    pub alt: bool,
    pub shift: bool,
    pub meta: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Key {
    pub code: KeyCode,
    pub mods: Mods,
}

/// A decoded key and the byte range of the chunk it came from.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KeyEvent {
    pub key: Key,
    pub bytes: Range<usize>,
}

/// Split a stdin chunk into keys (SPEC §7.2.1). Every byte belongs to exactly one event.
/// Stateless: a paste longer than the chunk ends the chunk as `Paste`; use [`Decoder`] to carry it over.
pub fn decode(chunk: &[u8]) -> Vec<KeyEvent> {
    Decoder::default().decode(chunk)
}

const PASTE_START: &[u8] = b"\x1b[200~";
const PASTE_END: &[u8] = b"\x1b[201~";

/// Stateful [`decode`]: a bracketed paste split across reads stays opaque (`Paste`) until `ESC[201~`.
#[derive(Default)]
pub struct Decoder {
    /// Inside a paste: how many bytes of `ESC[201~` matched so far.
    paste: Option<usize>,
}

impl Decoder {
    pub fn decode(&mut self, chunk: &[u8]) -> Vec<KeyEvent> {
        let mut out = Vec::new();
        let mut i = 0;
        while i < chunk.len() {
            let (key, len) = if self.paste.is_some() {
                (key(KeyCode::Paste, Mods::default()), self.scan_paste(&chunk[i..]))
            } else if chunk[i..].starts_with(PASTE_START) {
                self.paste = Some(0);
                let n = PASTE_START.len();
                (key(KeyCode::Paste, Mods::default()), n + self.scan_paste(&chunk[i + n..]))
            } else {
                decode_one(&chunk[i..])
            };
            out.push(KeyEvent { key, bytes: i..i + len });
            i += len;
        }
        out
    }

    /// Consume paste bytes up to and including `ESC[201~` (matched across chunks).
    fn scan_paste(&mut self, bytes: &[u8]) -> usize {
        let mut m = self.paste.unwrap_or(0);
        for (j, &b) in bytes.iter().enumerate() {
            m = if b == PASTE_END[m] { m + 1 } else { usize::from(b == 0x1b) };
            if m == PASTE_END.len() {
                self.paste = None;
                return j + 1;
            }
        }
        self.paste = Some(m);
        bytes.len()
    }
}

fn key(code: KeyCode, mods: Mods) -> Key {
    Key { code, mods }
}

/// One key at the start of `b` (non-empty) and its length in bytes.
fn decode_one(b: &[u8]) -> (Key, usize) {
    let none = Mods::default();
    let ctrl = Mods { ctrl: true, ..none };
    match b[0] {
        b'\r' | b'\n' => (key(KeyCode::Enter, none), 1),
        b'\t' => (key(KeyCode::Tab, none), 1),
        0x7f | 0x08 => (key(KeyCode::Backspace, none), 1),
        0x1b => decode_esc(b),
        c @ 0x01..=0x1a => (key(KeyCode::Char((b'a' + c - 1) as char), ctrl), 1),
        0x00..=0x1f => (key(KeyCode::Unknown, none), 1),
        lead => {
            let len = match lead {
                0xc0..=0xdf => 2,
                0xe0..=0xef => 3,
                0xf0..=0xf7 => 4,
                _ => 1,
            };
            match b.get(..len).and_then(|s| std::str::from_utf8(s).ok()).and_then(|s| s.chars().next()) {
                Some(c) => (key(KeyCode::Char(c), none), len),
                None => (key(KeyCode::Unknown, none), 1),
            }
        }
    }
}

fn decode_esc(b: &[u8]) -> (Key, usize) {
    let none = Mods::default();
    match b.get(1) {
        // ESC + CSI/SS3 (meta-sends-escape Alt+arrow) = alt+<key>; an ESC right before a paste stays a lone Esc.
        Some(0x1b) if b.len() > 3 && matches!(b[2], b'[' | b'O') && !b[1..].starts_with(PASTE_START) => {
            let (mut k, len) = decode_esc(&b[1..]);
            k.mods.alt = true;
            (k, len + 1)
        }
        // ESC alone at the end of the chunk, or the first of two ESCs.
        None | Some(0x1b) => (key(KeyCode::Esc, none), 1),
        Some(b'[') if b.len() > 2 => decode_csi(b),
        Some(b'O') if b.len() > 2 => {
            let code = match b[2] {
                b'A' => KeyCode::Up,
                b'B' => KeyCode::Down,
                b'C' => KeyCode::Right,
                b'D' => KeyCode::Left,
                b'H' => KeyCode::Home,
                b'F' => KeyCode::End,
                // Keypad application mode (DECKPAM, smkx).
                b'M' => KeyCode::Enter,
                b'I' => KeyCode::Tab,
                c @ b'P'..=b'S' => KeyCode::F(c - b'P' + 1),
                _ => KeyCode::Unknown,
            };
            (key(code, none), 3)
        }
        // ESC <key> = alt+<key>.
        Some(_) => {
            let (mut k, len) = decode_one(&b[1..]);
            k.mods.alt = true;
            (k, len + 1)
        }
    }
}

/// `ESC [ params final` (xterm keys, CSI-u, CPR).
fn decode_csi(b: &[u8]) -> (Key, usize) {
    let Some(end) = b[2..].iter().position(|c| (0x40..=0x7e).contains(c)).map(|p| p + 2) else {
        return (key(KeyCode::Unknown, Mods::default()), b.len());
    };
    let len = end + 1;
    let params = &b[2..end];
    let fin = b[end];
    if fin == b'R' {
        return (key(KeyCode::CursorReport, Mods::default()), len);
    }
    if params.first().is_some_and(|c| matches!(c, b'?' | b'<' | b'>' | b'=')) {
        return (key(KeyCode::Unknown, Mods::default()), len);
    }
    // Numeric params; sub-params (`a:b`) keep their first part.
    let p: Vec<u32> = std::str::from_utf8(params)
        .unwrap_or("")
        .split(';')
        .map(|s| s.split(':').next().unwrap_or("").parse().unwrap_or(0))
        .collect();
    let m = p.get(1).copied().unwrap_or(1).saturating_sub(1);
    let mods = Mods { shift: m & 1 != 0, alt: m & 2 != 0, ctrl: m & 4 != 0, meta: m & 8 != 0 };
    let code = match fin {
        b'A' => KeyCode::Up,
        b'B' => KeyCode::Down,
        b'C' => KeyCode::Right,
        b'D' => KeyCode::Left,
        b'H' => KeyCode::Home,
        b'F' => KeyCode::End,
        b'P' | b'Q' | b'S' => KeyCode::F(fin - b'P' + 1),
        b'Z' => return (key(KeyCode::Tab, Mods { shift: true, ..mods }), len),
        b'~' => match p[0] {
            1 | 7 => KeyCode::Home,
            2 => KeyCode::Insert,
            3 => KeyCode::Delete,
            4 | 8 => KeyCode::End,
            5 => KeyCode::PageUp,
            6 => KeyCode::PageDown,
            n @ 11..=15 => KeyCode::F((n - 10) as u8),
            n @ 17..=21 => KeyCode::F((n - 11) as u8),
            n @ 23..=24 => KeyCode::F((n - 12) as u8),
            _ => KeyCode::Unknown,
        },
        b'u' => match p[0] {
            9 => KeyCode::Tab,
            13 => KeyCode::Enter,
            27 => KeyCode::Esc,
            127 => KeyCode::Backspace,
            n => char::from_u32(n).map_or(KeyCode::Unknown, KeyCode::Char),
        },
        _ => KeyCode::Unknown,
    };
    (key(code, mods), len)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Action {
    InsertSelected,
    InsertCommonPrefix,
    InsertCommonPrefixOrNavigateDown,
    InsertCommonPrefixOrInsertSelected,
    InsertSelectedAndExecute,
    Execute,
    NavigateUp,
    NavigateDown,
    HideAutocomplete,
    ShowAutocomplete,
    ToggleAutocomplete,
    ToggleHistoryMode,
    ToggleDescription,
    ShowDescription,
    HideDescription,
    ToggleFuzzySearch,
    /// 1..=10
    SelectSuggestion(u8),
}

impl Action {
    /// Kiro action id (`insertSelected`, `selectSuggestion3`, …); an `autocomplete.` prefix is accepted.
    pub fn from_id(id: &str) -> Option<Action> {
        use Action::*;
        let id = id.strip_prefix("autocomplete.").unwrap_or(id);
        Some(match id {
            "insertSelected" => InsertSelected,
            "insertCommonPrefix" => InsertCommonPrefix,
            "insertCommonPrefixOrNavigateDown" => InsertCommonPrefixOrNavigateDown,
            "insertCommonPrefixOrInsertSelected" => InsertCommonPrefixOrInsertSelected,
            "insertSelectedAndExecute" => InsertSelectedAndExecute,
            "execute" => Execute,
            "navigateUp" => NavigateUp,
            "navigateDown" => NavigateDown,
            "hideAutocomplete" => HideAutocomplete,
            "showAutocomplete" => ShowAutocomplete,
            "toggleAutocomplete" => ToggleAutocomplete,
            "toggleHistoryMode" => ToggleHistoryMode,
            "toggleDescription" => ToggleDescription,
            "showDescription" => ShowDescription,
            "hideDescription" => HideDescription,
            "toggleFuzzySearch" => ToggleFuzzySearch,
            _ => {
                let n: u8 = id.strip_prefix("selectSuggestion")?.parse().ok()?;
                return (1..=10).contains(&n).then_some(SelectSuggestion(n));
            }
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Binding {
    Action(Action),
    /// `"ignore"`: never intercept this key.
    Ignore,
    /// Unknown action id: swallow the key, do nothing (Kiro).
    Unknown,
}

/// Parse a keystroke like `ctrl+k`, `shift+tab`, `+`, `ctrl++`, `f12` (SPEC §7.2.2).
pub fn parse_keystroke(s: &str) -> Option<Key> {
    let mut mods = Mods::default();
    let mut rest = s;
    let name = loop {
        match rest.split_once('+') {
            None | Some(("", "")) => break rest,
            Some((m, k)) => {
                match m {
                    "ctrl" | "control" => mods.ctrl = true,
                    "shift" => mods.shift = true,
                    "alt" | "option" => mods.alt = true,
                    "meta" | "command" => mods.meta = true,
                    _ => {}
                }
                rest = k;
            }
        }
    };
    let code = match name {
        "backspace" => KeyCode::Backspace,
        "enter" => KeyCode::Enter,
        "left" | "arrowleft" => KeyCode::Left,
        "right" | "arrowright" => KeyCode::Right,
        "up" | "arrowup" => KeyCode::Up,
        "down" | "arrowdown" => KeyCode::Down,
        "home" => KeyCode::Home,
        "end" => KeyCode::End,
        "pageup" => KeyCode::PageUp,
        "pagedown" => KeyCode::PageDown,
        "tab" => KeyCode::Tab,
        "delete" => KeyCode::Delete,
        "insert" => KeyCode::Insert,
        "esc" => KeyCode::Esc,
        f if f.len() > 1 && f.starts_with('f') && f[1..].bytes().all(|b| b.is_ascii_digit()) => {
            KeyCode::F(f[1..].parse().ok()?)
        }
        _ => {
            let mut chars = name.chars();
            let mut c = chars.next()?;
            if chars.next().is_some() {
                return None;
            }
            if mods.shift && c.is_ascii_lowercase() {
                c = c.to_ascii_uppercase();
                mods.shift = false;
            }
            KeyCode::Char(c)
        }
    };
    Some(Key { code, mods })
}

/// ctrl/alt + ASCII letter match both cases (Kiro registers the opposite case too).
fn normalize(mut k: Key) -> Key {
    if let KeyCode::Char(c) = k.code {
        if (k.mods.ctrl || k.mods.alt) && c.is_ascii_alphabetic() {
            k.code = KeyCode::Char(c.to_ascii_lowercase());
        }
    }
    k
}

/// fig.json defaults (D5).
const DEFAULTS: &[(&str, Action)] = &[
    ("enter", Action::InsertSelected),
    ("tab", Action::InsertCommonPrefix),
    ("shift+tab", Action::NavigateUp),
    ("up", Action::NavigateUp),
    ("control+p", Action::NavigateUp),
    ("down", Action::NavigateDown),
    ("control+n", Action::NavigateDown),
    ("esc", Action::HideAutocomplete),
    ("control+k", Action::ToggleDescription),
    ("control+r", Action::ToggleHistoryMode),
];

/// Default bindings (fig.json set, SPEC §7.2.3) overlaid with `autocomplete.keybindings.*`.
pub struct Keymap {
    bindings: Vec<(Key, Binding)>,
}

impl Keymap {
    pub fn from_settings(settings: &Settings) -> Keymap {
        let mut map = Keymap { bindings: Vec::new() };
        for (k, a) in DEFAULTS {
            map.set(k, Binding::Action(*a));
        }
        for (k, v) in settings.with_prefix("autocomplete.keybindings.") {
            let Some(v) = v.as_str() else { continue };
            let b = match v {
                "ignore" => Binding::Ignore,
                _ => Action::from_id(v).map_or(Binding::Unknown, Binding::Action),
            };
            map.set(k, b);
        }
        map
    }

    /// A later binding for the same key replaces the earlier one; unparsable keystrokes are dropped.
    fn set(&mut self, keystroke: &str, b: Binding) {
        let Some(key) = parse_keystroke(keystroke).map(normalize) else { return };
        match self.bindings.iter_mut().find(|(k, _)| *k == key) {
            Some(e) => e.1 = b,
            None => self.bindings.push((key, b)),
        }
    }

    /// Binding for a decoded key (handles case-insensitive ctrl/alt letters; `ESC O x` arrows decode as arrows).
    pub fn lookup(&self, key: &Key) -> Option<Binding> {
        let key = normalize(*key);
        self.bindings.iter().find(|(k, _)| *k == key).map(|(_, b)| *b)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const NONE: Mods = Mods { ctrl: false, alt: false, shift: false, meta: false };
    const CTRL: Mods = Mods { ctrl: true, ..NONE };
    const ALT: Mods = Mods { alt: true, ..NONE };
    const SHIFT: Mods = Mods { shift: true, ..NONE };

    fn k(code: KeyCode, mods: Mods) -> Key {
        Key { code, mods }
    }

    fn one(bytes: &[u8]) -> Key {
        let ev = decode(bytes);
        assert_eq!(ev.len(), 1, "{bytes:?} → {ev:?}");
        assert_eq!(ev[0].bytes, 0..bytes.len());
        ev[0].key
    }

    #[test]
    fn decode_table() {
        use KeyCode::*;
        let cases: &[(&[u8], Key)] = &[
            (b"\r", k(Enter, NONE)),
            (b"\n", k(Enter, NONE)),
            (b"\t", k(Tab, NONE)),
            (b"\x1b[Z", k(Tab, SHIFT)),
            (b"\x1b[A", k(Up, NONE)),
            (b"\x1b[B", k(Down, NONE)),
            (b"\x1b[C", k(Right, NONE)),
            (b"\x1b[D", k(Left, NONE)),
            (b"\x1bOA", k(Up, NONE)),
            (b"\x1bOB", k(Down, NONE)),
            (b"\x1bOC", k(Right, NONE)),
            (b"\x1bOD", k(Left, NONE)),
            (b"\x1b[1;5A", k(Up, CTRL)),
            (b"\x1b[1;2B", k(Down, SHIFT)),
            (b"\x1b[1;3C", k(Right, ALT)),
            (b"\x1b[1;7D", k(Left, Mods { ctrl: true, alt: true, ..NONE })),
            (b"\x1b[H", k(Home, NONE)),
            (b"\x1bOH", k(Home, NONE)),
            (b"\x1b[1~", k(Home, NONE)),
            (b"\x1b[7~", k(Home, NONE)),
            (b"\x1b[F", k(End, NONE)),
            (b"\x1bOF", k(End, NONE)),
            (b"\x1bOM", k(Enter, NONE)),
            (b"\x1bOI", k(Tab, NONE)),
            (b"\x1b\x1b[D", k(Left, ALT)),
            (b"\x1b\x1bOA", k(Up, ALT)),
            (b"\x1b\x1b[1;5C", k(Right, Mods { ctrl: true, alt: true, ..NONE })),
            (b"\x1b[4~", k(End, NONE)),
            (b"\x1b[8~", k(End, NONE)),
            (b"\x1b[3~", k(Delete, NONE)),
            (b"\x1b[2~", k(Insert, NONE)),
            (b"\x1b[5~", k(PageUp, NONE)),
            (b"\x1b[6~", k(PageDown, NONE)),
            (b"\x1b[3;5~", k(Delete, CTRL)),
            (b"\x1bOP", k(F(1), NONE)),
            (b"\x1b[24~", k(F(12), NONE)),
            (b"\x7f", k(Backspace, NONE)),
            (b"\x08", k(Backspace, NONE)),
            (b"\x01", k(Char('a'), CTRL)),
            (b"\x0b", k(Char('k'), CTRL)),
            (b"\x12", k(Char('r'), CTRL)),
            (b"\x1a", k(Char('z'), CTRL)),
            (b"\x1b", k(Esc, NONE)),
            (b"\x1bf", k(Char('f'), ALT)),
            (b"\x1bF", k(Char('F'), ALT)),
            (b"\x1b\x7f", k(Backspace, ALT)),
            (b"\x1b\xc3\xa9", k(Char('é'), ALT)),
            (b"\x1b[97;5u", k(Char('a'), CTRL)),
            (b"\x1b[13u", k(Enter, NONE)),
            (b"\x1b[27u", k(Esc, NONE)),
            (b"a", k(Char('a'), NONE)),
            (b"A", k(Char('A'), NONE)),
            ("中".as_bytes(), k(Char('中'), NONE)),
            (b"\x1b[12;40R", k(CursorReport, NONE)),
            (b"\x1b[?12;40R", k(CursorReport, NONE)),
            (b"\x1b[200~ls\r\n\x1b[201~", k(Paste, NONE)),
            (b"\x00", k(Unknown, NONE)),
            (b"\xff", k(Unknown, NONE)),
        ];
        for (bytes, want) in cases {
            assert_eq!(one(bytes), *want, "{bytes:?}");
        }
    }

    #[test]
    fn decode_multi_key_chunk() {
        let chunk = b"ab\x1b[A\r\x1b[200~x\ty\x1b[201~\x0e\x1b";
        let ev = decode(chunk);
        let keys: Vec<KeyCode> = ev.iter().map(|e| e.key.code).collect();
        use KeyCode::*;
        assert_eq!(keys, vec![Char('a'), Char('b'), Up, Enter, Paste, Char('n'), Esc]);
        let ranges: Vec<_> = ev.iter().map(|e| e.bytes.clone()).collect();
        assert_eq!(ranges, vec![0..1, 1..2, 2..5, 5..6, 6..21, 21..22, 22..23]);
        // Every byte belongs to exactly one event.
        assert_eq!(ev.last().unwrap().bytes.end, chunk.len());
    }

    #[test]
    fn esc_not_last_and_double_esc() {
        let ev = decode(b"\x1b\x1b");
        assert_eq!(ev.iter().map(|e| e.key.code).collect::<Vec<_>>(), vec![KeyCode::Esc, KeyCode::Esc]);
        // Esc right before a bracketed paste in the same chunk: the paste stays opaque.
        let ev = decode(b"\x1b\x1b[200~ls\x1b[201~");
        assert_eq!(ev.iter().map(|e| (e.key.code, e.bytes.clone())).collect::<Vec<_>>(), vec![
            (KeyCode::Esc, 0..1),
            (KeyCode::Paste, 1..15)
        ]);
        // Incomplete CSI at the end: one opaque unknown event.
        let ev = decode(b"x\x1b[1;");
        assert_eq!(ev[1].key.code, KeyCode::Unknown);
        assert_eq!(ev[1].bytes, 1..5);
    }

    #[test]
    fn paste_across_chunks() {
        let mut d = Decoder::default();
        let a = d.decode(b"q\x1b[200~line1\r");
        assert_eq!(a.iter().map(|e| e.key.code).collect::<Vec<_>>(), vec![KeyCode::Char('q'), KeyCode::Paste]);
        assert_eq!(a[1].bytes, 1..13);
        // End marker split across reads; `\r` inside the paste is not Enter.
        let b = d.decode(b"\tline2\r\x1b[20");
        assert_eq!(b.len(), 1);
        assert_eq!(b[0].key.code, KeyCode::Paste);
        let c = d.decode(b"1~\r");
        assert_eq!(c.iter().map(|e| e.key.code).collect::<Vec<_>>(), vec![KeyCode::Paste, KeyCode::Enter]);
        assert_eq!(c[0].bytes, 0..2);
    }

    #[test]
    fn keystroke_grammar() {
        use KeyCode::*;
        assert_eq!(parse_keystroke("ctrl+k"), Some(k(Char('k'), CTRL)));
        assert_eq!(parse_keystroke("control+K"), Some(k(Char('K'), CTRL)));
        assert_eq!(parse_keystroke("shift+tab"), Some(k(Tab, SHIFT)));
        assert_eq!(parse_keystroke("shift+a"), Some(k(Char('A'), NONE)));
        assert_eq!(parse_keystroke("+"), Some(k(Char('+'), NONE)));
        assert_eq!(parse_keystroke("ctrl++"), Some(k(Char('+'), CTRL)));
        assert_eq!(parse_keystroke("f12"), Some(k(F(12), NONE)));
        assert_eq!(parse_keystroke("f"), Some(k(Char('f'), NONE)));
        assert_eq!(parse_keystroke("hyper+x"), Some(k(Char('x'), NONE)));
        assert_eq!(parse_keystroke("option+arrowup"), Some(k(Up, ALT)));
        assert_eq!(parse_keystroke("command+i"), Some(k(Char('i'), Mods { meta: true, ..NONE })));
        assert_eq!(parse_keystroke("ctrl+foo"), None);
        assert_eq!(parse_keystroke("a+"), None);
        assert_eq!(parse_keystroke(""), None);
    }

    #[test]
    fn action_ids() {
        assert_eq!(Action::from_id("insertSelected"), Some(Action::InsertSelected));
        assert_eq!(Action::from_id("autocomplete.navigateDown"), Some(Action::NavigateDown));
        assert_eq!(Action::from_id("selectSuggestion10"), Some(Action::SelectSuggestion(10)));
        assert_eq!(Action::from_id("selectSuggestion0"), None);
        assert_eq!(Action::from_id("increaseSize"), None);
    }

    fn settings(json: &str) -> Settings {
        match serde_json::from_str(json).unwrap() {
            serde_json::Value::Object(m) => Settings::from_map(m),
            _ => unreachable!(),
        }
    }

    #[test]
    fn keymap_defaults_and_overrides() {
        let km = Keymap::from_settings(&Settings::default());
        let b = |bytes: &[u8]| km.lookup(&one(bytes));
        assert_eq!(b(b"\r"), Some(Binding::Action(Action::InsertSelected)));
        assert_eq!(b(b"\t"), Some(Binding::Action(Action::InsertCommonPrefix)));
        assert_eq!(b(b"\x1b[Z"), Some(Binding::Action(Action::NavigateUp)));
        assert_eq!(b(b"\x1bOA"), Some(Binding::Action(Action::NavigateUp)));
        assert_eq!(b(b"\x10"), Some(Binding::Action(Action::NavigateUp)));
        assert_eq!(b(b"\x1b[B"), Some(Binding::Action(Action::NavigateDown)));
        assert_eq!(b(b"\x0e"), Some(Binding::Action(Action::NavigateDown)));
        assert_eq!(b(b"\x1b"), Some(Binding::Action(Action::HideAutocomplete)));
        assert_eq!(b(b"\x0b"), Some(Binding::Action(Action::ToggleDescription)));
        assert_eq!(b(b"\x12"), Some(Binding::Action(Action::ToggleHistoryMode)));
        assert_eq!(b(b"x"), None);
        assert_eq!(b(b"\x1b[1;5A"), None);

        let km = Keymap::from_settings(&settings(
            r#"{"autocomplete.keybindings.tab": "ignore",
                "autocomplete.keybindings.control+j": "autocomplete.navigateDown",
                "autocomplete.keybindings.alt+K": "toggleAutocomplete",
                "autocomplete.keybindings.enter": "bogus",
                "autocomplete.keybindings.ctrl+foo": "execute",
                "autocomplete.keybindings.f5": 3}"#,
        ));
        let b = |bytes: &[u8]| km.lookup(&one(bytes));
        assert_eq!(b(b"\t"), Some(Binding::Ignore));
        assert_eq!(b(b"\r"), Some(Binding::Unknown));
        assert_eq!(b(b"\x1bk"), Some(Binding::Action(Action::ToggleAutocomplete)));
        assert_eq!(b(b"\x1bK"), Some(Binding::Action(Action::ToggleAutocomplete)));
        assert_eq!(b(b"\x1b[15~"), None);
        assert_eq!(b(b"\x1b[A"), Some(Binding::Action(Action::NavigateUp)));
        // ctrl+j is 0x0a = Enter on the wire; bound via CSI-u it decodes as ctrl+j.
        assert_eq!(b(b"\x1b[106;5u"), Some(Binding::Action(Action::NavigateDown)));
    }
}
