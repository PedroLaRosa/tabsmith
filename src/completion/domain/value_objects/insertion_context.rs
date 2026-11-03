use crate::shared::domain::value_objects::shell::Shell;

pub(crate) struct InsertionContext<'a> {
    search_term: &'a str,
    /// Raw on-screen text of the current token (quotes/escapes included).
    raw: &'a str,
    shell: Shell,
    fuzzy: bool,
    prefer_verbose: bool,
    insert_space: bool,
}

impl<'a> InsertionContext<'a> {
    /// The three flags start false; set each with its `with_*`.
    pub(crate) fn new(search_term: &'a str, raw: &'a str, shell: Shell) -> Self {
        InsertionContext {
            search_term,
            raw,
            shell,
            fuzzy: false,
            prefer_verbose: false,
            insert_space: false,
        }
    }

    pub(crate) fn search_term(&self) -> &'a str {
        self.search_term
    }

    pub(crate) fn raw(&self) -> &'a str {
        self.raw
    }

    pub(crate) fn shell(&self) -> Shell {
        self.shell
    }

    pub(crate) fn fuzzy(&self) -> bool {
        self.fuzzy
    }

    pub(crate) fn prefer_verbose(&self) -> bool {
        self.prefer_verbose
    }

    pub(crate) fn insert_space(&self) -> bool {
        self.insert_space
    }

    pub(crate) fn with_fuzzy(mut self, fuzzy: bool) -> Self {
        self.fuzzy = fuzzy;
        self
    }

    pub(crate) fn with_prefer_verbose(mut self, prefer_verbose: bool) -> Self {
        self.prefer_verbose = prefer_verbose;
        self
    }

    pub(crate) fn with_insert_space(mut self, insert_space: bool) -> Self {
        self.insert_space = insert_space;
        self
    }
}
