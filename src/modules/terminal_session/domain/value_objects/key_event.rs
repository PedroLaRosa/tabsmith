use crate::modules::dropdown::domain::value_objects::key::Key;
use std::ops::Range;

/// A decoded key and the byte range of the chunk it came from.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct KeyEvent {
    key: Key,
    bytes: Range<usize>,
}

impl KeyEvent {
    pub(crate) fn new(key: Key, bytes: Range<usize>) -> Self {
        Self { key, bytes }
    }

    pub(crate) fn key(&self) -> Key {
        self.key
    }

    pub(crate) fn bytes(&self) -> Range<usize> {
        self.bytes.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::dropdown::domain::value_objects::key_code::KeyCode;
    use crate::modules::dropdown::domain::value_objects::modifiers::Modifiers;

    #[test]
    fn should_hold_the_key_and_the_byte_range_of_its_chunk() {
        let key = Key::new(KeyCode::Char('a'), Modifiers::default());

        let event = KeyEvent::new(key, 3..5);

        assert_eq!(event.key(), key);
        assert_eq!(event.bytes(), 3..5);
    }
}
