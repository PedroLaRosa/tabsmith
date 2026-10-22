//! Dropdown UI state: visibility machine, selection/scroll, actions, common prefix (SPEC §7.1–7.3).
//! Pure: no I/O. OWNER: ui agent. Driven by the wrapper (pty.rs); read by render.rs.
use crate::keys::Action;
use crate::settings::Settings;
use crate::types::{ArgInfo, Completion, GenKey, Insertion, Item, ItemType, Phase};
use std::time::{Duration, Instant};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Vis {
    Visible,
    HiddenUntilKeypress,
    HiddenUntilShown,
    HiddenByInsertion,
}

/// What the wrapper must do after an action.
#[derive(Clone, Debug, PartialEq)]
pub enum Effect {
    /// Write an insertion to the PTY (with buffer reconciliation + insertion lock, SPEC §7.3).
    /// `full` = a full insertion (visibility HiddenByInsertion already applied); `record` = (cmd, name) for recency.
    Insert { ins: Insertion, full: bool, record: Option<(String, String)> },
    /// Write raw bytes to the PTY (e.g. "\r" for `execute`).
    Write(Vec<u8>),
    /// Ring the bell (common-prefix failure "shake").
    Bell,
    /// Re-request completions for the current buffer (history mode toggled).
    Rerequest,
    /// Toggle fuzzy search in the engine, then re-request.
    ToggleFuzzy,
}

pub struct Menu {
    vis: Vis,
    items: Vec<Item>,
    selected: usize,
    /// Kiro `hasChangedIndex`: the user navigated, keep the selection across recomputes.
    sticky: bool,
    scroll_top: usize,
    /// Generator of the last fully inserted item (re-trigger reopens the menu).
    last_inserted_gen: Option<GenKey>,
    just_inserted: bool,
    history_mode: bool,
    popout: bool,
    /// Latest reply; its `items` are moved into `self.items`.
    last: Option<Completion>,
    loading_since: Option<Instant>,
    backspace_pending: bool,
}

/// Max visible list rows (`autocomplete.height`, D14).
pub fn max_rows(settings: &Settings) -> usize {
    settings.f64("autocomplete.height", 7.0).max(1.0) as usize
}

/// Kiro `isBufferDifferenceFromTyping`: one is a prefix of the other and they differ by < 2 chars.
fn typing(a: &str, b: &str) -> bool {
    (a.starts_with(b) || b.starts_with(a)) && a.chars().count().abs_diff(b.chars().count()) < 2
}

fn same_item(a: &Item, b: &Item) -> bool {
    a.names == b.names && a.kind == b.kind && a.insert.text == b.insert.text && a.description == b.description
}

/// Kiro `isMatchingType` (`../` only matches `../`; file and folder match each other).
fn matching_type(a: &Item, b_first: &str, b_kind: ItemType) -> bool {
    if a.first_name() == "../" {
        return b_first == "../";
    }
    let file_like = |k| matches!(k, ItemType::File | ItemType::Folder);
    (file_like(a.kind) && file_like(b_kind)) || a.kind == b_kind
}

impl Menu {
    pub fn new() -> Menu {
        Menu {
            vis: Vis::HiddenUntilKeypress,
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
            backspace_pending: false,
        }
    }

    /// Phase-Parse / phase-Generators reply for the latest request (SPEC §7.1). An error reply resets.
    pub fn on_completion(&mut self, mut c: Completion, settings: &Settings) {
        if c.error {
            return self.error_reset();
        }
        let items = std::mem::take(&mut c.items);
        self.loading_since = if c.loading { self.loading_since.or_else(|| Some(Instant::now())) } else { None };
        if c.phase == Phase::Parse {
            // After a reset Kiro compares against its initial state: new arg, empty command text.
            let (has_new_arg, backspaced, large) = match &self.last {
                None => (true, false, !typing(&c.command_text, "")),
                Some(l) => {
                    let n = c.tokens.len();
                    (
                        l.arg_key != c.arg_key,
                        n > 0 && n < l.tokens.len() && l.tokens[n - 1] == c.tokens[n - 1],
                        !typing(&c.command_text, &l.command_text),
                    )
                }
            };
            let orig = self.vis;
            self.vis = match orig {
                Vis::HiddenUntilKeypress => Vis::Visible,
                Vis::HiddenByInsertion => {
                    let retriggered = self.last_inserted_gen.is_some_and(|g| c.triggered_generators.contains(&g));
                    if has_new_arg || retriggered {
                        Vis::Visible
                    } else {
                        Vis::HiddenUntilKeypress
                    }
                }
                v => v,
            };
            if settings.bool("autocomplete.onlyShowOnTab", false) {
                self.vis = orig;
                if has_new_arg {
                    self.vis = Vis::HiddenUntilShown;
                }
            }
            if backspaced || (large && !self.just_inserted) {
                self.vis = Vis::HiddenUntilKeypress;
            }
            self.just_inserted = false;
        }
        self.last = Some(c);
        self.set_items(items, settings);
    }

    /// Kiro does not recompute while HiddenUntilKeypress (stale items); keeps a sticky selection if still listed.
    fn set_items(&mut self, items: Vec<Item>, settings: &Settings) {
        if self.vis == Vis::HiddenUntilKeypress {
            return;
        }
        let keep = match self.items.get(self.selected) {
            Some(old) if self.sticky && self.vis == Vis::Visible => items.iter().position(|it| same_item(it, old)),
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
    pub fn error_reset(&mut self) {
        // The pop-out is UI state outside Kiro's store: it survives resets.
        *self = Menu { popout: self.popout, ..Menu::new() };
    }

    /// Menu drawn? (`Visible` and items or arg hint, and not `autocomplete.disable`.)
    pub fn shown(&self, settings: &Settings) -> bool {
        !settings.bool("autocomplete.disable", false)
            && self.vis == Vis::Visible
            && (!self.items.is_empty() || self.arg_hint().is_some())
    }

    /// Current argument with a name or description (argument-hint row).
    pub fn arg_hint(&self) -> Option<&ArgInfo> {
        self.last.as_ref()?.current_arg.as_ref().filter(|a| !a.name.is_empty() || !a.description.is_empty())
    }

    /// Keys are intercepted (shown and items non-empty, SPEC §7.2.4).
    pub fn intercepting(&self, settings: &Settings) -> bool {
        self.shown(settings) && !self.items.is_empty()
    }

    pub fn has_items(&self) -> bool {
        !self.items.is_empty()
    }

    /// Run an action (SPEC §7.2.5). No-op without a selected item. The `onlyShowOnTab` preamble is `show_on_tab`.
    pub fn handle(&mut self, action: Action, settings: &Settings) -> Vec<Effect> {
        if self.items.is_empty() {
            return Vec::new();
        }
        let always_desc = settings.bool("autocomplete.alwaysShowDescription", false);
        match action {
            Action::NavigateUp => self.navigate(-1, true, settings),
            Action::NavigateDown => self.navigate(1, false, settings),
            Action::InsertSelected => self.insert_full(self.selected, false),
            Action::InsertSelectedAndExecute => self.insert_full(self.selected, true),
            Action::InsertCommonPrefix => self.common_prefix().unwrap_or_else(|| vec![Effect::Bell]),
            Action::InsertCommonPrefixOrNavigateDown => {
                self.common_prefix().unwrap_or_else(|| self.navigate(1, false, settings))
            }
            Action::InsertCommonPrefixOrInsertSelected => {
                self.common_prefix().unwrap_or_else(|| self.insert_full(self.selected, false))
            }
            Action::Execute => vec![Effect::Write(b"\r".to_vec())],
            Action::HideAutocomplete => {
                self.vis = Vis::HiddenUntilShown;
                Vec::new()
            }
            Action::ShowAutocomplete => {
                self.vis = Vis::Visible;
                Vec::new()
            }
            Action::ToggleAutocomplete => {
                self.vis = if self.vis == Vis::Visible { Vis::HiddenUntilShown } else { Vis::Visible };
                Vec::new()
            }
            Action::ToggleHistoryMode => {
                self.history_mode = !self.history_mode;
                vec![Effect::Rerequest]
            }
            Action::ToggleDescription | Action::ShowDescription | Action::HideDescription => {
                if !always_desc {
                    self.popout = match action {
                        Action::ToggleDescription => !self.popout,
                        a => a == Action::ShowDescription,
                    };
                }
                Vec::new()
            }
            Action::ToggleFuzzySearch => vec![Effect::ToggleFuzzy],
            Action::SelectSuggestion(n) => {
                let i = self.scroll_top + usize::from(n.max(1)) - 1;
                if i < self.items.len() {
                    self.select(i, settings);
                }
                Vec::new()
            }
        }
    }

    /// `onlyShowOnTab` Tab while hidden: 1 item → insert it, else show (SPEC §7.2.5 preamble).
    pub fn show_on_tab(&mut self, settings: &Settings) -> Vec<Effect> {
        let _ = settings;
        if self.items.len() == 1 {
            return self.insert_full(0, false);
        }
        self.vis = Vis::Visible;
        Vec::new()
    }

    /// Ctrl-C / Ctrl-D / new prompt etc. handled by the wrapper via `error_reset`; this marks a backspace whose
    /// echo is pending (keeps a visible menu up until the echo arrives).
    pub fn note_backspace(&mut self) {
        self.backspace_pending = true;
    }

    /// Consume the backspace mark (call once per PTY batch). While it was set, the wrapper skips the mid-word
    /// error reset for that batch: a split `\b \b` echo briefly puts the cursor over the deleted char (ish).
    pub fn take_backspace_pending(&mut self) -> bool {
        std::mem::take(&mut self.backspace_pending)
    }

    pub fn history_mode(&self) -> bool {
        self.history_mode
    }

    /// Latest completion applied (tokens, current arg, buffer it was computed from). Its `items` are empty:
    /// the list lives in `items()`.
    pub fn last(&self) -> Option<&Completion> {
        self.last.as_ref()
    }

    pub fn items(&self) -> &[Item] {
        &self.items
    }

    pub fn selected(&self) -> usize {
        self.selected
    }

    pub fn scroll_top(&self) -> usize {
        self.scroll_top
    }

    pub fn vis(&self) -> Vis {
        self.vis
    }

    /// Description pop-out open (or `alwaysShowDescription`).
    pub fn popout(&self, settings: &Settings) -> bool {
        self.popout || settings.bool("autocomplete.alwaysShowDescription", false)
    }

    /// Some generator has been loading for > 200 ms (spinner).
    pub fn loading(&self) -> bool {
        self.loading_since.is_some_and(|t| t.elapsed() > Duration::from_millis(200))
    }

    fn navigate(&mut self, d: isize, hide_on_negative: bool, settings: &Settings) -> Vec<Effect> {
        let len = self.items.len() as isize;
        let mut i = self.selected as isize + d;
        if settings.bool("autocomplete.scrollWrapAround", false) {
            i = i.rem_euclid(len);
        } else if i == -1 && hide_on_negative {
            if settings.bool("autocomplete.navigateToHistory", false) {
                self.history_mode = !self.history_mode;
                return vec![Effect::Rerequest];
            }
            self.vis = Vis::HiddenUntilKeypress;
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
    fn insert_full(&mut self, idx: usize, execute: bool) -> Vec<Effect> {
        let item = &self.items[idx];
        let mut ins = item.insert.clone();
        if execute && item.kind != ItemType::AutoExecute {
            ins.execute = true;
        }
        let name = item.first_name();
        let record = match &self.last {
            Some(c) if !name.contains('↪') => c.tokens.get(c.command_index).map(|cmd| (cmd.clone(), name.to_string())),
            _ => None,
        };
        self.last_inserted_gen = item.gen;
        self.vis = Vis::HiddenByInsertion;
        self.just_inserted = true;
        vec![Effect::Insert { ins, full: true, record }]
    }

    /// Kiro `insertCommonPrefix` (`insertion.ts:226-304`); `None` = failure (shake).
    fn common_prefix(&mut self) -> Option<Vec<Effect>> {
        let sel = self.items[self.selected].clone();
        if self.items.len() == 1 {
            return Some(self.insert_full(self.selected, false));
        }
        let auto = sel.kind == ItemType::AutoExecute;
        let kind = if auto { sel.original_kind? } else { sel.kind };
        if matches!(kind, ItemType::Special | ItemType::AutoExecute) {
            return None;
        }
        let mut sel_first = sel.first_name().to_string();
        if auto && kind == ItemType::Folder && !sel_first.ends_with('/') {
            sel_first.push('/');
        }
        let q = sel.query.to_lowercase();
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
        if shared.is_empty() || shared == sel.query {
            return None;
        }
        let text: String = shared.chars().map(|c| if c.is_whitespace() { "\\ ".into() } else { c.to_string() }).collect();
        // The engine strips an already-typed query from `insert.text` (delete == 0): put it back to compare.
        let full = if sel.insert.delete == 0 { format!("{}{}", sel.query, sel.insert.text) } else { sel.insert.text.clone() };
        if text == full || format!("{text} ") == full {
            return (!auto).then(|| self.insert_full(self.selected, false));
        }
        // Partial insertion. Raw on-screen token chars (D22): the item's own delete count, or the query when it was
        // stripped (then the typed text equals the query and is not retyped).
        let q_chars = sel.query.chars().count();
        let raw = if sel.insert.delete > 0 { sel.insert.delete } else { q_chars };
        let (delete, text) = match text.strip_prefix(sel.query.as_str()) {
            Some(rest) if raw == q_chars => (0, rest.to_string()),
            _ => (raw, text),
        };
        self.just_inserted = true;
        Some(vec![Effect::Insert {
            ins: Insertion { delete, text, cursor_back: 0, execute: false },
            full: false,
            record: None,
        }])
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::types::ArgInfo;

    pub fn item(name: &str, kind: ItemType, query: &str) -> Item {
        let typed = name.starts_with(query) && !query.is_empty();
        Item {
            names: vec![name.to_string()],
            display_name: None,
            description: Some(format!("about {name}")),
            kind,
            original_kind: None,
            priority: 50.0,
            is_dangerous: false,
            args_hint: String::new(),
            highlight: (0..query.chars().count()).collect(),
            query: query.to_string(),
            insert: Insertion {
                delete: if typed { 0 } else { query.chars().count() },
                text: if typed { name[query.len()..].to_string() } else { name.to_string() },
                cursor_back: 0,
                execute: false,
            },
            gen: None,
            prefix_match: true,
            template_type: None,
        }
    }

    /// A Parse reply for `buffer` (tokens split on spaces, a trailing space adds an empty token).
    pub fn comp(buffer: &str, arg_key: u64, items: Vec<Item>) -> Completion {
        let tokens: Vec<String> = buffer.split(' ').map(String::from).collect();
        Completion {
            req: 1,
            phase: Phase::Parse,
            error: false,
            buffer: buffer.to_string(),
            cursor: buffer.len(),
            search_term: tokens.last().cloned().unwrap_or_default(),
            tokens,
            command_text: buffer.to_string(),
            command_index: 0,
            arg_key,
            current_arg: Some(ArgInfo { name: "path".into(), description: "A path".into() }),
            triggered_generators: Vec::new(),
            items,
            loading: false,
            ghost: None,
            fuzzy: false,
        }
    }

    pub fn settings(json: &str) -> Settings {
        match serde_json::from_str(json).unwrap() {
            serde_json::Value::Object(m) => Settings::from_map(m),
            _ => unreachable!(),
        }
    }

    fn subs(q: &str) -> Vec<Item> {
        ["checkout", "cherry-pick", "clone", "commit"]
            .iter()
            .filter(|n| n.starts_with(q))
            .map(|n| item(n, ItemType::Subcommand, q))
            .collect()
    }

    /// Menu after typing `git c`, visible.
    fn typed(s: &Settings) -> Menu {
        let mut m = Menu::new();
        m.on_completion(comp("g", 1, vec![]), s);
        m.on_completion(comp("gi", 1, vec![]), s);
        m.on_completion(comp("git", 1, vec![]), s);
        m.on_completion(comp("git ", 2, subs("")), s);
        m.on_completion(comp("git c", 2, subs("c")), s);
        m
    }

    #[test]
    fn typing_and_space() {
        let s = Settings::default();
        let mut m = Menu::new();
        assert_eq!(m.vis(), Vis::HiddenUntilKeypress);
        m.on_completion(comp("g", 1, vec![]), &s);
        assert_eq!(m.vis(), Vis::Visible);
        // Visible with an arg hint but no items: drawn, not intercepting.
        assert!(m.shown(&s) && !m.intercepting(&s));
        m.on_completion(comp("gi", 1, vec![]), &s);
        m.on_completion(comp("git", 1, vec![]), &s);
        m.on_completion(comp("git ", 2, subs("")), &s);
        assert_eq!(m.vis(), Vis::Visible);
        assert_eq!(m.items().len(), 4);
        assert!(m.intercepting(&s));
        assert!(!m.intercepting(&settings(r#"{"autocomplete.disable": true}"#)));
    }

    #[test]
    fn full_insertion_reopens_only_for_new_arg_or_retrigger() {
        let s = Settings::default();
        // Insertion with a new argument next → visible.
        let mut m = typed(&s);
        let fx = m.handle(Action::InsertSelected, &s);
        assert_eq!(
            fx,
            vec![Effect::Insert {
                ins: Insertion { delete: 0, text: "heckout".into(), cursor_back: 0, execute: false },
                full: true,
                record: Some(("git".into(), "checkout".into())),
            }]
        );
        assert_eq!(m.vis(), Vis::HiddenByInsertion);
        m.on_completion(comp("git checkout ", 3, vec![item("main", ItemType::Arg, "")]), &s);
        assert_eq!(m.vis(), Vis::Visible, "multi-char echo is not a large change after an insertion");

        // Flag without a new argument → hidden until the next key.
        let mut m = typed(&s);
        m.handle(Action::InsertSelected, &s);
        m.on_completion(comp("git checkout", 2, vec![]), &s);
        assert_eq!(m.vis(), Vis::HiddenUntilKeypress);
        m.on_completion(comp("git checkout2", 2, vec![]), &s);
        assert_eq!(m.vis(), Vis::Visible);

        // Folder `src/` re-triggers its generator → visible.
        let mut m = typed(&s);
        m.on_completion(comp("git c ", 5, vec![Item { gen: Some(7), ..item("src/", ItemType::Folder, "") }]), &s);
        m.handle(Action::InsertSelected, &s);
        let mut c = comp("git c src/", 5, vec![item("main.rs", ItemType::File, "")]);
        c.triggered_generators = vec![7];
        m.on_completion(c, &s);
        assert_eq!(m.vis(), Vis::Visible);
    }

    #[test]
    fn esc_hides_for_the_line() {
        let s = Settings::default();
        let mut m = typed(&s);
        assert!(m.handle(Action::HideAutocomplete, &s).is_empty());
        assert_eq!(m.vis(), Vis::HiddenUntilShown);
        m.on_completion(comp("git co", 2, subs("co")), &s);
        m.on_completion(comp("git co ", 3, vec![item("x", ItemType::Arg, "")]), &s);
        assert_eq!(m.vis(), Vis::HiddenUntilShown);
        assert!(!m.shown(&s));
        // Items are still recomputed (globally interceptable keys like showAutocomplete work).
        assert_eq!(m.items()[0].names[0], "x");
        m.handle(Action::ShowAutocomplete, &s);
        assert_eq!(m.vis(), Vis::Visible);
        m.handle(Action::ToggleAutocomplete, &s);
        assert_eq!(m.vis(), Vis::HiddenUntilShown);
        m.handle(Action::ToggleAutocomplete, &s);
        assert_eq!(m.vis(), Vis::Visible);
    }

    #[test]
    fn backspace_into_previous_token_and_large_change() {
        let s = Settings::default();
        let mut m = typed(&s);
        m.handle(Action::HideAutocomplete, &s);
        m.on_completion(comp("git ", 2, subs("")), &s);
        m.on_completion(comp("git", 1, vec![]), &s);
        assert_eq!(m.vis(), Vis::HiddenUntilKeypress, "backspace into the previous token overrides Esc");
        m.on_completion(comp("gi", 1, vec![]), &s);
        assert_eq!(m.vis(), Vis::Visible);

        // A paste is a large change.
        let mut m = typed(&s);
        m.on_completion(comp("git commit -m 'x' --amend", 9, vec![]), &s);
        assert_eq!(m.vis(), Vis::HiddenUntilKeypress);
        // Stale items are kept while HiddenUntilKeypress.
        assert_eq!(m.items().len(), 4);
    }

    #[test]
    fn error_reset_and_loading() {
        let s = Settings::default();
        let mut m = typed(&s);
        m.handle(Action::ToggleHistoryMode, &s);
        m.handle(Action::ToggleDescription, &s);
        m.on_completion(Completion::error(9, "git c".into(), 5), &s);
        assert_eq!(m.vis(), Vis::HiddenUntilKeypress);
        assert!(!m.has_items() && !m.history_mode() && m.last().is_none());
        assert!(m.popout(&s));
        let mut c = comp("g", 1, vec![]);
        c.loading = true;
        m.on_completion(c, &s);
        assert!(!m.loading(), "spinner only after 200 ms");
        m.loading_since = Some(Instant::now() - Duration::from_millis(300));
        assert!(m.loading());
        m.on_completion(comp("gi", 1, vec![]), &s);
        assert!(!m.loading());
    }

    #[test]
    fn only_show_on_tab() {
        let s = settings(r#"{"autocomplete.onlyShowOnTab": true}"#);
        let mut m = Menu::new();
        m.on_completion(comp("g", 1, vec![]), &s);
        assert_eq!(m.vis(), Vis::HiddenUntilShown, "never auto-opens");
        m.on_completion(comp("gi", 1, vec![]), &s);
        m.on_completion(comp("git", 1, vec![]), &s);
        m.on_completion(comp("git ", 2, subs("")), &s);
        m.on_completion(comp("git c", 2, subs("c")), &s);
        assert_eq!(m.vis(), Vis::HiddenUntilShown);
        assert_eq!(m.items().len(), 4);
        assert!(m.show_on_tab(&s).is_empty());
        assert_eq!(m.vis(), Vis::Visible);
        m.on_completion(comp("git co", 2, subs("co")), &s);
        assert_eq!(m.vis(), Vis::Visible, "stays open within the argument");
        m.on_completion(comp("git co ", 3, vec![item("main", ItemType::Arg, "")]), &s);
        assert_eq!(m.vis(), Vis::HiddenUntilShown, "every new argument hides again");
        let fx = m.show_on_tab(&s);
        assert!(matches!(&fx[..], [Effect::Insert { full: true, .. }]), "single item: Tab inserts it");
    }

    #[test]
    fn navigate_wrap_hide_history() {
        let s = Settings::default();
        let mut m = typed(&s);
        m.handle(Action::NavigateDown, &s);
        m.handle(Action::NavigateDown, &s);
        m.handle(Action::NavigateDown, &s);
        m.handle(Action::NavigateDown, &s);
        assert_eq!(m.selected(), 3, "clamped at the bottom");
        for _ in 0..3 {
            m.handle(Action::NavigateUp, &s);
        }
        assert_eq!((m.selected(), m.vis()), (0, Vis::Visible));
        m.handle(Action::NavigateUp, &s);
        assert_eq!(m.vis(), Vis::HiddenUntilKeypress, "Up at index 0 hides");

        let s = settings(r#"{"autocomplete.navigateToHistory": true}"#);
        let mut m = typed(&s);
        assert_eq!(m.handle(Action::NavigateUp, &s), vec![Effect::Rerequest]);
        assert!(m.history_mode() && m.vis() == Vis::Visible);

        let s = settings(r#"{"autocomplete.scrollWrapAround": true, "autocomplete.height": 2}"#);
        let mut m = typed(&s);
        m.handle(Action::NavigateUp, &s);
        assert_eq!((m.selected(), m.scroll_top()), (3, 2));
        m.handle(Action::NavigateDown, &s);
        assert_eq!((m.selected(), m.scroll_top()), (0, 0));
        m.handle(Action::SelectSuggestion(2), &s);
        assert_eq!(m.selected(), 1);
    }

    #[test]
    fn sticky_selection_across_recompute() {
        let s = Settings::default();
        let mut m = typed(&s);
        m.handle(Action::NavigateDown, &s); // cherry-pick
        // Generator phase reply reorders the same items.
        let mut c = comp("git c", 2, subs("c").into_iter().rev().collect());
        c.phase = Phase::Generators;
        m.on_completion(c, &s);
        assert_eq!(m.items()[m.selected()].names[0], "cherry-pick");
        // Item no longer listed → back to the top, not sticky.
        m.on_completion(comp("git cl", 2, subs("cl")), &s);
        assert_eq!(m.selected(), 0);
        m.on_completion(comp("git c", 2, subs("c")), &s);
        assert_eq!(m.selected(), 0, "not sticky after the reset");
    }

    fn prefix_fx(m: &mut Menu) -> Vec<Effect> {
        m.handle(Action::InsertCommonPrefix, &Settings::default())
    }

    #[test]
    fn common_prefix() {
        let s = Settings::default();
        // checkout + cherry-pick → "che" → types "e" (query "ch" already typed), menu stays visible.
        let mut m = typed(&s);
        m.on_completion(comp("git ch", 2, subs("ch")), &s);
        let fx = prefix_fx(&mut m);
        assert_eq!(
            fx,
            vec![Effect::Insert {
                ins: Insertion { delete: 0, text: "e".into(), cursor_back: 0, execute: false },
                full: false,
                record: None
            }]
        );
        assert_eq!(m.vis(), Vis::Visible);
        assert!(m.just_inserted);

        // Typed in another case → delete the query and retype in the selected item's casing.
        let fx = prefix_fx(&mut with(["checkout", "cherry-pick"].iter().map(|n| item(n, ItemType::Subcommand, "CH")).collect()));
        assert!(matches!(&fx[..], [Effect::Insert { ins: Insertion { delete: 2, text, .. }, full: false, .. }] if text == "che"));

        // No remaining prefix → bell; a single item → full insertion.
        let mut m = typed(&s);
        assert_eq!(prefix_fx(&mut m), vec![Effect::Bell]);
        m.on_completion(comp("git cl", 2, subs("cl")), &s);
        assert!(matches!(&prefix_fx(&mut m)[..], [Effect::Insert { full: true, .. }]));

        // Mixed types: only items of the selected item's type count; file and folder match each other.
        let mut m = with(vec![
            item("src/", ItemType::Folder, "s"),
            item("setup.py", ItemType::File, "s"),
            item("--src-dir", ItemType::Option, "s"),
        ]);
        assert_eq!(prefix_fx(&mut m), vec![Effect::Bell], "lcp of src/ and setup.py is the query");
        let mut m = with(vec![
            item("src/", ItemType::Folder, "s"),
            item("srv/", ItemType::Folder, "s"),
            item("stuff", ItemType::Subcommand, "s"),
        ]);
        let fx = prefix_fx(&mut m);
        assert!(matches!(&fx[..], [Effect::Insert { ins: Insertion { delete: 0, text, .. }, full: false, .. }] if text == "r"));

        // `../` only matches `../`: with `..foo/` selected, `../` is not a candidate → full insertion.
        let mut m = with(vec![item("..foo/", ItemType::Folder, ""), item("../", ItemType::Folder, "")]);
        assert!(matches!(&prefix_fx(&mut m)[..], [Effect::Insert { full: true, .. }]));

        // Shared prefix equal to the full insertion → full insertion (foo vs foobar, foo selected).
        let mut m = with(vec![item("foo", ItemType::Arg, "f"), item("foobar", ItemType::Arg, "f")]);
        assert!(matches!(&prefix_fx(&mut m)[..], [Effect::Insert { full: true, .. }]));

        // Escaped spaces: the raw token `my\ d` (5 chars) is deleted and retyped escaped (D22).
        let mk = |n: &str| Item {
            insert: Insertion { delete: 5, text: n.replace(' ', "\\ "), cursor_back: 0, execute: false },
            ..item(n, ItemType::Folder, "my d")
        };
        let fx = prefix_fx(&mut with(vec![mk("my dir/"), mk("my dirt/")]));
        assert!(matches!(&fx[..], [Effect::Insert { ins: Insertion { delete: 5, text, .. }, .. }] if text == "my\\ dir"));

        // Auto-execute / special → failure.
        let mut m = with(vec![
            Item { original_kind: Some(ItemType::Special), ..item("↪", ItemType::AutoExecute, "") },
            item("x", ItemType::Arg, ""),
        ]);
        assert_eq!(prefix_fx(&mut m), vec![Effect::Bell]);
        // …and `OrNavigateDown` falls back to navigation.
        assert!(m.handle(Action::InsertCommonPrefixOrNavigateDown, &s).is_empty());
        assert_eq!(m.selected(), 1);
    }

    /// Visible menu over `items` (one-char buffer: not a large change).
    fn with(items: Vec<Item>) -> Menu {
        let mut m = Menu::new();
        m.on_completion(comp("x", 1, items), &Settings::default());
        assert_eq!(m.vis(), Vis::Visible);
        m
    }

    #[test]
    fn execute_and_record() {
        let s = Settings::default();
        let mut m = typed(&s);
        let fx = m.handle(Action::InsertSelectedAndExecute, &s);
        assert!(matches!(&fx[..], [Effect::Insert { ins: Insertion { execute: true, .. }, .. }]));
        let mut m = typed(&s);
        m.on_completion(comp("git c ", 3, vec![item("↪", ItemType::AutoExecute, "")]), &s);
        assert!(matches!(&m.handle(Action::InsertSelected, &s)[..], [Effect::Insert { record: None, .. }]));
        assert_eq!(m.handle(Action::Execute, &s), vec![Effect::Write(b"\r".to_vec())]);
        let mut m = typed(&s);
        m.handle(Action::ShowDescription, &s);
        assert!(m.popout(&s));
        m.handle(Action::HideDescription, &s);
        assert!(!m.popout(&s));
        let always = settings(r#"{"autocomplete.alwaysShowDescription": true}"#);
        m.handle(Action::ToggleDescription, &always);
        assert!(m.popout(&always) && !m.popout(&s), "toggle is a no-op with alwaysShowDescription");
    }
}
