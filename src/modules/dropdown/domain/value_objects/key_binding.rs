use crate::modules::dropdown::domain::value_objects::key_action::KeyAction;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum KeyBinding {
    Action(KeyAction),
    /// `"ignore"`: never intercept this key.
    Ignore,
    /// Unknown action id: swallow the key, do nothing.
    Unknown,
}
