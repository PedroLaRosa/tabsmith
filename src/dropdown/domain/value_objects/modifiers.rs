#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub(crate) struct Modifiers {
    pub(crate) ctrl: bool,
    pub(crate) alt: bool,
    pub(crate) shift: bool,
    pub(crate) meta: bool,
}

impl Modifiers {
    pub(crate) const fn none() -> Self {
        Self {
            ctrl: false,
            alt: false,
            shift: false,
            meta: false,
        }
    }

    pub(crate) const fn with_ctrl(self, ctrl: bool) -> Self {
        Self { ctrl, ..self }
    }

    pub(crate) const fn with_alt(self, alt: bool) -> Self {
        Self { alt, ..self }
    }

    pub(crate) const fn with_shift(self, shift: bool) -> Self {
        Self { shift, ..self }
    }

    pub(crate) const fn with_meta(self, meta: bool) -> Self {
        Self { meta, ..self }
    }

    pub(crate) fn ctrl(self) -> bool {
        self.ctrl
    }

    pub(crate) fn alt(self) -> bool {
        self.alt
    }

    pub(crate) fn shift(self) -> bool {
        self.shift
    }

    /// Only `Eq` reads the meta flag in production, so the reader is for tests.
    #[cfg(test)]
    pub(crate) fn meta(self) -> bool {
        self.meta
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const CTRL_ONLY: Modifiers = Modifiers::none().with_ctrl(true);

    fn flags(m: Modifiers) -> (bool, bool, bool, bool) {
        (m.ctrl(), m.alt(), m.shift(), m.meta())
    }

    /// Each builder sets (or clears) its own flag and leaves the other three as they were.
    #[test]
    fn should_set_one_modifier_without_touching_the_others() {
        let alt = Modifiers::none().with_alt(true);

        let all = alt.with_ctrl(true).with_shift(true).with_meta(true);
        let cleared = all.with_alt(false);

        assert_eq!(
            [alt, all, cleared, CTRL_ONLY].map(flags),
            [
                (false, true, false, false),
                (true, true, true, true),
                (true, false, true, true),
                (true, false, false, false),
            ]
        );
        assert_eq!(Modifiers::none(), Modifiers::default());
    }
}
