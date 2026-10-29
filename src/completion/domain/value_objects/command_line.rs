use crate::completion::domain::value_objects::token::Token;

/// The command under the cursor.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct CommandLine {
    /// Tokens up to the cursor; a trailing empty token when the text ends with whitespace.
    pub(crate) tokens: Vec<Token>,
    /// Source text of the command (after alias expansion).
    pub(crate) text: String,
    /// Source text as typed, before alias expansion (what the menu compares between keystrokes).
    pub(crate) original_text: String,
}
