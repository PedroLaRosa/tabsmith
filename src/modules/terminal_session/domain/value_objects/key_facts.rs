/// What the key router needs to know of the session besides the key (SPEC §7.2.4).
#[derive(Clone, Debug, Default)]
pub(crate) struct KeyFacts {
    prompt: bool,
    menu_intercepts: bool,
    drawable: bool,
    menu_has_items: bool,
    autocomplete_disabled: bool,
    only_show_on_tab: bool,
    waiting_for_echo: bool,
    ghost_text: Option<String>,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct KeyFactsParts {
    pub(crate) prompt: bool,
    pub(crate) menu_intercepts: bool,
    pub(crate) drawable: bool,
    pub(crate) menu_has_items: bool,
    pub(crate) autocomplete_disabled: bool,
    pub(crate) only_show_on_tab: bool,
    pub(crate) waiting_for_echo: bool,
    pub(crate) ghost_text: Option<String>,
}

impl KeyFacts {
    pub(crate) fn new(p: KeyFactsParts) -> Self {
        KeyFacts {
            prompt: p.prompt,
            menu_intercepts: p.menu_intercepts,
            drawable: p.drawable,
            menu_has_items: p.menu_has_items,
            autocomplete_disabled: p.autocomplete_disabled,
            only_show_on_tab: p.only_show_on_tab,
            waiting_for_echo: p.waiting_for_echo,
            ghost_text: p.ghost_text,
        }
    }
    pub(crate) fn prompt(&self) -> bool {
        self.prompt
    }

    pub(crate) fn menu_intercepts(&self) -> bool {
        self.menu_intercepts
    }

    pub(crate) fn drawable(&self) -> bool {
        self.drawable
    }

    pub(crate) fn menu_has_items(&self) -> bool {
        self.menu_has_items
    }

    pub(crate) fn autocomplete_disabled(&self) -> bool {
        self.autocomplete_disabled
    }

    pub(crate) fn only_show_on_tab(&self) -> bool {
        self.only_show_on_tab
    }

    pub(crate) fn waiting_for_echo(&self) -> bool {
        self.waiting_for_echo
    }

    pub(crate) fn ghost_text(&self) -> Option<&str> {
        self.ghost_text.as_deref()
    }
}
