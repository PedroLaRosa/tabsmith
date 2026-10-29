use crate::completion::domain::value_objects::generator_key::GeneratorKey;
use crate::completion::domain::value_objects::insertion::Insertion;
use crate::specs::domain::value_objects::suggestion_kind::SuggestionKind;

/// One dropdown row, already filtered, ranked and with its insertion precomputed by the engine.
#[derive(Clone, Debug)]
pub(crate) struct Suggestion {
    pub(crate) names: Vec<String>,
    pub(crate) display_name: Option<String>,
    pub(crate) description: Option<String>,
    pub(crate) kind: SuggestionKind,
    /// For auto-execute rows: the type of the row it was derived from.
    pub(crate) original_kind: Option<SuggestionKind>,
    pub(crate) priority: f64,
    pub(crate) is_dangerous: bool,
    /// e.g. `<file> [mode]...` (SPEC §7.4.2), empty if none.
    pub(crate) args_hint: String,
    /// Char indexes into `title()` to highlight as matching (SPEC §7.4.2).
    pub(crate) highlight: Vec<usize>,
    /// Query term this item was matched against (SPEC §6.7.3 `query(s)`).
    pub(crate) query: String,
    pub(crate) insert: Insertion,
    /// Generator that produced it, if any.
    pub(crate) generator: Option<GeneratorKey>,
    /// Matched by prefix (not only fuzzy) — used by the spec ghost source (SPEC §6.11).
    pub(crate) prefix_match: bool,
    /// `context.templateType` of the generator result (`filepaths`/`folders`/`help`), if any.
    pub(crate) template_type: Option<String>,
    /// Spec `icon`, if any (SPEC §7.4.2).
    pub(crate) icon: Option<String>,
}

impl Suggestion {
    /// Display title: `display_name` or names joined by ", ".
    pub(crate) fn title(&self) -> String {
        self.display_name
            .clone()
            .unwrap_or_else(|| self.names.join(", "))
    }

    pub(crate) fn first_name(&self) -> &str {
        self.names.first().map(String::as_str).unwrap_or("")
    }
}
