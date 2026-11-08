use crate::modules::dropdown::domain::value_objects::menu_layout::{MenuLayout, MenuLayoutParts};
use crate::modules::dropdown::domain::value_objects::menu_side::MenuSide;

/// SPEC §7.4.1 (D8: never scroll; below, above, shrink, else hide). `items == 0` = argument-hint mode (one row).
pub(crate) fn layout(
    (rows, cols): (usize, usize),
    (cy, cx): (usize, usize),
    items: usize,
    n_max: usize,
    menu_w: usize,
    query_w: usize,
) -> Option<MenuLayout> {
    let w = menu_w.min(cols);
    let mut n = n_max.min(items);
    let mut h = n + 1;
    let (below, above) = (rows - 1 - cy, cy);
    let side = match () {
        _ if below >= h => MenuSide::Below,
        _ if above >= h => MenuSide::Above,
        _ if below >= above => MenuSide::Below,
        _ => MenuSide::Above,
    };
    let room = if side == MenuSide::Below { below } else { above };
    if room < h {
        if items == 0 || room < 2 {
            return None;
        }
        n = room - 1;
        h = room;
    }
    // Rows start with `' '` + icon (2 columns) + `' '`: 4 columns before the title.
    let x0 = (cx as isize - query_w as isize - 4).clamp(0, (cols - w) as isize) as usize;
    let (list_top, desc_row) = match side {
        MenuSide::Below => (cy + 1, cy + n + 1),
        MenuSide::Above => (cy - h, cy - 1),
    };
    Some(MenuLayout::new(MenuLayoutParts {
        side,
        rows: n,
        list_top,
        desc_row,
        x0,
        width: w,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn layout_decisions() {
        // (rows, cols), (cy, cx), items, n_max, width, query width
        let l = layout((24, 40), (2, 10), 5, 7, 20, 0).unwrap();
        assert_eq!(
            l,
            MenuLayout::new(MenuLayoutParts {
                side: MenuSide::Below,
                rows: 5,
                list_top: 3,
                desc_row: 8,
                x0: 6,
                width: 20
            })
        );
        let l = layout((24, 40), (22, 10), 9, 7, 20, 0).unwrap();
        assert_eq!(
            l,
            MenuLayout::new(MenuLayoutParts {
                side: MenuSide::Above,
                rows: 7,
                list_top: 14,
                desc_row: 21,
                x0: 6,
                width: 20
            })
        );
        // Neither side fits: shrink on the bigger side (above here).
        let l = layout((10, 40), (5, 10), 9, 7, 20, 0).unwrap();
        assert_eq!(
            l,
            MenuLayout::new(MenuLayoutParts {
                side: MenuSide::Above,
                rows: 4,
                list_top: 0,
                desc_row: 4,
                x0: 6,
                width: 20
            })
        );
        let l = layout((10, 40), (4, 10), 9, 7, 20, 0).unwrap();
        assert_eq!(
            (l.side(), l.rows(), l.list_top(), l.desc_row()),
            (MenuSide::Below, 4, 5, 9)
        );
        // No room for one row + description: hide. Argument hint needs one row.
        assert_eq!(layout((3, 40), (1, 10), 3, 7, 20, 0), None);
        let l = layout((3, 40), (1, 10), 0, 7, 20, 0).unwrap();
        assert_eq!(
            (l.side(), l.rows(), l.list_top(), l.desc_row()),
            (MenuSide::Below, 0, 2, 2)
        );
        let l = layout((3, 40), (2, 10), 0, 7, 20, 0).unwrap();
        assert_eq!((l.side(), l.list_top(), l.desc_row()), (MenuSide::Above, 1, 1));
        // x0 clamping (aligned under the typed token, kept on screen) and width clamping.
        // Title (after `' '` + icon + `' '`) starts in the column of the typed token: 12 - 3 = 5 + 4.
        assert_eq!(layout((24, 40), (2, 12), 3, 7, 20, 3).unwrap().x0(), 5);
        assert_eq!(layout((24, 40), (2, 2), 3, 7, 20, 5).unwrap().x0(), 0);
        assert_eq!(layout((24, 40), (2, 39), 3, 7, 20, 0).unwrap().x0(), 20);
        let l = layout((24, 30), (2, 9), 3, 7, 40, 0).unwrap();
        assert_eq!(
            l,
            MenuLayout::new(MenuLayoutParts {
                side: MenuSide::Below,
                rows: 3,
                list_top: 3,
                desc_row: 6,
                x0: 0,
                width: 30
            })
        );
    }
}
