use crate::dropdown::domain::value_objects::key_action::KeyAction;
use crate::dropdown::domain::value_objects::menu_effect::MenuEffect;
use crate::dropdown::domain::value_objects::menu_visibility::MenuVisibility;
use crate::settings::domain::services::settings_catalog::{
    AUTOCOMPLETE_ALWAYS_SHOW_DESCRIPTION, AUTOCOMPLETE_DISABLE, AUTOCOMPLETE_HEIGHT,
    AUTOCOMPLETE_NAVIGATE_TO_HISTORY, AUTOCOMPLETE_ONLY_SHOW_ON_TAB,
    AUTOCOMPLETE_SCROLL_WRAP_AROUND,
};
use crate::settings::domain::value_objects::settings::Settings;
use crate::completion::domain::value_objects::argument_hint::ArgumentHint;
use crate::completion::domain::value_objects::completion_phase::CompletionPhase;
use crate::completion::domain::value_objects::completion_result::CompletionResult;
use crate::completion::domain::value_objects::generator_key::GeneratorKey;
use crate::completion::domain::value_objects::insertion::{Insertion, InsertionParts};
use crate::completion::domain::value_objects::suggestion::Suggestion;
use crate::specs::domain::value_objects::suggestion_kind::SuggestionKind;
use std::time::{Duration, Instant};

/// Dropdown UI state: visibility machine, selection/scroll, actions, common prefix (SPEC §7.1–7.3).
/// Pure: no I/O.
pub(crate) struct Menu {
    vis: MenuVisibility,
    items: Vec<Suggestion>,
    selected: usize,
    /// The user navigated: keep the selection across recomputes.
    sticky: bool,
    scroll_top: usize,
    /// Generator of the last fully inserted item (re-trigger reopens the menu).
    last_inserted_gen: Option<GeneratorKey>,
    just_inserted: bool,
    history_mode: bool,
    popout: bool,
    /// Latest reply; its `items` are moved into `self.items`.
    last: Option<CompletionResult>,
    loading_since: Option<Instant>,
}

/// Max visible list rows (`autocomplete.height`, D14).
pub(crate) fn max_rows(settings: &Settings) -> usize {
    settings.f64(AUTOCOMPLETE_HEIGHT).max(1.0) as usize
}

/// Over `steps` consecutive buffers: `a` is at most `steps` one-char edits from `b`
/// (trie distance). With `steps == 1`: one is a prefix of the other and they differ by < 2 chars.
fn typing(a: &str, b: &str, steps: usize) -> bool {
    let lcp = a.chars().zip(b.chars()).take_while(|(x, y)| x == y).count();
    a.chars().count() + b.chars().count() - 2 * lcp <= steps
}

/// Compares names, type, insertValue and description; `insert.text` has the typed query stripped, so it is
/// left out (it changes on every keystroke).
fn same_item(a: &Suggestion, b: &Suggestion) -> bool {
    a.names() == b.names() && a.kind() == b.kind() && a.description() == b.description()
}

/// Whether the types match (`../` only matches `../`; file and folder match each other).
fn matching_type(a: &Suggestion, b_first: &str, b_kind: SuggestionKind) -> bool {
    if a.first_name() == "../" {
        return b_first == "../";
    }
    let file_like = |k| matches!(k, SuggestionKind::File | SuggestionKind::Folder);
    (file_like(a.kind()) && file_like(b_kind)) || a.kind() == b_kind
}

impl Menu {
    pub(crate) fn new() -> Menu {
        Menu {
            vis: MenuVisibility::HiddenUntilKeypress,
            items: Vec::new(),
            selected: 0,
            sticky: false,
            scroll_top: 0,
            last_inserted_gen: None,
            just_inserted: false,
            history_mode: false,
            popout: false,
            last: None,
            loading_since: None,
        }
    }

    /// Phase-Parse / phase-Generators reply for the latest request (SPEC §7.1). An error reply resets.
    pub(crate) fn on_completion(&mut self, mut c: CompletionResult, settings: &Settings, now: Instant) {
        if c.is_error() {
            return self.error_reset();
        }
        let items = c.take_suggestions();
        self.loading_since = if c.is_loading() {
            self.loading_since.or(Some(now))
        } else {
            None
        };
        if c.phase() == CompletionPhase::Parse {
            // After a reset the comparison is against the initial state: new arg, empty command text.
            let (has_new_arg, backspaced, large) = match &self.last {
                None => (true, false, !typing(c.command_text(), "", 1)),
                Some(l) => {
                    let n = c.tokens().len();
                    // The engine serves only the latest request, so replies for the buffers in between are never
                    // seen: allow one typing edit per request since `l`.
                    // ponytail: a large change followed by skipped typing stays hidden until the next key; exact =
                    // compare with the previous request's command text (engine would have to send it).
                    let steps = c.request_id().saturating_sub(l.request_id()).max(1) as usize;
                    (
                        l.arg_key() != c.arg_key(),
                        n > 0 && n < l.tokens().len() && l.tokens()[n - 1] == c.tokens()[n - 1],
                        !typing(c.command_text(), l.command_text(), steps),
                    )
                }
            };
            let orig = self.vis;
            self.vis = match orig {
                MenuVisibility::HiddenUntilKeypress => MenuVisibility::Visible,
                MenuVisibility::HiddenByInsertion => {
                    let retriggered = self
                        .last_inserted_gen
                        .is_some_and(|g| c.triggered_generators().contains(&g));
                    if has_new_arg || retriggered {
                        MenuVisibility::Visible
                    } else {
                        MenuVisibility::HiddenUntilKeypress
                    }
                }
                v => v,
            };
            if settings.bool(AUTOCOMPLETE_ONLY_SHOW_ON_TAB) {
                self.vis = orig;
                if has_new_arg {
                    self.vis = MenuVisibility::HiddenUntilShown;
                }
            }
            if backspaced || (large && !self.just_inserted) {
                self.vis = MenuVisibility::HiddenUntilKeypress;
            }
            self.just_inserted = false;
        }
        self.last = Some(c);
        self.set_items(items, settings);
    }

    /// Keeps a sticky selection if still listed. While HiddenUntilKeypress nothing is drawn, but the list is still
    /// replaced, so show/toggle/Tab never act on a stale list.
    fn set_items(&mut self, items: Vec<Suggestion>, settings: &Settings) {
        let keep = match self.items.get(self.selected) {
            Some(old) if self.sticky && self.vis == MenuVisibility::Visible => {
                items.iter().position(|it| same_item(it, old))
            }
            _ => None,
        };
        self.items = items;
        match keep {
            Some(i) => {
                self.selected = i;
                self.scroll_into_view(max_rows(settings));
            }
            None => {
                self.selected = 0;
                self.sticky = false;
                self.scroll_top = 0;
            }
        }
    }

    /// Error reset: no items, HiddenUntilKeypress, history mode off (SPEC §7.1).
    pub(crate) fn error_reset(&mut self) {
        // The pop-out is UI state outside the item store: it survives resets.
        *self = Menu {
            popout: self.popout,
            ..Menu::new()
        };
    }

    /// Menu drawn? (`Visible` and items or arg hint, and not `autocomplete.disable`.)
    pub(crate) fn shown(&self, settings: &Settings) -> bool {
        !settings.bool(AUTOCOMPLETE_DISABLE)
            && self.vis == MenuVisibility::Visible
            && (!self.items.is_empty() || self.arg_hint().is_some())
    }

    /// Current argument with a name or description (argument-hint row).
    pub(crate) fn arg_hint(&self) -> Option<&ArgumentHint> {
        self.last
            .as_ref()?
            .current_arg()
            .filter(|a| !a.name().is_empty() || !a.description().is_empty())
    }

    /// Keys are intercepted (shown and items non-empty, SPEC §7.2.4).
    pub(crate) fn intercepting(&self, settings: &Settings) -> bool {
        self.shown(settings) && !self.items.is_empty()
    }

    pub(crate) fn has_items(&self) -> bool {
        !self.items.is_empty()
    }

    /// Run an action (SPEC §7.2.5). No-op without a selected item. The `onlyShowOnTab` preamble is `show_on_tab`.
    pub(crate) fn handle(&mut self, action: KeyAction, settings: &Settings) -> Vec<MenuEffect> {
        if self.items.is_empty() {
            return Vec::new();
        }
        let always_desc = settings.bool(AUTOCOMPLETE_ALWAYS_SHOW_DESCRIPTION);
        match action {
            KeyAction::NavigateUp => self.navigate(-1, true, settings),
            KeyAction::NavigateDown => self.navigate(1, false, settings),
            KeyAction::InsertSelected => self.insert_full(self.selected, false),
            KeyAction::InsertSelectedAndExecute => self.insert_full(self.selected, true),
            KeyAction::InsertCommonPrefix => {
                self.common_prefix().unwrap_or_else(|| vec![MenuEffect::Bell])
            }
            KeyAction::InsertCommonPrefixOrNavigateDown => self
                .common_prefix()
                .unwrap_or_else(|| self.navigate(1, false, settings)),
            KeyAction::InsertCommonPrefixOrInsertSelected => self
                .common_prefix()
                .unwrap_or_else(|| self.insert_full(self.selected, false)),
            KeyAction::Execute => vec![MenuEffect::Write(b"\r".to_vec())],
            KeyAction::HideAutocomplete => {
                self.vis = MenuVisibility::HiddenUntilShown;
                Vec::new()
            }
            KeyAction::ShowAutocomplete => {
                self.vis = MenuVisibility::Visible;
                Vec::new()
            }
            KeyAction::ToggleAutocomplete => {
                self.vis = if self.vis == MenuVisibility::Visible {
                    MenuVisibility::HiddenUntilShown
                } else {
                    MenuVisibility::Visible
                };
                Vec::new()
            }
            KeyAction::ToggleHistoryMode => {
                self.history_mode = !self.history_mode;
                vec![MenuEffect::Rerequest]
            }
            KeyAction::ToggleDescription | KeyAction::ShowDescription | KeyAction::HideDescription => {
                if !always_desc {
                    self.popout = match action {
                        KeyAction::ToggleDescription => !self.popout,
                        a => a == KeyAction::ShowDescription,
                    };
                }
                Vec::new()
            }
            KeyAction::ToggleFuzzySearch => vec![MenuEffect::ToggleFuzzy],
            KeyAction::SelectSuggestion(n) => {
                let i = self.scroll_top + usize::from(n.max(1)) - 1;
                if i < self.items.len() {
                    self.select(i, settings);
                }
                Vec::new()
            }
        }
    }

    /// `onlyShowOnTab` Tab while hidden: 1 item → insert it, else show (SPEC §7.2.5 preamble).
    pub(crate) fn show_on_tab(&mut self, settings: &Settings) -> Vec<MenuEffect> {
        let _ = settings;
        if self.items.len() == 1 {
            return self.insert_full(0, false);
        }
        self.vis = MenuVisibility::Visible;
        Vec::new()
    }

    pub(crate) fn history_mode(&self) -> bool {
        self.history_mode
    }

    /// Latest completion applied (tokens, current arg, buffer it was computed from). Its `items` are empty:
    /// the list lives in `items()`.
    pub(crate) fn last(&self) -> Option<&CompletionResult> {
        self.last.as_ref()
    }

    pub(crate) fn items(&self) -> &[Suggestion] {
        &self.items
    }

    pub(crate) fn selected(&self) -> usize {
        self.selected
    }

    pub(crate) fn scroll_top(&self) -> usize {
        self.scroll_top
    }

    #[cfg(test)]
    pub(crate) fn vis(&self) -> MenuVisibility {
        self.vis
    }

    /// Description pop-out open (or `alwaysShowDescription`).
    pub(crate) fn popout(&self, settings: &Settings) -> bool {
        self.popout || settings.bool(AUTOCOMPLETE_ALWAYS_SHOW_DESCRIPTION)
    }

    /// Some generator has been loading for > 200 ms (spinner).
    pub(crate) fn loading(&self, now: Instant) -> bool {
        self.loading_since
            .is_some_and(|t| now.duration_since(t) > Duration::from_millis(200))
    }

    fn navigate(&mut self, d: isize, hide_on_negative: bool, settings: &Settings) -> Vec<MenuEffect> {
        let len = self.items.len() as isize;
        let mut i = self.selected as isize + d;
        if settings.bool(AUTOCOMPLETE_SCROLL_WRAP_AROUND) {
            i = i.rem_euclid(len);
        } else if i == -1 && hide_on_negative {
            if settings.bool(AUTOCOMPLETE_NAVIGATE_TO_HISTORY) {
                self.history_mode = !self.history_mode;
                return vec![MenuEffect::Rerequest];
            }
            self.vis = MenuVisibility::HiddenUntilKeypress;
            return Vec::new();
        }
        self.select(i.clamp(0, len - 1) as usize, settings);
        Vec::new()
    }

    fn select(&mut self, i: usize, settings: &Settings) {
        self.sticky = i != self.selected;
        self.selected = i;
        self.scroll_into_view(max_rows(settings));
    }

    fn scroll_into_view(&mut self, rows: usize) {
        if self.selected < self.scroll_top {
            self.scroll_top = self.selected;
        } else if self.selected >= self.scroll_top + rows {
            self.scroll_top = self.selected + 1 - rows;
        }
    }

    /// Full insertion of item `idx` (SPEC §7.3): HiddenByInsertion + recency record (D21; skipped for `↪`).
    fn insert_full(&mut self, idx: usize, execute: bool) -> Vec<MenuEffect> {
        let item = &self.items[idx];
        let mut ins = item.insert().clone();
        if execute && item.kind() != SuggestionKind::AutoExecute {
            ins = ins.with_execute();
        }
        let name = item.first_name();
        let record = match &self.last {
            Some(c) if !name.contains('↪') => c
                .tokens()
                .get(c.command_index())
                .map(|cmd| (cmd.clone(), name.to_string())),
            _ => None,
        };
        self.last_inserted_gen = item.generator();
        self.vis = MenuVisibility::HiddenByInsertion;
        self.just_inserted = true;
        vec![MenuEffect::Insert {
            ins,
            full: true,
            record,
        }]
    }

    /// Inserts the common prefix of the listed items; `None` = failure (shake).
    fn common_prefix(&mut self) -> Option<Vec<MenuEffect>> {
        let sel = self.items[self.selected].clone();
        if self.items.len() == 1 {
            return Some(self.insert_full(self.selected, false));
        }
        let auto = sel.kind() == SuggestionKind::AutoExecute;
        let kind = if auto { sel.original_kind()? } else { sel.kind() };
        if matches!(kind, SuggestionKind::Special | SuggestionKind::AutoExecute) {
            return None;
        }
        let mut sel_first = sel.first_name().to_string();
        if auto && kind == SuggestionKind::Folder && !sel_first.ends_with('/') {
            sel_first.push('/');
        }
        let q = sel.query().to_lowercase();
        let mut names: Vec<String> = (self.items.iter())
            .filter(|a| matching_type(a, &sel_first, kind))
            .map(|a| a.first_name().to_lowercase())
            .filter(|n| n.starts_with(&q))
            .collect();
        if names.len() == 1 {
            return (!auto).then(|| self.insert_full(self.selected, false));
        }
        names.sort();
        let lcp = match (names.first(), names.last()) {
            (Some(a), Some(b)) => a.chars().zip(b.chars()).take_while(|(x, y)| x == y).count(),
            _ => 0,
        };
        let shared: String = sel.first_name().chars().take(lcp).collect();
        if shared.is_empty() || shared == sel.query() {
            return None;
        }
        let text: String = shared
            .chars()
            .map(|c| {
                if c.is_whitespace() {
                    "\\ ".into()
                } else {
                    c.to_string()
                }
            })
            .collect();
        // The engine strips an already-typed query from `insert.text` (delete == 0): put it back to compare.
        let full = if sel.insert().delete() == 0 {
            format!("{}{}", sel.query(), sel.insert().text())
        } else {
            sel.insert().text().to_string()
        };
        // The engine escaped the row for its shell: type the shared prefix only as the start of that insertion
        // (`$XDG_` of `$XDG_CONFIG_HOME`), else BEL (`zz;` of `'zz;a'`, pwsh `my\ dir` of `'my dir1.txt'`).
        // ponytail: a prefix that needs quoting is not quoted half-way.
        if !full.starts_with(&text) {
            return None;
        }
        if text == full || format!("{text} ") == full {
            return (!auto).then(|| self.insert_full(self.selected, false));
        }
        // Partial insertion. Raw on-screen token chars (D22): the item's own delete count, or the query when it was
        // stripped (then the typed text equals the query and is not retyped).
        let q_chars = sel.query().chars().count();
        let raw = if sel.insert().delete() > 0 {
            sel.insert().delete()
        } else {
            q_chars
        };
        let (delete, text) = match text.strip_prefix(sel.query()) {
            Some(rest) if raw == q_chars => (0, rest.to_string()),
            _ => (raw, text),
        };
        self.just_inserted = true;
        Some(vec![MenuEffect::Insert {
            ins: Insertion::new(InsertionParts {
                delete,
                text,
                cursor_back: 0,
                execute: false,
            }),
            full: false,
            record: None,
        }])
    }
}

#[cfg(test)]
pub(crate) mod tests {
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
}
