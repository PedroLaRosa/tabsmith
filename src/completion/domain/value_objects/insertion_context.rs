use crate::shared::domain::value_objects::shell::Shell;

pub(crate) struct InsertionContext<'a> {
    pub(crate) search_term: &'a str,
    /// Raw on-screen text of the current token (quotes/escapes included).
    pub(crate) raw: &'a str,
    pub(crate) shell: Shell,
    pub(crate) fuzzy: bool,
    pub(crate) prefer_verbose: bool,
    pub(crate) insert_space: bool,
}
