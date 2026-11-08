use crate::modules::dropdown::domain::value_objects::key::Key;
use crate::modules::dropdown::domain::value_objects::key_code::KeyCode;
use crate::modules::dropdown::domain::value_objects::modifiers::Modifiers;

/// Parse a keystroke like `ctrl+k`, `shift+tab`, `+`, `ctrl++`, `f12` (SPEC §7.2.2).
pub(crate) fn parse_keystroke(s: &str) -> Option<Key> {
    let mut mods = Modifiers::default();
    let mut rest = s;
    let name = loop {
        match rest.split_once('+') {
            None | Some(("", "")) => break rest,
            Some((m, k)) => {
                match m {
                    "ctrl" | "control" => mods = mods.with_ctrl(true),
                    "shift" => mods = mods.with_shift(true),
                    "alt" | "option" => mods = mods.with_alt(true),
                    "meta" | "command" => mods = mods.with_meta(true),
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
            if mods.shift() && c.is_ascii_lowercase() {
                c = c.to_ascii_uppercase();
                mods = mods.with_shift(false);
            }
            KeyCode::Char(c)
        }
    };
    Some(Key::new(code, mods))
}

/// ctrl/alt + ASCII letter match both cases.
pub(crate) fn normalize(k: Key) -> Key {
    if let KeyCode::Char(c) = k.code() {
        if (k.modifiers().ctrl() || k.modifiers().alt()) && c.is_ascii_alphabetic() {
            return Key::new(KeyCode::Char(c.to_ascii_lowercase()), k.modifiers());
        }
    }
    k
}

#[cfg(test)]
mod tests {
    use super::*;

    const NONE: Modifiers = Modifiers::none();
    const CTRL: Modifiers = NONE.with_ctrl(true);
    const ALT: Modifiers = NONE.with_alt(true);
    const SHIFT: Modifiers = NONE.with_shift(true);

    fn k(code: KeyCode, mods: Modifiers) -> Key {
        Key::new(code, mods)
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
            Some(k(Char('i'), NONE.with_meta(true)))
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
