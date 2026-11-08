use crate::modules::dropdown::domain::value_objects::key_code::KeyCode;
use crate::modules::dropdown::domain::value_objects::modifiers::Modifiers;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) struct Key {
    code: KeyCode,
    modifiers: Modifiers,
}

impl Key {
    pub(crate) fn new(code: KeyCode, modifiers: Modifiers) -> Self {
        Self { code, modifiers }
    }

    pub(crate) fn code(&self) -> KeyCode {
        self.code
    }

    pub(crate) fn modifiers(&self) -> Modifiers {
        self.modifiers
    }

    /// The same key plus alt, as `ESC <key>` means; the case is left alone, `normalize` is the keymap's.
    pub(crate) fn with_alt(self) -> Self {
        Self::new(self.code, self.modifiers.with_alt(true))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `ESC <key>` adds alt to the key as decoded: the other modifiers and the case of the character stay.
    #[test]
    fn should_keep_the_modifiers_of_a_key_when_adding_alt() {
        let ctrl_right = Key::new(KeyCode::Right, Modifiers::none().with_ctrl(true));
        let upper_f = Key::new(KeyCode::Char('F'), Modifiers::none());

        let with_alt = ctrl_right.with_alt();

        assert_eq!(with_alt.code(), KeyCode::Right);
        assert_eq!(
            with_alt.modifiers(),
            Modifiers::none().with_ctrl(true).with_alt(true)
        );
        assert_eq!(upper_f.with_alt().code(), KeyCode::Char('F'));
    }
}
