#[derive(Default)]
pub(crate) struct FilterOptions {
    fuzzy: bool,
    /// `hideAutoExecuteSuggestion || onlyShowOnTab`.
    auto_exec_hidden: bool,
    exec_after_space: bool,
    run_dangerous: bool,
    /// `current_arg.suggestCurrentToken ?? alwaysSuggestCurrentToken`.
    suggest_current_token: bool,
}

impl FilterOptions {
    pub(crate) fn fuzzy(&self) -> bool {
        self.fuzzy
    }

    pub(crate) fn auto_exec_hidden(&self) -> bool {
        self.auto_exec_hidden
    }

    pub(crate) fn exec_after_space(&self) -> bool {
        self.exec_after_space
    }

    pub(crate) fn run_dangerous(&self) -> bool {
        self.run_dangerous
    }

    pub(crate) fn suggest_current_token(&self) -> bool {
        self.suggest_current_token
    }

    pub(crate) fn with_fuzzy(mut self, fuzzy: bool) -> Self {
        self.fuzzy = fuzzy;
        self
    }

    pub(crate) fn with_auto_exec_hidden(mut self, auto_exec_hidden: bool) -> Self {
        self.auto_exec_hidden = auto_exec_hidden;
        self
    }

    pub(crate) fn with_exec_after_space(mut self, exec_after_space: bool) -> Self {
        self.exec_after_space = exec_after_space;
        self
    }

    pub(crate) fn with_run_dangerous(mut self, run_dangerous: bool) -> Self {
        self.run_dangerous = run_dangerous;
        self
    }

    pub(crate) fn with_suggest_current_token(mut self, suggest_current_token: bool) -> Self {
        self.suggest_current_token = suggest_current_token;
        self
    }
}
