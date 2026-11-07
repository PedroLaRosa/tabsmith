use crate::modules::completion::domain::value_objects::generator_key::GeneratorKey;
use crate::modules::completion::domain::value_objects::insertion::Insertion;
use crate::modules::specs::domain::value_objects::suggestion_kind::SuggestionKind;

/// One dropdown row, already filtered, ranked and with its insertion precomputed by the engine.
#[derive(Clone, Debug)]
pub(crate) struct Suggestion {
    names: Vec<String>,
    display_name: Option<String>,
    description: Option<String>,
    kind: SuggestionKind,
    /// For auto-execute rows: the type of the row it was derived from.
    original_kind: Option<SuggestionKind>,
    /// e.g. `<file> [mode]...` (SPEC §7.4.2), empty if none.
    args_hint: String,
    /// Char indexes into `title()` to highlight as matching (SPEC §7.4.2).
    highlight: Vec<usize>,
    /// Query term this item was matched against (SPEC §6.7.3 `query(s)`).
    query: String,
    insert: Insertion,
    /// Generator that produced it, if any.
    generator: Option<GeneratorKey>,
    /// Matched by prefix (not only fuzzy) — used by the spec ghost source (SPEC §6.11).
    prefix_match: bool,
    /// Spec `icon`, if any (SPEC §7.4.2).
    icon: Option<String>,
}

/// What a `Suggestion` is built from: named fields, so two counts or flags cannot be swapped.
pub(crate) struct SuggestionParts {
    pub(crate) names: Vec<String>,
    pub(crate) display_name: Option<String>,
    pub(crate) description: Option<String>,
    pub(crate) kind: SuggestionKind,
    pub(crate) original_kind: Option<SuggestionKind>,
    pub(crate) args_hint: String,
    pub(crate) highlight: Vec<usize>,
    pub(crate) query: String,
    pub(crate) insert: Insertion,
    pub(crate) generator: Option<GeneratorKey>,
    pub(crate) prefix_match: bool,
    pub(crate) icon: Option<String>,
}

impl Suggestion {
    pub(crate) fn new(parts: SuggestionParts) -> Self {
        let SuggestionParts {
            names,
            display_name,
            description,
            kind,
            original_kind,
            args_hint,
            highlight,
            query,
            insert,
            generator,
            prefix_match,
            icon,
        } = parts;
        Suggestion {
            names,
            display_name,
            description,
            kind,
            original_kind,
            args_hint,
            highlight,
            query,
            insert,
            generator,
            prefix_match,
            icon,
        }
    }

    pub(crate) fn names(&self) -> &[String] {
        &self.names
    }

    pub(crate) fn description(&self) -> Option<&str> {
        self.description.as_deref()
    }

    pub(crate) fn kind(&self) -> SuggestionKind {
        self.kind
    }

    pub(crate) fn original_kind(&self) -> Option<SuggestionKind> {
        self.original_kind
    }

    pub(crate) fn args_hint(&self) -> &str {
        &self.args_hint
    }

    pub(crate) fn highlight(&self) -> &[usize] {
        &self.highlight
    }

    pub(crate) fn query(&self) -> &str {
        &self.query
    }

    pub(crate) fn insert(&self) -> &Insertion {
        &self.insert
    }

    pub(crate) fn generator(&self) -> Option<GeneratorKey> {
        self.generator
    }

    pub(crate) fn prefix_match(&self) -> bool {
        self.prefix_match
    }

    pub(crate) fn icon(&self) -> Option<&str> {
        self.icon.as_deref()
    }

    /// Display title: `display_name` or names joined by ", ".
    pub(crate) fn title(&self) -> String {
        self.display_name
            .clone()
            .unwrap_or_else(|| self.names.join(", "))
    }

    pub(crate) fn first_name(&self) -> &str {
        self.names.first().map(String::as_str).unwrap_or("")
    }

    #[cfg(test)]
    pub(crate) fn with_generator(mut self, generator: Option<GeneratorKey>) -> Self {
        self.generator = generator;
        self
    }

    #[cfg(test)]
    pub(crate) fn with_icon(mut self, icon: Option<String>) -> Self {
        self.icon = icon;
        self
    }

    #[cfg(test)]
    pub(crate) fn with_args_hint(mut self, args_hint: String) -> Self {
        self.args_hint = args_hint;
        self
    }

    #[cfg(test)]
    pub(crate) fn with_names(mut self, names: Vec<String>) -> Self {
        self.names = names;
        self
    }

    #[cfg(test)]
    pub(crate) fn with_description(mut self, description: Option<String>) -> Self {
        self.description = description;
        self
    }

    #[cfg(test)]
    pub(crate) fn with_insertion(mut self, insert: Insertion) -> Self {
        self.insert = insert;
        self
    }

    #[cfg(test)]
    pub(crate) fn with_original_kind(mut self, original_kind: Option<SuggestionKind>) -> Self {
        self.original_kind = original_kind;
        self
    }
}
