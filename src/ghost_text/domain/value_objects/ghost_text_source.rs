use crate::ghost_text::domain::value_objects::llm_profile::LlmProfile;

/// The selected inline source (`inline.source`, default `history+specs`).
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum GhostTextSource {
    History,
    Specs,
    HistoryAndSpecs,
    Profile(LlmProfile),
}

/// Built-in source names with descriptions, in display order.
pub(crate) const BUILTIN_SOURCES: [(&str, &str); 3] = [
    ("history+specs", "Local history, then completion specs"),
    ("history", "Local history only"),
    ("specs", "Completion specs only"),
];
