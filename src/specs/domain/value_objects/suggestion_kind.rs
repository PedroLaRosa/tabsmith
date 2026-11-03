/// Suggestion row type (Fig `type`, plus internal `history` and `auto-execute`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum SuggestionKind {
    Subcommand,
    Option,
    Arg,
    File,
    Folder,
    Special,
    Shortcut,
    History,
    AutoExecute,
}
