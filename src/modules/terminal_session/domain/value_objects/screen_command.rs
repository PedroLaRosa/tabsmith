use crate::modules::terminal_session::domain::value_objects::cell_tag::CellTag;

/// What the model's cells must get so the tracker can read the command line back (SPEC D3), in the order given.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum ScreenCommand {
    /// Cells written from here on carry the tag.
    Tag(CellTag),
    /// Cells written from here on carry no tag.
    ClearTag,
    /// A prompt ended at the cursor: the command line starts here. `redrawn_cycle` is the cycle that was open (a
    /// redraw of the prompt), whose cells are re-tagged.
    MarkLineStart { redrawn_cycle: Option<u64> },
}
