/// One argument of the current command.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Token {
    /// Unquoted/unescaped text (`inner_text`).
    text: String,
    /// Raw source text as typed (quotes/escapes included).
    raw: String,
    /// Byte offsets into the (alias-expanded) command source.
    start: usize,
    end: usize,
}

impl Token {
    /// `start`/`end` are the byte span in `CommandLine::text`.
    pub(crate) fn new(text: String, raw: String, start: usize, end: usize) -> Self {
        Self { text, raw, start, end }
    }

    pub(crate) fn text(&self) -> &str {
        &self.text
    }

    pub(crate) fn raw(&self) -> &str {
        &self.raw
    }

    pub(crate) fn start(&self) -> usize {
        self.start
    }

    pub(crate) fn end(&self) -> usize {
        self.end
    }

    /// The token with the raw text of the token it replaces (an alias keeps what is on screen).
    pub(crate) fn with_raw(self, raw: String) -> Self {
        Self { raw, ..self }
    }
}
