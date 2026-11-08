use crate::modules::dropdown::domain::value_objects::menu_side::MenuSide;

/// Where the menu goes: the side of the cursor line, the list rows, the description row, the left column and the width.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct MenuLayout {
    side: MenuSide,
    /// List rows (0 in argument-hint mode).
    rows: usize,
    list_top: usize,
    desc_row: usize,
    x0: usize,
    width: usize,
}

/// What a `MenuLayout` is built from: named fields, so two row numbers cannot be swapped.
pub(crate) struct MenuLayoutParts {
    pub(crate) side: MenuSide,
    pub(crate) rows: usize,
    pub(crate) list_top: usize,
    pub(crate) desc_row: usize,
    pub(crate) x0: usize,
    pub(crate) width: usize,
}

impl MenuLayout {
    pub(crate) fn new(parts: MenuLayoutParts) -> Self {
        let MenuLayoutParts { side, rows, list_top, desc_row, x0, width } = parts;
        Self { side, rows, list_top, desc_row, x0, width }
    }

    #[cfg(test)]
    pub(crate) fn side(&self) -> MenuSide {
        self.side
    }

    pub(crate) fn rows(&self) -> usize {
        self.rows
    }

    pub(crate) fn list_top(&self) -> usize {
        self.list_top
    }

    pub(crate) fn desc_row(&self) -> usize {
        self.desc_row
    }

    pub(crate) fn x0(&self) -> usize {
        self.x0
    }

    pub(crate) fn width(&self) -> usize {
        self.width
    }
}
