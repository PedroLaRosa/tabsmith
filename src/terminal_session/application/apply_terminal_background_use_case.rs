use crate::dropdown::domain::services::theme_resolver::resolve_theme;
use crate::dropdown::domain::repositories::theme_repository::ThemeRepository;
use crate::terminal_session::application::ports::terminal_screen::TerminalScreen;
use crate::terminal_session::domain::entities::terminal_session::TerminalSession;
use crate::terminal_session::domain::value_objects::terminal_background::TerminalBackground;
use std::rc::Rc;

/// The terminal answered the background probe after the start-up timeout: the `system` theme can pick light or dark
/// from it now.
pub(crate) struct ApplyTerminalBackgroundUseCase {
    screen: Rc<dyn TerminalScreen>,
    themes: Rc<dyn ThemeRepository>,
}

impl ApplyTerminalBackgroundUseCase {
    pub(crate) fn new(screen: Rc<dyn TerminalScreen>, themes: Rc<dyn ThemeRepository>) -> Self {
        ApplyTerminalBackgroundUseCase { screen, themes }
    }

    /// Re-themes with the current settings, whether or not the theme changed.
    pub(crate) fn execute(&self, session: &mut TerminalSession, background: TerminalBackground) {
        session.probe_background_reported(background);
        self.screen
            .set_theme(resolve_theme(session.settings(), Some(background.rgb()), &*self.themes));
    }
}

#[cfg(test)]
mod tests {
    use crate::completion::domain::test_fixtures::settings;
    use crate::dropdown::domain::value_objects::theme::Theme;
    use crate::terminal_session::application::ports::terminal_screen::TerminalScreen;
    use crate::terminal_session::application::test_fixtures::{subs, TerminalSessionHarness};
    use crate::terminal_session::domain::value_objects::terminal_background::TerminalBackground;
    use std::time::{Duration, SystemTime};

    const SYSTEM_THEME: &str = r#"{"autocomplete.theme": "system"}"#;
    fn white() -> TerminalBackground {
        TerminalBackground::new((255, 255, 255))
    }

    fn black() -> TerminalBackground {
        TerminalBackground::new((0, 0, 0))
    }

    fn harness_showing_a_menu(settings_json: &str) -> TerminalSessionHarness {
        let mut harness = TerminalSessionHarness::new(settings_json);
        harness.prompt("git ch");
        harness.reply(subs("ch"), None);
        harness
    }

    fn frame_after_a_redraw(harness: &mut TerminalSessionHarness) -> Vec<u8> {
        harness.clear_written();
        harness.redraw();
        harness.written()
    }

    #[test]
    fn should_recolour_the_screen_from_the_reported_background_for_the_system_theme() {
        let mut harness = harness_showing_a_menu(SYSTEM_THEME);
        let dark_frame = frame_after_a_redraw(&mut harness);
        let mut reference = harness_showing_a_menu(SYSTEM_THEME);
        reference.screen.set_theme(Theme::light());
        let light_frame = frame_after_a_redraw(&mut reference);

        harness.background(white());

        let recoloured = frame_after_a_redraw(&mut harness);
        assert_eq!((recoloured == light_frame, recoloured == dark_frame), (true, false));
    }

    #[test]
    fn should_set_the_theme_even_when_it_did_not_change() {
        let mut harness = harness_showing_a_menu("{}");
        harness.redraw();
        harness.clear_written();

        harness.background(black());

        harness.redraw();
        assert!(!harness.written().is_empty());
    }

    #[test]
    fn should_remember_the_background_for_later_reloads() {
        let mut harness = harness_showing_a_menu(SYSTEM_THEME);
        let reloaded = r#"{"autocomplete.theme": "system", "inline.enabled": false}"#;
        let mut reference = harness_showing_a_menu(reloaded);
        reference.screen.set_theme(Theme::light());
        let light_frame = frame_after_a_redraw(&mut reference);
        harness.background(white());

        harness.live_settings.set(settings(reloaded), SystemTime::UNIX_EPOCH + Duration::from_secs(5));
        harness.use_cases.poller().poll(&mut harness.session);

        assert_eq!(frame_after_a_redraw(&mut harness), light_frame);
    }
}
