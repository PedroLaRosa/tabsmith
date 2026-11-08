/// How a row's query term is derived from the search term (SPEC §6.7.3 `query(s)`).
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum SuggestionQuery {
    /// The search term itself.
    Term,
    /// String `getQueryTerm`: the part after its last occurrence (D23: `+ g.len()`).
    Str(String),
    /// Precomputed: function `getQueryTerm` result, or `""` for args inside an option chain.
    Value(String),
    /// `filepaths`/`folders` templates: the part after the last path separator (`/`; pwsh also `\`).
    Path { backslash: bool },
}

impl SuggestionQuery {
    pub(crate) fn of(&self, search_term: &str) -> String {
        match self {
            SuggestionQuery::Term => search_term.to_string(),
            SuggestionQuery::Str(g) => {
                search_term[search_term.rfind(g.as_str()).map_or(0, |i| i + g.len())..].to_string()
            }
            SuggestionQuery::Value(v) => v.clone(),
            SuggestionQuery::Path { backslash } => {
                search_term[after_sep(search_term, *backslash)..].to_string()
            }
        }
    }
}

/// Byte index just past the last path separator in `s` (0 when none). `\` counts only in pwsh: POSIX shells
/// use it as the escape char.
pub(crate) fn after_sep(s: &str, backslash: bool) -> usize {
    s.rfind(|c| c == '/' || (backslash && c == '\\'))
        .map_or(0, |i| i + 1)
}
