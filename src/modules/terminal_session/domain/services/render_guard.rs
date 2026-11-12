use crate::modules::terminal_session::domain::value_objects::screen_state::ScreenState;

/// The screen leaves room for an overlay (SPEC §7.5.1): no alternate screen, origin or insert mode, pending wrap
/// or scroll region, and at least 10 columns by 3 lines. The overlay renderer's `compose` refuses a frame in exactly
/// the same cases (its parity test pins it).
pub(crate) fn allows_drawing(screen: &ScreenState) -> bool {
    !screen.scroll_region()
        && !(screen.alt_screen() || screen.origin_mode() || screen.insert_mode())
        && !screen.pending_wrap()
        && screen.columns() >= 10
        && screen.lines() >= 3
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::terminal_session::domain::value_objects::screen_state::ScreenStateParts;

    fn screen(columns: usize, lines: usize, adjust: impl FnOnce(&mut ScreenStateParts)) -> ScreenState {
        let mut parts = ScreenStateParts {
            columns,
            lines,
            ..ScreenStateParts::default()
        };
        adjust(&mut parts);
        ScreenState::new(parts)
    }

    #[test]
    fn should_allow_drawing_on_a_plain_80_by_24_screen() {
        let screen = ScreenState::plain(80, 24);

        assert!(allows_drawing(&screen));
    }

    #[test]
    fn should_not_allow_drawing_with_a_scroll_region() {
        let screen = screen(80, 24, |p| p.scroll_region = true);

        assert!(!allows_drawing(&screen));
    }

    #[test]
    fn should_not_allow_drawing_on_the_alternate_screen() {
        let screen = screen(80, 24, |p| p.alt_screen = true);

        assert!(!allows_drawing(&screen));
    }

    #[test]
    fn should_not_allow_drawing_in_origin_mode() {
        let screen = screen(80, 24, |p| p.origin_mode = true);

        assert!(!allows_drawing(&screen));
    }

    #[test]
    fn should_not_allow_drawing_in_insert_mode() {
        let screen = screen(80, 24, |p| p.insert_mode = true);

        assert!(!allows_drawing(&screen));
    }

    #[test]
    fn should_not_allow_drawing_with_a_pending_wrap() {
        let screen = screen(80, 24, |p| p.pending_wrap = true);

        assert!(!allows_drawing(&screen));
    }

    #[test]
    fn should_need_ten_columns_to_draw() {
        let nine = ScreenState::plain(9, 24);
        let ten = ScreenState::plain(10, 24);

        assert_eq!((allows_drawing(&nine), allows_drawing(&ten)), (false, true));
    }

    #[test]
    fn should_need_three_lines_to_draw() {
        let two = ScreenState::plain(80, 2);
        let three = ScreenState::plain(80, 3);

        assert_eq!((allows_drawing(&two), allows_drawing(&three)), (false, true));
    }
}
