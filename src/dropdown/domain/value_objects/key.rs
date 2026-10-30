use crate::dropdown::domain::value_objects::key_code::KeyCode;
use crate::dropdown::domain::value_objects::modifiers::Modifiers;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) struct Key {
    pub(crate) code: KeyCode,
    pub(crate) mods: Modifiers,
}
