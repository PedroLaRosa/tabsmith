/// The text attributes a shell's prediction colour can require besides its colours.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct TextEmphasis {
    dim: bool,
    italic: bool,
}

impl TextEmphasis {
    pub(crate) const fn new(dim: bool, italic: bool) -> TextEmphasis {
        TextEmphasis { dim, italic }
    }

    pub(crate) fn is_empty(self) -> bool {
        !self.dim && !self.italic
    }

    /// Every attribute of `self` is also set in `other`; the empty emphasis is within any.
    pub(crate) fn is_within(self, other: TextEmphasis) -> bool {
        (!self.dim || other.dim) && (!self.italic || other.italic)
    }

    pub(crate) fn is_dim_or_italic(self) -> bool {
        self.dim || self.italic
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn should_be_within_an_emphasis_that_has_every_attribute_and_an_empty_one_within_any() {
        let dim_italic = TextEmphasis::new(true, true);
        let dim = TextEmphasis::new(true, false);
        let italic = TextEmphasis::new(false, true);

        assert!(dim.is_within(dim_italic));
        assert!(dim_italic.is_within(dim_italic));
        assert!(!dim_italic.is_within(dim));
        assert!(!dim.is_within(italic));
        assert!(TextEmphasis::default().is_within(TextEmphasis::default()));
        assert!(TextEmphasis::default().is_within(italic));
    }

    #[test]
    fn should_tell_dim_or_italic() {
        assert!(TextEmphasis::new(true, false).is_dim_or_italic());
        assert!(TextEmphasis::new(false, true).is_dim_or_italic());
        assert!(TextEmphasis::new(true, true).is_dim_or_italic());
        assert!(!TextEmphasis::default().is_dim_or_italic());
    }
}
