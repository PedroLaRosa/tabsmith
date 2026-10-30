use super::*;
use crate::completion::domain::test_fixtures::{comp, item, settings};

fn subs(q: &str) -> Vec<Suggestion> {
    ["checkout", "cherry-pick", "clone", "commit"]
        .iter()
        .filter(|n| n.starts_with(q))
        .map(|n| item(n, SuggestionKind::Subcommand, q))
        .collect()
}

/// Menu after typing `git c`, visible.
fn typed(s: &Settings) -> Menu {
    let mut m = Menu::new();
    m.on_completion(comp("g", 1, vec![]), s, Instant::now());
    m.on_completion(comp("gi", 1, vec![]), s, Instant::now());
    m.on_completion(comp("git", 1, vec![]), s, Instant::now());
    m.on_completion(comp("git ", 2, subs("")), s, Instant::now());
    m.on_completion(comp("git c", 2, subs("c")), s, Instant::now());
    m
}

#[test]
fn typing_and_space() {
    let s = Settings::default();
    let mut m = Menu::new();
    assert_eq!(m.vis(), MenuVisibility::HiddenUntilKeypress);
    m.on_completion(comp("g", 1, vec![]), &s, Instant::now());
    assert_eq!(m.vis(), MenuVisibility::Visible);
    // Visible with an arg hint but no items: drawn, not intercepting.
    assert!(m.shown(&s) && !m.intercepting(&s));
    m.on_completion(comp("gi", 1, vec![]), &s, Instant::now());
    m.on_completion(comp("git", 1, vec![]), &s, Instant::now());
    m.on_completion(comp("git ", 2, subs("")), &s, Instant::now());
    assert_eq!(m.vis(), MenuVisibility::Visible);
    assert_eq!(m.items().len(), 4);
    assert!(m.intercepting(&s));
    assert!(!m.intercepting(&settings(r#"{"autocomplete.disable": true}"#)));
}

#[test]
fn full_insertion_reopens_only_for_new_arg_or_retrigger() {
    let s = Settings::default();
    // Insertion with a new argument next → visible.
    let mut m = typed(&s);
    let fx = m.handle(KeyAction::InsertSelected, &s);
    assert_eq!(
        fx,
        vec![MenuEffect::Insert {
            ins: Insertion::new(InsertionParts {
                delete: 0,
                text: "heckout".into(),
                cursor_back: 0,
                execute: false
            }),
            full: true,
            record: Some(("git".into(), "checkout".into())),
        }]
    );
    assert_eq!(m.vis(), MenuVisibility::HiddenByInsertion);
    m.on_completion(
        comp("git checkout ", 3, vec![item("main", SuggestionKind::Arg, "")]),
        &s,
        Instant::now(),
    );
    assert_eq!(
        m.vis(),
        MenuVisibility::Visible,
        "multi-char echo is not a large change after an insertion"
    );

    // Flag without a new argument → hidden until the next key.
    let mut m = typed(&s);
    m.handle(KeyAction::InsertSelected, &s);
    m.on_completion(comp("git checkout", 2, vec![]), &s, Instant::now());
    assert_eq!(m.vis(), MenuVisibility::HiddenUntilKeypress);
    m.on_completion(comp("git checkout2", 2, vec![]), &s, Instant::now());
    assert_eq!(m.vis(), MenuVisibility::Visible);

    // Folder `src/` re-triggers its generator → visible.
    let mut m = typed(&s);
    m.on_completion(
        comp(
            "git c ",
            5,
            vec![item("src/", SuggestionKind::Folder, "").with_generator(Some(GeneratorKey::from_raw(7)))],
        ),
        &s,
        Instant::now(),
    );
    m.handle(KeyAction::InsertSelected, &s);
    let mut c = comp("git c src/", 5, vec![item("main.rs", SuggestionKind::File, "")]);
    c = c.with_triggered_generators(vec![GeneratorKey::from_raw(7)]);
    m.on_completion(c, &s, Instant::now());
    assert_eq!(m.vis(), MenuVisibility::Visible);
}

#[test]
fn esc_hides_for_the_line() {
    let s = Settings::default();
    let mut m = typed(&s);
    assert!(m.handle(KeyAction::HideAutocomplete, &s).is_empty());
    assert_eq!(m.vis(), MenuVisibility::HiddenUntilShown);
    m.on_completion(comp("git co", 2, subs("co")), &s, Instant::now());
    m.on_completion(comp("git co ", 3, vec![item("x", SuggestionKind::Arg, "")]), &s, Instant::now());
    assert_eq!(m.vis(), MenuVisibility::HiddenUntilShown);
    assert!(!m.shown(&s));
    // Items are still recomputed (globally interceptable keys like showAutocomplete work).
    assert_eq!(m.items()[0].names()[0], "x");
    m.handle(KeyAction::ShowAutocomplete, &s);
    assert_eq!(m.vis(), MenuVisibility::Visible);
    m.handle(KeyAction::ToggleAutocomplete, &s);
    assert_eq!(m.vis(), MenuVisibility::HiddenUntilShown);
    m.handle(KeyAction::ToggleAutocomplete, &s);
    assert_eq!(m.vis(), MenuVisibility::Visible);
}

#[test]
fn backspace_into_previous_token_and_large_change() {
    let s = Settings::default();
    let mut m = typed(&s);
    m.handle(KeyAction::HideAutocomplete, &s);
    m.on_completion(comp("git ", 2, subs("")), &s, Instant::now());
    m.on_completion(comp("git", 1, vec![]), &s, Instant::now());
    assert_eq!(
        m.vis(),
        MenuVisibility::HiddenUntilKeypress,
        "backspace into the previous token overrides Esc"
    );
    m.on_completion(comp("gi", 1, vec![]), &s, Instant::now());
    assert_eq!(m.vis(), MenuVisibility::Visible);

    // A paste is a large change.
    let mut m = typed(&s);
    m.on_completion(comp("git commit -m 'x' --amend", 9, vec![]), &s, Instant::now());
    assert_eq!(m.vis(), MenuVisibility::HiddenUntilKeypress);
    // The list is refreshed while hidden (nothing is drawn): show/toggle/Tab never act on stale items.
    assert!(!m.has_items());
}

#[test]
fn hidden_until_keypress_refreshes_items() {
    let s = settings(r#"{"autocomplete.onlyShowOnTab": true}"#);
    let mut m = typed(&s);
    for q in ["ch", "che", "chec"] {
        m.on_completion(comp(&format!("git {q}"), 2, subs(q)), &s, Instant::now());
    }
    assert_eq!(m.items().len(), 1);
    // Up recalls `echo hi`: a large change hides the menu; Tab must not type `kout` into it.
    m.on_completion(
        comp("echo hi", 3, vec![item("hi.txt", SuggestionKind::File, "hi")]),
        &s,
        Instant::now(),
    );
    assert_eq!(m.vis(), MenuVisibility::HiddenUntilKeypress);
    let fx = m.show_on_tab(&s);
    assert!(
        matches!(&fx[..], [MenuEffect::Insert { record: Some((c, n)), .. }] if c == "echo" && n == "hi.txt"),
        "{fx:?}"
    );
}

#[test]
fn large_change_counts_skipped_requests() {
    let s = Settings::default();
    let at = |req, buffer: &str, arg, items| comp(buffer, arg, items).with_request_id(req);
    let mut m = Menu::new();
    m.on_completion(at(1, "g", 1, vec![]), &s, Instant::now());
    m.on_completion(at(2, "gi", 1, vec![]), &s, Instant::now());
    m.on_completion(at(3, "git", 1, vec![]), &s, Instant::now());
    // The reply for `git ` (request 4) was skipped by the engine: two edits over two requests is typing.
    m.on_completion(at(5, "git c", 2, subs("c")), &s, Instant::now());
    assert_eq!(m.vis(), MenuVisibility::Visible);
    // A paste is still large when a request was skipped before it.
    m.on_completion(at(7, "git commit -m x", 3, vec![]), &s, Instant::now());
    assert_eq!(m.vis(), MenuVisibility::HiddenUntilKeypress);
    m.on_completion(at(8, "git commit -m x ", 3, vec![]), &s, Instant::now());
    assert_eq!(m.vis(), MenuVisibility::Visible, "keypress");
    // Two requests, but `x ` → `zz` needs four edits (not prefix related): large.
    m.on_completion(at(10, "git commit -m zz", 3, vec![]), &s, Instant::now());
    assert_eq!(m.vis(), MenuVisibility::HiddenUntilKeypress);
}

#[test]
fn error_reset_and_loading() {
    let s = Settings::default();
    let mut m = typed(&s);
    m.handle(KeyAction::ToggleHistoryMode, &s);
    m.handle(KeyAction::ToggleDescription, &s);
    m.on_completion(CompletionResult::error(9, "git c".into(), 5), &s, Instant::now());
    assert_eq!(m.vis(), MenuVisibility::HiddenUntilKeypress);
    assert!(!m.has_items() && !m.history_mode() && m.last().is_none());
    assert!(m.popout(&s));
    let mut c = comp("g", 1, vec![]);
    c = c.with_loading(true);
    m.on_completion(c, &s, Instant::now());
    assert!(!m.loading(Instant::now()), "spinner only after 200 ms");
    m.loading_since = Some(Instant::now() - Duration::from_millis(300));
    assert!(m.loading(Instant::now()));
    m.on_completion(comp("gi", 1, vec![]), &s, Instant::now());
    assert!(!m.loading(Instant::now()));
}

#[test]
fn should_not_report_loading_at_exactly_200_ms_and_report_it_after() {
    let s = Settings::default();
    let t0 = Instant::now();
    let mut m = Menu::new();
    m.on_completion(comp("g", 1, vec![]).with_loading(true), &s, t0);

    let at_the_limit = m.loading(t0 + Duration::from_millis(200));
    let after_the_limit = m.loading(t0 + Duration::from_millis(201));

    assert!(!at_the_limit);
    assert!(after_the_limit);
}

#[test]
fn should_keep_the_first_loading_start_across_consecutive_loading_replies() {
    let s = Settings::default();
    let t0 = Instant::now();
    let mut m = Menu::new();
    m.on_completion(comp("g", 1, vec![]).with_loading(true), &s, t0);
    let generators = comp("g", 1, vec![]).with_loading(true).with_phase(CompletionPhase::Generators);

    m.on_completion(generators, &s, t0 + Duration::from_millis(150));

    assert!(m.loading(t0 + Duration::from_millis(201)), "counted from the first reply, not the second");
}

#[test]
fn should_stop_loading_on_the_first_reply_that_is_not_loading() {
    let s = Settings::default();
    let t0 = Instant::now();
    let mut m = Menu::new();
    m.on_completion(comp("g", 1, vec![]).with_loading(true), &s, t0);

    m.on_completion(comp("gi", 1, vec![]), &s, t0 + Duration::from_millis(100));

    assert!(!m.loading(t0 + Duration::from_millis(300)));
}

#[test]
fn only_show_on_tab() {
    let s = settings(r#"{"autocomplete.onlyShowOnTab": true}"#);
    let mut m = Menu::new();
    m.on_completion(comp("g", 1, vec![]), &s, Instant::now());
    assert_eq!(m.vis(), MenuVisibility::HiddenUntilShown, "never auto-opens");
    m.on_completion(comp("gi", 1, vec![]), &s, Instant::now());
    m.on_completion(comp("git", 1, vec![]), &s, Instant::now());
    m.on_completion(comp("git ", 2, subs("")), &s, Instant::now());
    m.on_completion(comp("git c", 2, subs("c")), &s, Instant::now());
    assert_eq!(m.vis(), MenuVisibility::HiddenUntilShown);
    assert_eq!(m.items().len(), 4);
    assert!(m.show_on_tab(&s).is_empty());
    assert_eq!(m.vis(), MenuVisibility::Visible);
    m.on_completion(comp("git co", 2, subs("co")), &s, Instant::now());
    assert_eq!(m.vis(), MenuVisibility::Visible, "stays open within the argument");
    m.on_completion(
        comp("git co ", 3, vec![item("main", SuggestionKind::Arg, "")]),
        &s,
        Instant::now(),
    );
    assert_eq!(
        m.vis(),
        MenuVisibility::HiddenUntilShown,
        "every new argument hides again"
    );
    let fx = m.show_on_tab(&s);
    assert!(
        matches!(&fx[..], [MenuEffect::Insert { full: true, .. }]),
        "single item: Tab inserts it"
    );
}

#[test]
fn navigate_wrap_hide_history() {
    let s = Settings::default();
    let mut m = typed(&s);
    m.handle(KeyAction::NavigateDown, &s);
    m.handle(KeyAction::NavigateDown, &s);
    m.handle(KeyAction::NavigateDown, &s);
    m.handle(KeyAction::NavigateDown, &s);
    assert_eq!(m.selected(), 3, "clamped at the bottom");
    for _ in 0..3 {
        m.handle(KeyAction::NavigateUp, &s);
    }
    assert_eq!((m.selected(), m.vis()), (0, MenuVisibility::Visible));
    m.handle(KeyAction::NavigateUp, &s);
    assert_eq!(m.vis(), MenuVisibility::HiddenUntilKeypress, "Up at index 0 hides");

    let s = settings(r#"{"autocomplete.navigateToHistory": true}"#);
    let mut m = typed(&s);
    assert_eq!(m.handle(KeyAction::NavigateUp, &s), vec![MenuEffect::Rerequest]);
    assert!(m.history_mode() && m.vis() == MenuVisibility::Visible);

    let s = settings(r#"{"autocomplete.scrollWrapAround": true, "autocomplete.height": 2}"#);
    let mut m = typed(&s);
    m.handle(KeyAction::NavigateUp, &s);
    assert_eq!((m.selected(), m.scroll_top()), (3, 2));
    m.handle(KeyAction::NavigateDown, &s);
    assert_eq!((m.selected(), m.scroll_top()), (0, 0));
    m.handle(KeyAction::SelectSuggestion(2), &s);
    assert_eq!(m.selected(), 1);
}

#[test]
fn sticky_selection_across_recompute() {
    let s = Settings::default();
    let mut m = typed(&s);
    m.handle(KeyAction::NavigateDown, &s); // cherry-pick
                                        // Generator phase reply reorders the same items.
    let mut c = comp("git c", 2, subs("c").into_iter().rev().collect());
    c = c.with_phase(CompletionPhase::Generators);
    m.on_completion(c, &s, Instant::now());
    assert_eq!(m.items()[m.selected()].names()[0], "cherry-pick");
    // Item no longer listed → back to the top, not sticky.
    m.on_completion(comp("git cl", 2, subs("cl")), &s, Instant::now());
    assert_eq!(m.selected(), 0);
    m.on_completion(comp("git c", 2, subs("c")), &s, Instant::now());
    assert_eq!(m.selected(), 0, "not sticky after the reset");
    // Typing keeps the selection: the query-stripped insert text changes, the item does not.
    m.on_completion(comp("git ch", 2, subs("ch")), &s, Instant::now());
    m.handle(KeyAction::NavigateDown, &s);
    m.on_completion(comp("git che", 2, subs("che")), &s, Instant::now());
    assert_eq!(m.items()[m.selected()].names()[0], "cherry-pick");
}

fn prefix_fx(m: &mut Menu) -> Vec<MenuEffect> {
    m.handle(KeyAction::InsertCommonPrefix, &Settings::default())
}

#[test]
fn common_prefix() {
    let s = Settings::default();
    // checkout + cherry-pick → "che" → types "e" (query "ch" already typed), menu stays visible.
    let mut m = typed(&s);
    m.on_completion(comp("git ch", 2, subs("ch")), &s, Instant::now());
    let fx = prefix_fx(&mut m);
    assert_eq!(
        fx,
        vec![MenuEffect::Insert {
            ins: Insertion::new(InsertionParts {
                delete: 0,
                text: "e".into(),
                cursor_back: 0,
                execute: false
            }),
            full: false,
            record: None
        }]
    );
    assert_eq!(m.vis(), MenuVisibility::Visible);
    assert!(m.just_inserted);

    // Typed in another case → delete the query and retype in the selected item's casing.
    let fx = prefix_fx(&mut with(
        ["checkout", "cherry-pick"]
            .iter()
            .map(|n| item(n, SuggestionKind::Subcommand, "CH"))
            .collect(),
    ));
    assert!(
        matches!(&fx[..], [MenuEffect::Insert { ins, full: false, .. }] if ins.delete() == 2 && ins.text() == "che")
    );

    // No remaining prefix → bell; a single item → full insertion.
    let mut m = typed(&s);
    assert_eq!(prefix_fx(&mut m), vec![MenuEffect::Bell]);
    m.on_completion(comp("git cl", 2, subs("cl")), &s, Instant::now());
    assert!(matches!(
        &prefix_fx(&mut m)[..],
        [MenuEffect::Insert { full: true, .. }]
    ));

    // Mixed types: only items of the selected item's type count; file and folder match each other.
    let mut m = with(vec![
        item("src/", SuggestionKind::Folder, "s"),
        item("setup.py", SuggestionKind::File, "s"),
        item("--src-dir", SuggestionKind::Option, "s"),
    ]);
    assert_eq!(
        prefix_fx(&mut m),
        vec![MenuEffect::Bell],
        "lcp of src/ and setup.py is the query"
    );
    let mut m = with(vec![
        item("src/", SuggestionKind::Folder, "s"),
        item("srv/", SuggestionKind::Folder, "s"),
        item("stuff", SuggestionKind::Subcommand, "s"),
    ]);
    let fx = prefix_fx(&mut m);
    assert!(
        matches!(&fx[..], [MenuEffect::Insert { ins, full: false, .. }] if ins.delete() == 0 && ins.text() == "r")
    );

    // `../` only matches `../`: with `..foo/` selected, `../` is not a candidate → full insertion.
    let mut m = with(vec![
        item("..foo/", SuggestionKind::Folder, ""),
        item("../", SuggestionKind::Folder, ""),
    ]);
    assert!(matches!(
        &prefix_fx(&mut m)[..],
        [MenuEffect::Insert { full: true, .. }]
    ));

    // Shared prefix equal to the full insertion → full insertion (foo vs foobar, foo selected).
    let mut m = with(vec![
        item("foo", SuggestionKind::Arg, "f"),
        item("foobar", SuggestionKind::Arg, "f"),
    ]);
    assert!(matches!(
        &prefix_fx(&mut m)[..],
        [MenuEffect::Insert { full: true, .. }]
    ));

    // Escaped spaces: the raw token `my\ d` (5 chars) is deleted and retyped escaped (D22).
    let mk = |n: &str| {
        item(n, SuggestionKind::Folder, "my d").with_insertion(Insertion::new(InsertionParts {
            delete: 5,
            text: n.replace(' ', "\\ "),
            cursor_back: 0,
            execute: false,
        }))
    };
    let fx = prefix_fx(&mut with(vec![mk("my dir/"), mk("my dirt/")]));
    assert!(
        matches!(&fx[..], [MenuEffect::Insert { ins, .. }] if ins.delete() == 5 && ins.text() == "my\\ dir")
    );

    // A shared prefix with shell syntax (`zz;` of branches `zz;a`, `zz;b`) is never typed unquoted → bell.
    let quoted = |n: &str, q: &str, ins: &str| {
        item(n, SuggestionKind::File, q).with_insertion(Insertion::new(InsertionParts {
            delete: q.chars().count(),
            text: ins.into(),
            cursor_back: 0,
            execute: false,
        }))
    };
    let mut m = with(vec![
        quoted("zz;a", "z", "'zz;a'"),
        quoted("zz;b", "z", "'zz;b'"),
    ]);
    assert_eq!(prefix_fx(&mut m), vec![MenuEffect::Bell]);
    // r3-common-prefix-var-ref-bell: `$XDG_` of `$XDG_CONFIG_HOME` rows is typed (the engine leaves a variable
    // reference bare); files `$foo1.txt` (quoted by the engine) still ring.
    let fx = prefix_fx(&mut with(vec![
        item("$XDG_CONFIG_HOME", SuggestionKind::Arg, "$XDG"),
        item("$XDG_DATA_HOME", SuggestionKind::Arg, "$XDG"),
    ]));
    assert!(
        matches!(&fx[..], [MenuEffect::Insert { ins, full: false, .. }] if ins.delete() == 0 && ins.text() == "_"),
        "{fx:?}"
    );
    let mut m = with(vec![
        quoted("$foo1.txt", "$f", "'$foo1.txt'"),
        quoted("$foo2.txt", "$f", "'$foo2.txt'"),
    ]);
    assert_eq!(prefix_fx(&mut m), vec![MenuEffect::Bell]);
    // r3-pwsh-common-prefix-backslash: pwsh quotes `my dir1.txt`, so `my\ dir` (two pwsh args) is never typed
    let mut m = with(vec![
        quoted("my dir1.txt", "my", "'my dir1.txt'"),
        quoted("my dir2.txt", "my", "'my dir2.txt'"),
    ]);
    assert_eq!(prefix_fx(&mut m), vec![MenuEffect::Bell]);
    let fx = prefix_fx(&mut with(vec![
        quoted("my dir1.txt", "my", "my\\ dir1.txt"),
        quoted("my dir2.txt", "my", "my\\ dir2.txt"),
    ]));
    assert!(
        matches!(&fx[..], [MenuEffect::Insert { ins, .. }] if ins.delete() == 0 && ins.text() == "\\ dir"),
        "{fx:?}"
    );

    // Auto-execute / special → failure.
    let mut m = with(vec![
        item("↪", SuggestionKind::AutoExecute, "").with_original_kind(Some(SuggestionKind::Special)),
        item("x", SuggestionKind::Arg, ""),
    ]);
    assert_eq!(prefix_fx(&mut m), vec![MenuEffect::Bell]);
    // …and `OrNavigateDown` falls back to navigation.
    assert!(m
        .handle(KeyAction::InsertCommonPrefixOrNavigateDown, &s)
        .is_empty());
    assert_eq!(m.selected(), 1);
}

/// Visible menu over `items` (one-char buffer: not a large change).
fn with(items: Vec<Suggestion>) -> Menu {
    let mut m = Menu::new();
    m.on_completion(comp("x", 1, items), &Settings::default(), Instant::now());
    assert_eq!(m.vis(), MenuVisibility::Visible);
    m
}

#[test]
fn execute_and_record() {
    let s = Settings::default();
    let mut m = typed(&s);
    let fx = m.handle(KeyAction::InsertSelectedAndExecute, &s);
    assert!(matches!(
        &fx[..],
        [MenuEffect::Insert {
            ins,
            ..
        }] if ins.executes()
    ));
    let mut m = typed(&s);
    m.on_completion(
        comp("git c ", 3, vec![item("↪", SuggestionKind::AutoExecute, "")]),
        &s,
        Instant::now(),
    );
    assert!(matches!(
        &m.handle(KeyAction::InsertSelected, &s)[..],
        [MenuEffect::Insert { record: None, .. }]
    ));
    assert_eq!(
        m.handle(KeyAction::Execute, &s),
        vec![MenuEffect::Write(b"\r".to_vec())]
    );
    let mut m = typed(&s);
    m.handle(KeyAction::ShowDescription, &s);
    assert!(m.popout(&s));
    m.handle(KeyAction::HideDescription, &s);
    assert!(!m.popout(&s));
    let always = settings(r#"{"autocomplete.alwaysShowDescription": true}"#);
    m.handle(KeyAction::ToggleDescription, &always);
    assert!(
        m.popout(&always) && !m.popout(&s),
        "toggle is a no-op with alwaysShowDescription"
    );
}

#[test]
fn should_insert_a_lone_special_row_in_full() {
    // One row decides before its kind is looked at: a lone special row (or `↪`) is inserted whole.
    let mut lone = with(vec![item("<path>", SuggestionKind::Special, "")]);
    let mut lone_auto = with(vec![item("↪", SuggestionKind::AutoExecute, "")]);
    let mut two = with(vec![
        item("<path>", SuggestionKind::Special, ""),
        item("<dir>", SuggestionKind::Special, ""),
    ]);

    let fx = prefix_fx(&mut lone);
    let fx_auto = prefix_fx(&mut lone_auto);
    let fx_two = prefix_fx(&mut two);

    assert!(matches!(
        &fx[..],
        [MenuEffect::Insert { ins, full: true, record: Some((_, name)) }] if ins.text() == "<path>" && name == "<path>"
    ));
    assert_eq!(lone.vis(), MenuVisibility::HiddenByInsertion);
    assert!(matches!(
        &fx_auto[..],
        [MenuEffect::Insert {
            full: true,
            record: None,
            ..
        }]
    ));
    assert_eq!(
        fx_two,
        vec![MenuEffect::Bell],
        "two special rows share no prefix to type"
    );
}

#[test]
fn should_not_insert_an_auto_execute_row_in_full_on_a_shared_prefix() {
    // A row that matches nothing else of its kind (one match) and a prefix equal to the whole insertion
    // (`foo` of `foobar`, `foobaz`) both insert in full for a normal row and ring the bell for an auto-execute one.
    let auto = |name: &str| {
        item(name, SuggestionKind::AutoExecute, "")
            .with_original_kind(Some(SuggestionKind::Arg))
    };
    let mut single = with(vec![auto("↪"), item("foo", SuggestionKind::Arg, "")]);
    let mut single_plain = with(vec![
        item("foo", SuggestionKind::Arg, ""),
        item("--bar", SuggestionKind::Option, ""),
    ]);
    let mut whole = with(vec![
        auto("foo"),
        item("foobar", SuggestionKind::Arg, ""),
        item("foobaz", SuggestionKind::Arg, ""),
    ]);
    let mut whole_plain = with(vec![
        item("foo", SuggestionKind::Arg, ""),
        item("foobar", SuggestionKind::Arg, ""),
        item("foobaz", SuggestionKind::Arg, ""),
    ]);

    let fx_single = prefix_fx(&mut single);
    let fx_single_plain = prefix_fx(&mut single_plain);
    let fx_whole = prefix_fx(&mut whole);
    let fx_whole_plain = prefix_fx(&mut whole_plain);

    assert_eq!(fx_single, vec![MenuEffect::Bell]);
    assert_eq!(fx_whole, vec![MenuEffect::Bell]);
    assert_eq!(
        single.vis(),
        MenuVisibility::Visible,
        "a failure leaves the menu untouched"
    );
    assert!(!single.just_inserted && !whole.just_inserted);
    assert!(matches!(
        &fx_single_plain[..],
        [MenuEffect::Insert { full: true, .. }]
    ));
    assert!(matches!(
        &fx_whole_plain[..],
        [MenuEffect::Insert { full: true, .. }]
    ));
}
