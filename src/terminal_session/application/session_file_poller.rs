use crate::dropdown::domain::repositories::theme_repository::ThemeRepository;
use crate::dropdown::domain::services::theme_resolver::resolve_theme;
use crate::terminal_session::application::ports::completion_engine::CompletionEngine;
use crate::terminal_session::application::ports::live_settings::LiveSettings;
use crate::terminal_session::application::ports::terminal_screen::TerminalScreen;
use crate::terminal_session::domain::entities::terminal_session::TerminalSession;
use crate::terminal_session::domain::repositories::session_context_repository::SessionContextRepository;
use crate::terminal_session::domain::value_objects::terminal_background::TerminalBackground;
use std::rc::Rc;

/// Looks at the files a session follows while it runs: the settings, reloaded live, and the session context that
/// `_ pre-cmd` reports (D17). What changed goes to the session, the screen and the completion engine.
pub(crate) struct SessionFilePoller {
    live_settings: Rc<dyn LiveSettings>,
    session_contexts: Rc<dyn SessionContextRepository>,
    engine: Rc<dyn CompletionEngine>,
    screen: Rc<dyn TerminalScreen>,
    themes: Rc<dyn ThemeRepository>,
}

impl SessionFilePoller {
    pub(crate) fn new(
        live_settings: Rc<dyn LiveSettings>,
        session_contexts: Rc<dyn SessionContextRepository>,
        engine: Rc<dyn CompletionEngine>,
        screen: Rc<dyn TerminalScreen>,
        themes: Rc<dyn ThemeRepository>,
    ) -> Self {
        SessionFilePoller { live_settings, session_contexts, engine, screen, themes }
    }

    /// Settings (live reload) and the session context from `_ pre-cmd` (D17). True when settings changed. The context
    /// comes first, then the settings, and the engine hears the new settings last (P9-T44).
    pub(crate) fn poll(&self, session: &mut TerminalSession) -> bool {
        let modified = self.session_contexts.last_modified(session.id());
        if session.context_changed(modified) {
            if let Some(context) = self.session_contexts.find(session.id()) {
                self.engine
                    .apply_session_context(context.env().clone(), context.alias().map(str::to_owned));
            }
        }
        let modified = self.live_settings.last_modified();
        if !session.settings_changed(modified) {
            return false;
        }
        session.apply_settings(self.live_settings.load());
        self.screen.set_theme(resolve_theme(
            session.settings(),
            session.terminal_background().map(TerminalBackground::rgb),
            &*self.themes,
        ));
        self.engine.apply_settings(session.settings().clone());
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::completion::domain::test_fixtures::{comp, item, settings};
    use crate::dropdown::domain::value_objects::key::Key;
    use crate::dropdown::domain::value_objects::key_action::KeyAction;
    use crate::dropdown::domain::value_objects::key_code::KeyCode;
    use crate::dropdown::domain::value_objects::modifiers::Modifiers;
    use crate::settings::domain::value_objects::settings::Settings;
    use crate::shared::domain::value_objects::data_directory::DataDirectory;
    use crate::shared::infrastructure::testing::temp_dir::TempDir;
    use crate::specs::domain::value_objects::suggestion_kind::SuggestionKind;
    use crate::terminal_session::application::ports::completion_engine::CompletionEngineCall;
    use crate::terminal_session::application::test_fixtures::{TerminalSessionHarness, SID};
    use crate::terminal_session::domain::entities::session_context::SessionContext;
    use crate::terminal_session::domain::repositories::session_context_repository::InMemorySessionContextRepository;
    use crate::terminal_session::domain::value_objects::edit_buffer::EditBuffer;
    use crate::terminal_session::infrastructure::adapters::file_session_context_repository::FileSessionContextRepository;
    use crate::terminal_session::domain::value_objects::key_route::KeyRoute;
    use crate::terminal_session::domain::value_objects::session_id::SessionId;
    use crate::terminal_session::domain::value_objects::shell_marker::ShellMarker;
    use std::collections::HashMap;
    use std::sync::mpsc::Receiver;
    use std::time::{Duration, SystemTime, UNIX_EPOCH};

    fn at(secs: u64) -> SystemTime {
        UNIX_EPOCH + Duration::from_secs(secs)
    }

    fn settings_sent(h: &TerminalSessionHarness) -> Vec<Settings> {
        h.engine_calls
            .try_iter()
            .filter_map(|call| match call {
                CompletionEngineCall::ApplySettings(settings) => Some(settings),
                _ => None,
            })
            .collect()
    }

    /// What the engine was told, in order: `session context` or `settings`.
    fn told_to_the_engine(h: &TerminalSessionHarness) -> Vec<&'static str> {
        h.engine_calls
            .try_iter()
            .map(|call| match call {
                CompletionEngineCall::ApplySessionContext { .. } => "session context",
                CompletionEngineCall::ApplySettings(_) => "settings",
                other => panic!("only files are polled: {other:?}"),
            })
            .collect()
    }

    /// The session is at a prompt that shows `git ch` with a menu of two items.
    fn show_menu(h: &mut TerminalSessionHarness) {
        let now = h.clock.now_instant();
        h.session.tracker_mut().on_marker(ShellMarker::parse(&format!("NewCmd={SID}")), 0, 0);
        let line = EditBuffer::new("git ch".into(), 6, false, true);
        h.session.refresh(&h.screen.state(), Some(line), now);
        let items = ["cherry-pick", "checkout"]
            .iter()
            .map(|name| item(name, SuggestionKind::Subcommand, "ch"))
            .collect();
        let reply = comp("git ch", 1, items).with_request_id(h.session.request_id());
        h.session.apply_completion(reply.clone(), now);
        h.session.apply_completion(reply, now);
    }

    #[test]
    fn should_reload_the_settings_when_their_file_changed_and_tell_the_engine_last() {
        let mut h = TerminalSessionHarness::new("{}");
        let reloaded = settings(r#"{"inline.enabled": false}"#);
        h.live_settings.set(reloaded.clone(), at(5));

        let changed = h.use_cases.poller().poll(&mut h.session);

        assert_eq!((changed, h.session.settings().clone(), settings_sent(&h)), (true, reloaded.clone(), vec![reloaded]));
    }

    #[test]
    fn should_load_the_defaults_once_the_settings_file_is_gone() {
        let mut h = TerminalSessionHarness::new("{}");
        h.live_settings.set(settings(r#"{"inline.enabled": false}"#), at(5));
        h.use_cases.poller().poll(&mut h.session);
        h.live_settings.remove();

        let changed = h.use_cases.poller().poll(&mut h.session);

        assert_eq!((changed, h.session.settings().clone()), (true, Settings::default()));
    }

    #[test]
    fn should_set_the_theme_even_when_the_theme_did_not_change() {
        let mut h = TerminalSessionHarness::new("{}");
        show_menu(&mut h);
        h.redraw();
        h.clear_written();
        h.live_settings.set(settings(r#"{"inline.enabled": false}"#), at(5));
        h.use_cases.poller().poll(&mut h.session);

        h.redraw();

        assert!(!h.written().is_empty());
    }

    #[test]
    fn should_rebuild_the_key_bindings_from_the_reloaded_settings() {
        let mut h = TerminalSessionHarness::new("{}");
        show_menu(&mut h);
        let tab = Key::new(KeyCode::Tab, Modifiers::default());
        let before = h.session.route_key(&tab, 0, &h.screen.state());
        h.live_settings.set(settings(r#"{"autocomplete.keybindings.tab": "ignore"}"#), at(5));

        h.use_cases.poller().poll(&mut h.session);

        let after = h.session.route_key(&tab, 0, &h.screen.state());
        assert_eq!((before, after), (KeyRoute::MenuAction(KeyAction::InsertCommonPrefix), KeyRoute::Forward));
    }

    #[test]
    fn should_tell_the_engine_the_session_context_before_the_settings() {
        let contexts = Rc::new(InMemorySessionContextRepository::default());
        let id = SessionId::new("abc".to_string()).unwrap();
        contexts.save_modified_at(&SessionContext::new(id.clone(), HashMap::new(), None, 0), at(7));
        let mut h = TerminalSessionHarness::with_session_contexts(id, contexts);
        h.live_settings.set(settings(r#"{"inline.enabled": false}"#), at(5));

        h.use_cases.poller().poll(&mut h.session);

        assert_eq!(told_to_the_engine(&h), ["session context", "settings"]);
    }

    #[test]
    fn should_not_reload_settings_while_their_time_is_unchanged() {
        let mut h = TerminalSessionHarness::new("{}");
        let first = settings(r#"{"inline.enabled": false}"#);
        h.live_settings.set(first.clone(), at(5));
        h.use_cases.poller().poll(&mut h.session);
        h.live_settings.set(settings(r#"{"inline.enabled": true}"#), at(5));

        let changed = h.use_cases.poller().poll(&mut h.session);

        assert_eq!((changed, h.session.settings().clone()), (false, first));
    }

    /// A session reading its context from `<temp>/run/abc.json`; what it sends the engine is in `engine_calls`.
    fn session_reading_context_in(temp_dir: &TempDir) -> TerminalSessionHarness {
        TerminalSessionHarness::with_session_contexts(
            SessionId::new("abc".to_string()).unwrap(),
            Rc::new(FileSessionContextRepository::new(DataDirectory::new(
                Some(temp_dir.path().to_path_buf()),
                None,
            ))),
        )
    }

    fn write_context_file(temp_dir: &TempDir, json: &str, modified: SystemTime) {
        let path = temp_dir.path().join("run/abc.json");
        std::fs::create_dir_all(temp_dir.path().join("run")).unwrap();
        std::fs::write(&path, json).unwrap();
        std::fs::File::options()
            .write(true)
            .open(&path)
            .unwrap()
            .set_modified(modified)
            .unwrap();
    }

    fn contexts_sent(sent_to_engine: &Receiver<CompletionEngineCall>) -> Vec<(HashMap<String, String>, Option<String>)> {
        sent_to_engine
            .try_iter()
            .map(|m| match m {
                CompletionEngineCall::ApplySessionContext { env, alias } => (env, alias),
                _ => panic!("only session contexts are sent"),
            })
            .collect()
    }

    #[test]
    fn should_send_the_engine_the_env_and_alias_of_a_saved_session_context() {
        let temp_dir = TempDir::new("pty-context-sent");
        let mut h = session_reading_context_in(&temp_dir);
        write_context_file(&temp_dir, r#"{"env":{"A":"1"},"alias":"ll='ls -l'","ts":7}"#, at(1_000));

        h.use_cases.poller().poll(&mut h.session);

        assert_eq!(
            contexts_sent(&h.engine_calls),
            [(
                HashMap::from([("A".to_string(), "1".to_string())]),
                Some("ll='ls -l'".to_string())
            )]
        );
    }

    #[test]
    fn should_not_send_the_session_context_again_while_its_file_keeps_its_modification_time() {
        let temp_dir = TempDir::new("pty-context-unchanged");
        let mut h = session_reading_context_in(&temp_dir);
        write_context_file(&temp_dir, r#"{"env":{"A":"1"}}"#, at(1_000));
        h.use_cases.poller().poll(&mut h.session);
        contexts_sent(&h.engine_calls);

        h.use_cases.poller().poll(&mut h.session);

        assert_eq!(contexts_sent(&h.engine_calls), Vec::new());
    }

    /// A removed file is no change: its context is not reloaded until the file comes back with a new time.
    #[test]
    fn should_send_nothing_and_keep_the_modification_time_once_the_context_file_is_deleted() {
        let temp_dir = TempDir::new("pty-context-deleted");
        let mut h = session_reading_context_in(&temp_dir);
        write_context_file(&temp_dir, r#"{"env":{"A":"1"}}"#, at(1_000));
        h.use_cases.poller().poll(&mut h.session);
        contexts_sent(&h.engine_calls);
        std::fs::remove_file(temp_dir.path().join("run/abc.json")).unwrap();

        h.use_cases.poller().poll(&mut h.session);

        assert_eq!(
            (contexts_sent(&h.engine_calls), h.session.context_modified()),
            (Vec::new(), Some(at(1_000)))
        );
    }

    /// The time is kept before the file is read, so a file that cannot be read is not read again until it changes.
    #[test]
    fn should_send_nothing_for_a_context_file_that_is_not_json_but_keep_its_new_modification_time() {
        let temp_dir = TempDir::new("pty-context-bad-json");
        let mut h = session_reading_context_in(&temp_dir);
        write_context_file(&temp_dir, r#"{"env":{"A":"1"}}"#, at(1_000));
        h.use_cases.poller().poll(&mut h.session);
        contexts_sent(&h.engine_calls);
        write_context_file(&temp_dir, "not json", at(2_000));

        h.use_cases.poller().poll(&mut h.session);

        assert_eq!(
            (contexts_sent(&h.engine_calls), h.session.context_modified()),
            (Vec::new(), Some(at(2_000)))
        );
    }

    #[test]
    fn should_send_the_session_context_again_once_its_file_is_rewritten_with_a_new_modification_time() {
        let temp_dir = TempDir::new("pty-context-rewritten");
        let mut h = session_reading_context_in(&temp_dir);
        write_context_file(&temp_dir, r#"{"env":{"A":"1"}}"#, at(1_000));
        h.use_cases.poller().poll(&mut h.session);
        contexts_sent(&h.engine_calls);
        write_context_file(&temp_dir, r#"{"env":{"B":"2"},"alias":""}"#, at(2_000));

        h.use_cases.poller().poll(&mut h.session);

        assert_eq!(
            contexts_sent(&h.engine_calls),
            [(
                HashMap::from([("B".to_string(), "2".to_string())]),
                Some(String::new())
            )]
        );
    }
}
