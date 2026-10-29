#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum AnnotationKind {
    Subcommand,
    Option,
    OptionArg,
    SubcommandArg,
    /// Token that could not be consumed (only possible for the final token).
    None,
}
