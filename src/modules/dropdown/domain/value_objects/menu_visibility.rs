#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum MenuVisibility {
    Visible,
    HiddenUntilKeypress,
    HiddenUntilShown,
    HiddenByInsertion,
}
