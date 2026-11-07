use crate::modules::completion::domain::value_objects::insertion::Insertion;

/// What inserting the common prefix of the listed suggestions does.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum CommonPrefixOutcome {
    /// Nothing can be inserted: the menu rings the bell, or falls back to what its action says.
    Failed,
    /// The selected row is inserted in full, without executing it.
    InsertSelected,
    /// Only the start the rows share is typed; the menu stays visible.
    InsertPrefix(Insertion),
}
