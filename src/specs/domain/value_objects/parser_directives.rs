#[derive(Clone, Debug, PartialEq)]
pub(crate) struct ParserDirectives {
    flags_posix_noncompliant: bool,
    options_must_precede_args: bool,
    /// Default `["="]`.
    option_arg_separators: Vec<String>,
}

impl ParserDirectives {
    pub(crate) fn flags_posix_noncompliant(&self) -> bool {
        self.flags_posix_noncompliant
    }

    pub(crate) fn options_must_precede_args(&self) -> bool {
        self.options_must_precede_args
    }

    pub(crate) fn option_arg_separators(&self) -> &Vec<String> {
        &self.option_arg_separators
    }

    pub(crate) fn with_flags_posix_noncompliant(mut self, value: bool) -> Self {
        self.flags_posix_noncompliant = value;
        self
    }

    pub(crate) fn with_options_must_precede_args(mut self, value: bool) -> Self {
        self.options_must_precede_args = value;
        self
    }

    pub(crate) fn with_option_arg_separators(mut self, value: Vec<String>) -> Self {
        self.option_arg_separators = value;
        self
    }
}

impl Default for ParserDirectives {
    fn default() -> Self {
        ParserDirectives {
            flags_posix_noncompliant: false,
            options_must_precede_args: false,
            option_arg_separators: vec!["=".into()],
        }
    }
}
