#[derive(Clone, Debug, PartialEq)]
pub(crate) struct ParserDirectives {
    pub(crate) flags_posix_noncompliant: bool,
    pub(crate) options_must_precede_args: bool,
    /// Default `["="]`.
    pub(crate) option_arg_separators: Vec<String>,
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
