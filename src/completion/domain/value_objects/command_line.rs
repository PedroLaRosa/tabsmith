use crate::completion::domain::value_objects::token::Token;

/// The command under the cursor.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct CommandLine {
    /// Tokens up to the cursor; a trailing empty token when the text ends with whitespace.
    tokens: Vec<Token>,
    /// Source text of the command (after alias expansion).
    text: String,
    /// Source text as typed, before alias expansion (what the menu compares between keystrokes).
    original_text: String,
}

impl CommandLine {
    pub(crate) fn new(tokens: Vec<Token>, text: String, original_text: String) -> Self {
        Self {
            tokens,
            text,
            original_text,
        }
    }

    pub(crate) fn tokens(&self) -> &[Token] {
        &self.tokens
    }

    pub(crate) fn text(&self) -> &str {
        &self.text
    }

    pub(crate) fn original_text(&self) -> &str {
        &self.original_text
    }
}
