use crate::dropdown::domain::value_objects::key::Key;
use crate::dropdown::domain::value_objects::key_code::KeyCode;
use crate::dropdown::domain::value_objects::modifiers::Modifiers;

/// Parse a keystroke like `ctrl+k`, `shift+tab`, `+`, `ctrl++`, `f12` (SPEC §7.2.2).
pub(crate) fn parse_keystroke(s: &str) -> Option<Key> {
    let mut mods = Modifiers::default();
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

/// ctrl/alt + ASCII letter match both cases.
pub(crate) fn normalize(mut k: Key) -> Key {
    if let KeyCode::Char(c) = k.code {
        if (k.mods.ctrl || k.mods.alt) && c.is_ascii_alphabetic() {
            k.code = KeyCode::Char(c.to_ascii_lowercase());
        }
    }
    k
}

#[cfg(test)]
mod tests {
    use super::*;

    const NONE: Modifiers = Modifiers {
        ctrl: false,
        alt: false,
        shift: false,
        meta: false,
    };
    const CTRL: Modifiers = Modifiers { ctrl: true, ..NONE };
    const ALT: Modifiers = Modifiers { alt: true, ..NONE };
    const SHIFT: Modifiers = Modifiers {
        shift: true,
        ..NONE
    };

    fn k(code: KeyCode, mods: Modifiers) -> Key {
        Key { code, mods }
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
        assert_eq!(
            parse_keystroke("command+i"),
            Some(k(Char('i'), Modifiers { meta: true, ..NONE }))
        );
        assert_eq!(parse_keystroke("ctrl+foo"), None);
        assert_eq!(parse_keystroke("a+"), None);
        assert_eq!(parse_keystroke(""), None);
    }

    #[test]
    fn should_parse_the_plus_key_and_reject_a_keystroke_without_a_key() {
        use KeyCode::*;

        let plus = ["+", "++", "+x", "ctrl++"].map(parse_keystroke);
        let no_key = ["a+", "ctrl+", "", "f256"].map(parse_keystroke);
        let digits = ["f0", "f01", "f99"].map(parse_keystroke);

        // `+` alone, doubled or after a modifier is the plus key; `+x` is a plain x (the empty modifier is ignored).
        assert_eq!(
            plus,
            [
                Some(k(Char('+'), NONE)),
                Some(k(Char('+'), NONE)),
                Some(k(Char('x'), NONE)),
                Some(k(Char('+'), CTRL)),
            ]
        );
        assert_eq!(no_key, [None, None, None, None]);
        assert_eq!(
            digits,
            [
                Some(k(F(0), NONE)),
                Some(k(F(1), NONE)),
                Some(k(F(99), NONE))
            ]
        );
    }
}
