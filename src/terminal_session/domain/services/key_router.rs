use crate::dropdown::domain::value_objects::key::Key;
use crate::dropdown::domain::value_objects::key_code::KeyCode;
use crate::dropdown::domain::value_objects::modifiers::Modifiers;

/// Ghost accept keys (SPEC §8.3): `Some(true)` = whole ghost, `Some(false)` = next word.
pub(crate) fn ghost_key(k: &Key) -> Option<bool> {
    let m = k.modifiers();
    let plain = m == Modifiers::default();
    let ctrl = m == Modifiers::none().with_ctrl(true);
    let alt = m == Modifiers::none().with_alt(true);
    match k.code() {
        KeyCode::Right | KeyCode::End if plain => Some(true),
        KeyCode::Char('f' | 'F' | 'e' | 'E') if ctrl => Some(true),
        KeyCode::Char('f') if alt => Some(false),
        KeyCode::Right if ctrl => Some(false),
        _ => None,
    }
}

pub(crate) fn is_ctrl(k: &Key, c: char) -> bool {
    k.modifiers().ctrl() && matches!(k.code(), KeyCode::Char(x) if x.eq_ignore_ascii_case(&c))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ghost_keys_and_words_keys() {
        let k = |code, mods| Key::new(code, mods);
        let ctrl = Modifiers::none().with_ctrl(true);
        let alt = Modifiers::none().with_alt(true);
        assert_eq!(ghost_key(&k(KeyCode::Right, Modifiers::default())), Some(true));
        assert_eq!(ghost_key(&k(KeyCode::Char('e'), ctrl)), Some(true));
        assert_eq!(ghost_key(&k(KeyCode::Char('f'), alt)), Some(false));
        assert_eq!(ghost_key(&k(KeyCode::Right, ctrl)), Some(false));
        assert_eq!(ghost_key(&k(KeyCode::Tab, Modifiers::default())), None);
    }
}
