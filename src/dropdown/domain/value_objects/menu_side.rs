/// Which side of the cursor line the menu is drawn on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum MenuSide {
    Below,
    Above,
}
