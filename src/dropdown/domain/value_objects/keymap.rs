use crate::dropdown::domain::services::keystroke_parser::{normalize, parse_keystroke};
use crate::dropdown::domain::value_objects::key::Key;
use crate::dropdown::domain::value_objects::key_action::KeyAction;
use crate::dropdown::domain::value_objects::key_binding::KeyBinding;
use crate::settings::domain::value_objects::settings::Settings;

/// fig.json defaults (D5).
const DEFAULTS: &[(&str, KeyAction)] = &[
    ("enter", KeyAction::InsertSelected),
    ("tab", KeyAction::InsertCommonPrefix),
    ("shift+tab", KeyAction::NavigateUp),
    ("up", KeyAction::NavigateUp),
    ("control+p", KeyAction::NavigateUp),
    ("down", KeyAction::NavigateDown),
    ("control+n", KeyAction::NavigateDown),
    ("esc", KeyAction::HideAutocomplete),
    ("control+k", KeyAction::ToggleDescription),
    ("control+r", KeyAction::ToggleHistoryMode),
];

/// Default bindings (fig.json set, SPEC §7.2.3) overlaid with `autocomplete.keybindings.*`.
pub(crate) struct Keymap {
    bindings: Vec<(Key, KeyBinding)>,
}

impl Keymap {
    pub(crate) fn from_settings(settings: &Settings) -> Keymap {
        let mut map = Keymap {
            bindings: Vec::new(),
        };
        for (k, a) in DEFAULTS {
            map.set(k, KeyBinding::Action(*a));
        }
        for (k, v) in settings.with_prefix("autocomplete.keybindings.") {
            let Some(v) = v.as_str() else { continue };
            let b = match v {
                "ignore" => KeyBinding::Ignore,
                _ => KeyAction::from_id(v).map_or(KeyBinding::Unknown, KeyBinding::Action),
            };
            map.set(k, b);
        }
        map
    }

    /// A later binding for the same key replaces the earlier one; unparsable keystrokes are dropped.
    fn set(&mut self, keystroke: &str, b: KeyBinding) {
        let Some(key) = parse_keystroke(keystroke).map(normalize) else {
            return;
        };
        match self.bindings.iter_mut().find(|(k, _)| *k == key) {
            Some(e) => e.1 = b,
            None => self.bindings.push((key, b)),
        }
    }

    /// KeyBinding for a decoded key (handles case-insensitive ctrl/alt letters; `ESC O x` arrows decode as arrows).
    pub(crate) fn lookup(&self, key: &Key) -> Option<KeyBinding> {
        let key = normalize(*key);
        self.bindings
            .iter()
            .find(|(k, _)| *k == key)
            .map(|(_, b)| *b)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::completion::domain::test_fixtures::settings;
    use crate::dropdown::domain::value_objects::key_code::KeyCode;
    use crate::dropdown::domain::value_objects::modifiers::Modifiers;

    const NONE: Modifiers = Modifiers::none();
    const CTRL: Modifiers = NONE.with_ctrl(true);
    const ALT: Modifiers = NONE.with_alt(true);
    const SHIFT: Modifiers = NONE.with_shift(true);

    fn k(code: KeyCode, mods: Modifiers) -> Key {
        Key::new(code, mods)
    }

    #[test]
    fn should_let_the_lexicographically_last_user_binding_win() {
        // `control+x` < `ctrl+x` and `alt+K` < `alt+k` in byte order; each pair normalises to one key.
        // The text lists the later name first, so the file order is not what decides.
        let km = Keymap::from_settings(&settings(
            r#"{"autocomplete.keybindings.ctrl+x": "navigateDown",
                "autocomplete.keybindings.control+x": "navigateUp",
                "autocomplete.keybindings.alt+k": "showAutocomplete",
                "autocomplete.keybindings.alt+K": "hideAutocomplete"}"#,
        ));

        let ctrl_x = km.lookup(&k(KeyCode::Char('x'), CTRL));
        let alt_k = km.lookup(&k(KeyCode::Char('k'), ALT));

        assert_eq!(ctrl_x, Some(KeyBinding::Action(KeyAction::NavigateDown)));
        assert_eq!(alt_k, Some(KeyBinding::Action(KeyAction::ShowAutocomplete)));
    }

    #[test]
    fn should_ignore_an_unknown_modifier_and_bind_the_plain_key() {
        use KeyCode::*;
        // `Ctrl` is miscased, so the keystroke is the plain `k`; key names are case-sensitive, so `Enter` is no key.
        let km = Keymap::from_settings(&settings(
            r#"{"autocomplete.keybindings.Ctrl+k": "navigateDown",
                "autocomplete.keybindings.Enter": "execute"}"#,
        ));

        let miscased = parse_keystroke("Ctrl+k");
        let unknown_name = parse_keystroke("Enter");
        let plain_k = km.lookup(&k(Char('k'), NONE));
        let ctrl_k = km.lookup(&k(Char('k'), CTRL));
        let enter = km.lookup(&k(Enter, NONE));

        assert_eq!(miscased, Some(k(Char('k'), NONE)));
        assert_eq!(unknown_name, None);
        assert_eq!(plain_k, Some(KeyBinding::Action(KeyAction::NavigateDown)));
        assert_eq!(
            ctrl_k,
            Some(KeyBinding::Action(KeyAction::ToggleDescription)),
            "the real ctrl+k default is untouched"
        );
        assert_eq!(
            enter,
            Some(KeyBinding::Action(KeyAction::InsertSelected)),
            "the dropped override leaves the default"
        );
    }

    #[test]
    fn should_lowercase_only_ctrl_and_alt_letters_when_binding_and_looking_up() {
        use KeyCode::*;
        let km = Keymap::from_settings(&settings(
            r#"{"autocomplete.keybindings.f5": "autocomplete.ignore",
                "autocomplete.keybindings.f6": "ignore",
                "autocomplete.keybindings.f7": true}"#,
        ));

        let ctrl = normalize(k(Char('K'), CTRL));
        let alt = normalize(k(Char('K'), ALT));
        let untouched = [
            k(Char('K'), NONE),
            k(Char('K'), SHIFT),
            k(Char('1'), CTRL),
            k(Char('É'), ALT),
            k(Up, CTRL),
        ]
        .map(normalize);
        let ctrl_shift_k = parse_keystroke("ctrl+shift+k").map(normalize);
        let upper_ctrl_k = km.lookup(&k(Char('K'), CTRL));
        let upper_plain_k = km.lookup(&k(Char('K'), NONE));

        assert_eq!(ctrl, k(Char('k'), CTRL));
        assert_eq!(alt, k(Char('k'), ALT));
        assert_eq!(
            untouched,
            [
                k(Char('K'), NONE),
                k(Char('K'), SHIFT),
                k(Char('1'), CTRL),
                k(Char('É'), ALT),
                k(Up, CTRL)
            ]
        );
        assert_eq!(ctrl_shift_k, Some(k(Char('k'), CTRL)));
        assert_eq!(
            upper_ctrl_k,
            Some(KeyBinding::Action(KeyAction::ToggleDescription)),
            "ctrl+K finds the ctrl+k default"
        );
        assert_eq!(upper_plain_k, None);
        // The id is compared before `autocomplete.` is stripped, so this is an unknown action, not `ignore`.
        assert_eq!(km.lookup(&k(F(5), NONE)), Some(KeyBinding::Unknown));
        assert_eq!(km.lookup(&k(F(6), NONE)), Some(KeyBinding::Ignore));
        assert_eq!(
            km.lookup(&k(F(7), NONE)),
            None,
            "a value that is not text is skipped"
        );
    }
}
