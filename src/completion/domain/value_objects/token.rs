/// One argument of the current command.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Token {
    /// Unquoted/unescaped text (`inner_text`).
    pub(crate) text: String,
    /// Raw source text as typed (quotes/escapes included).
    pub(crate) raw: String,
    /// Byte offsets into the (alias-expanded) command source.
    pub(crate) start: usize,
    pub(crate) end: usize,
}
