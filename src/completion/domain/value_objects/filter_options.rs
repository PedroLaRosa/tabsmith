pub(crate) struct FilterOptions {
    pub(crate) fuzzy: bool,
    /// `hideAutoExecuteSuggestion || onlyShowOnTab`.
    pub(crate) auto_exec_hidden: bool,
    pub(crate) exec_after_space: bool,
    pub(crate) run_dangerous: bool,
    /// `current_arg.suggestCurrentToken ?? alwaysSuggestCurrentToken`.
    pub(crate) suggest_current_token: bool,
}
