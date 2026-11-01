/// What the render guards read of the screen (SPEC §7.5.1), as one snapshot taken at one moment.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct ScreenState {
    alt_screen: bool,
    origin_mode: bool,
    insert_mode: bool,
    pending_wrap: bool,
    columns: usize,
    lines: usize,
    scroll_region: bool,
    in_string: bool,
}

#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct ScreenStateParts {
    pub(crate) alt_screen: bool,
    pub(crate) origin_mode: bool,
    pub(crate) insert_mode: bool,
    pub(crate) pending_wrap: bool,
    pub(crate) columns: usize,
    pub(crate) lines: usize,
    pub(crate) scroll_region: bool,
    pub(crate) in_string: bool,
}

impl ScreenState {
    #[cfg(test)]
    pub(crate) fn plain(columns: usize, lines: usize) -> Self {
        ScreenState::new(ScreenStateParts {
            columns,
            lines,
            ..ScreenStateParts::default()
        })
    }

    pub(crate) fn new(p: ScreenStateParts) -> Self {
        ScreenState {
            alt_screen: p.alt_screen,
            origin_mode: p.origin_mode,
            insert_mode: p.insert_mode,
            pending_wrap: p.pending_wrap,
            columns: p.columns,
            lines: p.lines,
            scroll_region: p.scroll_region,
            in_string: p.in_string,
        }
    }

    pub(crate) fn alt_screen(&self) -> bool {
        self.alt_screen
    }
    pub(crate) fn origin_mode(&self) -> bool {
        self.origin_mode
    }
    pub(crate) fn insert_mode(&self) -> bool {
        self.insert_mode
    }
    pub(crate) fn pending_wrap(&self) -> bool {
        self.pending_wrap
    }
    pub(crate) fn columns(&self) -> usize {
        self.columns
    }
    pub(crate) fn lines(&self) -> usize {
        self.lines
    }
    pub(crate) fn scroll_region(&self) -> bool {
        self.scroll_region
    }
    pub(crate) fn in_string(&self) -> bool {
        self.in_string
    }
    pub(crate) fn size(&self) -> (usize, usize) {
        (self.columns, self.lines)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn should_report_the_size_as_columns_then_lines() {
        let state = ScreenState::plain(80, 24);

        let size = state.size();

        assert_eq!(size, (80, 24));
    }
}
