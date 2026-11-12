use crate::modules::dropdown::domain::value_objects::key::Key;
use crate::modules::dropdown::domain::value_objects::key_action::KeyAction;
use crate::modules::dropdown::domain::value_objects::key_binding::KeyBinding;
use crate::modules::dropdown::domain::value_objects::key_code::KeyCode;
use crate::modules::dropdown::domain::value_objects::keymap::Keymap;
use crate::modules::dropdown::domain::value_objects::modifiers::Modifiers;
use crate::modules::ghost_text::domain::value_objects::ghost_text::GhostText;
use crate::modules::terminal_session::domain::value_objects::key_facts::KeyFacts;
use crate::modules::terminal_session::domain::value_objects::key_route::KeyRoute;

/// What one decoded key does (SPEC §7.2.4): the menu acts on bound keys while it intercepts, a hidden menu shows on
/// Tab (`onlyShowOnTab`) or on show/toggle, the ghost text is accepted by Right/End/Ctrl-F/Ctrl-E (all of it) and
/// Alt-F/Ctrl-Right (next word), everything else goes to the shell. A key that would act while an unmodelled key's
/// echo is pending is held.
pub(crate) fn route(key: &Key, keymap: &Keymap, facts: &KeyFacts) -> KeyRoute {
    let prompt = facts.prompt();
    // A logically shown menu acts on keys only while the render guards let it be seen.
    let shown = prompt && facts.menu_intercepts();
    let intercepting = shown && facts.drawable();
    let global = prompt && !shown && !facts.autocomplete_disabled() && facts.menu_has_items();
    let binding = if prompt { keymap.lookup(key) } else { None };
    let tab_show = global
        && facts.only_show_on_tab()
        && key.code() == KeyCode::Tab
        && key.modifiers() == Modifiers::default();
    let ghost = ghost_key(key)
        .filter(|_| prompt && facts.drawable())
        .zip(facts.ghost_text());
    let acts = ghost.is_some()
        || tab_show
        || (intercepting && matches!(binding, Some(KeyBinding::Action(_) | KeyBinding::Unknown)));
    if acts && facts.waiting_for_echo() {
        return KeyRoute::Hold;
    }
    match binding {
        Some(KeyBinding::Action(action)) if intercepting => KeyRoute::MenuAction(action),
        Some(KeyBinding::Unknown) if intercepting => KeyRoute::Swallow,
        Some(KeyBinding::Action(_) | KeyBinding::Unknown | KeyBinding::Ignore) | None if tab_show => {
            KeyRoute::ShowOnTab
        }
        Some(KeyBinding::Action(action @ (KeyAction::ShowAutocomplete | KeyAction::ToggleAutocomplete)))
            if global =>
        {
            KeyRoute::MenuAction(action)
        }
        Some(KeyBinding::Action(_) | KeyBinding::Unknown | KeyBinding::Ignore) | None => match ghost {
            Some((true, text)) => KeyRoute::AcceptGhost(text.to_string()),
            Some((false, text)) => KeyRoute::AcceptGhost(GhostText::next_word(text).to_string()),
            None => KeyRoute::Forward,
        },
    }
}

/// A printable key without modifiers: it is echoed by the shell at the end of the line, and two of them in one read
/// make a paste.
pub(crate) fn plain_char(key: &Key) -> Option<char> {
    match key.code() {
        KeyCode::Char(c) if key.modifiers() == Modifiers::default() => Some(c),
        _ => None,
    }
}

/// Enter, Ctrl-C and Ctrl-D end the line: the menu resets before the shell's newline echo can re-anchor it.
pub(crate) fn ends_line(key: &Key) -> bool {
    is_ctrl(key, 'c') || is_ctrl(key, 'd') || is_enter(key)
}

pub(crate) fn is_enter(key: &Key) -> bool {
    key.code() == KeyCode::Enter
}

/// Ghost accept keys (SPEC §8.3): `Some(true)` = whole ghost, `Some(false)` = next word.
fn ghost_key(k: &Key) -> Option<bool> {
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

fn is_ctrl(k: &Key, c: char) -> bool {
    k.modifiers().ctrl() && matches!(k.code(), KeyCode::Char(x) if x.eq_ignore_ascii_case(&c))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::completion::domain::test_fixtures::settings;
    use crate::modules::terminal_session::domain::value_objects::key_facts::KeyFactsParts;

    fn keymap() -> Keymap {
        Keymap::from_settings(&settings("{}"))
    }

    fn key(code: KeyCode) -> Key {
        Key::new(code, Modifiers::default())
    }

    fn at_a_shown_menu() -> KeyFactsParts {
        KeyFactsParts { prompt: true, menu_intercepts: true, drawable: true, menu_has_items: true, ..KeyFactsParts::default() }
    }

    #[test]
    fn should_hand_a_bound_key_to_the_menu_while_it_intercepts() {
        let route = route(&key(KeyCode::Enter), &keymap(), &KeyFacts::new(at_a_shown_menu()));

        assert_eq!(route, KeyRoute::MenuAction(KeyAction::InsertSelected));
    }

    #[test]
    fn should_forward_a_bound_key_once_a_render_guard_hides_the_menu() {
        let facts = KeyFacts::new(KeyFactsParts { drawable: false, ..at_a_shown_menu() });

        let route = route(&key(KeyCode::Enter), &keymap(), &facts);

        assert_eq!(route, KeyRoute::Forward);
    }

    #[test]
    fn should_forward_every_key_off_the_command_line() {
        let facts = KeyFacts::new(KeyFactsParts { prompt: false, ..at_a_shown_menu() });

        let route = route(&key(KeyCode::Enter), &keymap(), &facts);

        assert_eq!(route, KeyRoute::Forward);
    }

    #[test]
    fn should_swallow_a_key_bound_to_an_unknown_action_while_the_menu_intercepts() {
        let keymap = Keymap::from_settings(&settings(r#"{"autocomplete.keybindings.enter": "nonsense"}"#));

        let route = route(&key(KeyCode::Enter), &keymap, &KeyFacts::new(at_a_shown_menu()));

        assert_eq!(route, KeyRoute::Swallow);
    }

    #[test]
    fn should_forward_a_key_bound_to_ignore() {
        let keymap = Keymap::from_settings(&settings(r#"{"autocomplete.keybindings.enter": "ignore"}"#));

        let route = route(&key(KeyCode::Enter), &keymap, &KeyFacts::new(at_a_shown_menu()));

        assert_eq!(route, KeyRoute::Forward);
    }

    #[test]
    fn should_show_a_hidden_menu_with_items_on_tab_when_only_tab_shows_it() {
        let facts = KeyFacts::new(KeyFactsParts { prompt: true, menu_has_items: true, only_show_on_tab: true, ..KeyFactsParts::default() });

        let route = route(&key(KeyCode::Tab), &keymap(), &facts);

        assert_eq!(route, KeyRoute::ShowOnTab);
    }

    #[test]
    fn should_let_tab_through_a_disabled_menu() {
        let facts = KeyFacts::new(KeyFactsParts {
            prompt: true,
            menu_has_items: true,
            only_show_on_tab: true,
            autocomplete_disabled: true,
            ..KeyFactsParts::default()
        });

        let route = route(&key(KeyCode::Tab), &keymap(), &facts);

        assert_eq!(route, KeyRoute::Forward);
    }

    #[test]
    fn should_run_show_autocomplete_on_a_hidden_menu_with_items_but_not_other_actions() {
        let keymap = Keymap::from_settings(&settings(
            r#"{"autocomplete.keybindings.control+s": "showAutocomplete", "autocomplete.keybindings.control+x": "navigateUp"}"#,
        ));
        let hidden = KeyFacts::new(KeyFactsParts { prompt: true, menu_has_items: true, ..KeyFactsParts::default() });
        let ctrl = Modifiers::none().with_ctrl(true);

        let show = route(&Key::new(KeyCode::Char('s'), ctrl), &keymap, &hidden);
        let other = route(&Key::new(KeyCode::Char('x'), ctrl), &keymap, &hidden);

        assert_eq!((show, other), (KeyRoute::MenuAction(KeyAction::ShowAutocomplete), KeyRoute::Forward));
    }

    #[test]
    fn should_accept_the_whole_ghost_text_on_right_and_its_next_word_on_ctrl_right() {
        let facts = KeyFacts::new(KeyFactsParts { prompt: true, drawable: true, ghost_text: Some(" -la /tmp".into()), ..KeyFactsParts::default() });
        let ctrl = Modifiers::none().with_ctrl(true);

        let whole = route(&key(KeyCode::Right), &keymap(), &facts);
        let word = route(&Key::new(KeyCode::Right, ctrl), &keymap(), &facts);

        assert_eq!((whole, word), (KeyRoute::AcceptGhost(" -la /tmp".into()), KeyRoute::AcceptGhost(" -la".into())));
    }

    #[test]
    fn should_not_accept_a_ghost_text_the_render_guards_hide() {
        let facts = KeyFacts::new(KeyFactsParts { prompt: true, drawable: false, ghost_text: Some("x".into()), ..KeyFactsParts::default() });

        let route = route(&key(KeyCode::Right), &keymap(), &facts);

        assert_eq!(route, KeyRoute::Forward);
    }

    #[test]
    fn should_hold_a_key_that_would_act_while_an_unmodelled_keys_echo_is_pending() {
        let at_menu = KeyFacts::new(KeyFactsParts { waiting_for_echo: true, ..at_a_shown_menu() });
        let at_ghost = KeyFacts::new(KeyFactsParts { prompt: true, drawable: true, waiting_for_echo: true, ghost_text: Some("x".into()), ..KeyFactsParts::default() });

        let menu = route(&key(KeyCode::Enter), &keymap(), &at_menu);
        let ghost = route(&key(KeyCode::Right), &keymap(), &at_ghost);

        assert_eq!((menu, ghost), (KeyRoute::Hold, KeyRoute::Hold));
    }

    #[test]
    fn should_hold_a_ghost_key_even_when_the_menu_would_take_it() {
        let keymap = Keymap::from_settings(&settings(r#"{"autocomplete.keybindings.right": "insertSelected"}"#));
        let facts = KeyFacts::new(KeyFactsParts { waiting_for_echo: true, ghost_text: Some("x".into()), ..at_a_shown_menu() });

        let route = route(&key(KeyCode::Right), &keymap, &facts);

        assert_eq!(route, KeyRoute::Hold);
    }

    #[test]
    fn should_not_hold_a_key_that_does_not_act() {
        let facts = KeyFacts::new(KeyFactsParts { prompt: true, waiting_for_echo: true, ..KeyFactsParts::default() });

        let route = route(&key(KeyCode::Char('a')), &keymap(), &facts);

        assert_eq!(route, KeyRoute::Forward);
    }

    #[test]
    fn should_count_only_unmodified_chars_as_typed_and_enter_ctrl_c_ctrl_d_as_ending_the_line() {
        let ctrl = Modifiers::none().with_ctrl(true);

        assert_eq!(
            (
                plain_char(&key(KeyCode::Char('a'))),
                plain_char(&Key::new(KeyCode::Char('a'), ctrl)),
                plain_char(&key(KeyCode::Tab)),
                ends_line(&key(KeyCode::Enter)),
                ends_line(&Key::new(KeyCode::Char('C'), ctrl)),
                ends_line(&Key::new(KeyCode::Char('d'), ctrl)),
                ends_line(&key(KeyCode::Char('d'))),
            ),
            (Some('a'), None, None, true, true, true, false)
        );
    }

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
