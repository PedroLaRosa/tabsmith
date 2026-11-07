use crate::modules::terminal_session::domain::services::session_deadlines::RESIZE_DEBOUNCE;
use crate::modules::terminal_session::domain::value_objects::screen_state::ScreenState;
use std::time::Instant;

/// SPEC §7.6: the terminal resized, the overlay stays away until the shell stopped redrawing for a quiet spell, then
/// the whole model is repainted once.
#[derive(Clone, Debug, Default)]
pub(crate) struct ResizeDebounce {
    settle_at: Option<Instant>,
    repaint: bool,
}

impl ResizeDebounce {
    /// The quiet spell is armed last, after the pty and the model took the new size (P9-T21).
    pub(crate) fn resized(&mut self, now: Instant) {
        self.settle_at = Some(now + RESIZE_DEBOUNCE);
    }

    /// Output extends the quiet spell: the shell (ConPTY) is still redrawing. Without a resize it starts none.
    pub(crate) fn output_arrived(&mut self, now: Instant) {
        if self.settle_at.is_some() {
            self.settle_at = Some(now + RESIZE_DEBOUNCE);
        }
    }

    pub(crate) fn is_settling(&self) -> bool {
        self.settle_at.is_some()
    }

    pub(crate) fn repaint_pending(&self) -> bool {
        self.repaint
    }

    pub(crate) fn settle_if_due(&mut self, now: Instant) {
        if self.settle_at.is_some_and(|d| d <= now) {
            self.settle_at = None;
            self.repaint = true;
        }
    }

    /// Full repaint once the resize settled (postponed while on the alternate screen). The flag is consumed only
    /// when the repaint is due (P9-T45).
    pub(crate) fn take_repaint(&mut self, alt_screen: bool) -> bool {
        if !self.repaint || self.settle_at.is_some() || alt_screen {
            return false;
        }
        self.repaint = false;
        true
    }

    pub(crate) fn deadline(&self) -> Option<Instant> {
        self.settle_at
    }

    /// Test arrangement: the quiet spell ends at `at`.
    #[cfg(test)]
    pub(crate) fn expire_settle_at(&mut self, at: Instant) {
        self.settle_at = Some(at);
    }

    /// A size of zero, or the size the MODEL already has (not the pty's), is ignored (P9-T21).
    pub(crate) fn wants(&self, columns: u16, lines: u16, model: &ScreenState) -> bool {
        columns != 0 && lines != 0 && (columns as usize, lines as usize) != model.size()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn should_repaint_once_after_the_quiet_spell() {
        let now = Instant::now();
        let mut resize = ResizeDebounce::default();
        resize.resized(now);

        resize.settle_if_due(now + RESIZE_DEBOUNCE);
        let first = resize.take_repaint(false);
        let second = resize.take_repaint(false);

        assert_eq!((first, second), (true, false));
    }

    #[test]
    fn should_extend_the_quiet_spell_while_output_arrives() {
        let now = Instant::now();
        let mut resize = ResizeDebounce::default();
        resize.resized(now);

        resize.output_arrived(now + Duration::from_millis(100));

        assert_eq!(resize.deadline(), Some(now + Duration::from_millis(250)));
    }

    #[test]
    fn should_not_start_a_quiet_spell_for_output_without_a_resize() {
        let mut resize = ResizeDebounce::default();

        resize.output_arrived(Instant::now());

        assert_eq!(resize.deadline(), None);
    }

    #[test]
    fn should_postpone_the_repaint_on_the_alternate_screen() {
        let now = Instant::now();
        let mut resize = ResizeDebounce::default();
        resize.resized(now);
        resize.settle_if_due(now + RESIZE_DEBOUNCE);

        let on_alt = resize.take_repaint(true);

        assert_eq!((on_alt, resize.repaint_pending()), (false, true));
    }

    #[test]
    fn should_ignore_a_resize_to_zero_or_to_the_size_the_model_has() {
        let resize = ResizeDebounce::default();
        let model = ScreenState::plain(80, 24);

        let wanted = [
            resize.wants(0, 24, &model),
            resize.wants(80, 0, &model),
            resize.wants(80, 24, &model),
            resize.wants(81, 24, &model),
            resize.wants(80, 25, &model),
        ];

        assert_eq!(wanted, [false, false, false, true, true]);
    }
}
