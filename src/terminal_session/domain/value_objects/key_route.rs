use crate::dropdown::domain::value_objects::key_action::KeyAction;

/// What one decoded key does (SPEC §7.2.4).
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum KeyRoute {
    /// The key would act on a menu or ghost text for a buffer the shell may no longer have: keep it and the rest of
    /// its read until the echo and its reply came.
    Hold,
    /// The menu runs the action (a bound key while it intercepts, or show/toggle while it is hidden).
    MenuAction(KeyAction),
    /// A key bound to an unknown action while the menu intercepts: nothing happens, the frame is drawn again.
    Swallow,
    /// `autocomplete.onlyShowOnTab`: Tab shows the hidden menu.
    ShowOnTab,
    /// Accept this much of the ghost text (the whole of it, or its next word).
    AcceptGhost(String),
    /// The key goes to the shell as it is.
    Forward,
}
