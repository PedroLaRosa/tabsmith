use crate::dropdown::domain::services::common_prefix;
use crate::dropdown::domain::value_objects::common_prefix_outcome::CommonPrefixOutcome;
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
        match common_prefix::decide(&self.items, self.selected) {
            CommonPrefixOutcome::Failed => None,
            CommonPrefixOutcome::InsertSelected => Some(self.insert_full(self.selected, false)),
            CommonPrefixOutcome::InsertPrefix(ins) => {
                self.just_inserted = true;
                Some(vec![MenuEffect::Insert {
                    ins,
                    full: false,
                    record: None,
                }])
            }
        }
    }
}

#[cfg(test)]
mod tests;
