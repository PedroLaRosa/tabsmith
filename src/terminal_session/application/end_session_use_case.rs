use crate::terminal_session::application::ports::completion_engine::CompletionEngine;
use crate::terminal_session::application::ports::generator_process_reaper::GeneratorProcessReaper;
use crate::terminal_session::application::ports::terminal_modes::TerminalModes;
use crate::terminal_session::application::ports::terminal_screen::TerminalScreen;
use crate::terminal_session::application::ports::terminal_writer::TerminalWriter;
use crate::terminal_session::domain::entities::terminal_session::TerminalSession;
use crate::terminal_session::domain::repositories::session_context_repository::SessionContextRepository;
use std::rc::Rc;

/// The session is over, whichever way the loop was left: the user gets a clean terminal and everything the session
/// started is stopped.
pub(crate) struct EndSessionUseCase {
    screen: Rc<dyn TerminalScreen>,
    writer: Rc<dyn TerminalWriter>,
    engine: Rc<dyn CompletionEngine>,
    reaper: Rc<dyn GeneratorProcessReaper>,
    modes: Rc<dyn TerminalModes>,
    session_contexts: Rc<dyn SessionContextRepository>,
}

impl EndSessionUseCase {
    pub(crate) fn new(
        screen: Rc<dyn TerminalScreen>,
        writer: Rc<dyn TerminalWriter>,
        engine: Rc<dyn CompletionEngine>,
        reaper: Rc<dyn GeneratorProcessReaper>,
        modes: Rc<dyn TerminalModes>,
        session_contexts: Rc<dyn SessionContextRepository>,
    ) -> Self {
        EndSessionUseCase { screen, writer, engine, reaper, modes, session_contexts }
    }

    /// One write, then the engine, the generator processes, the terminal modes and the context file, in that order.
    pub(crate) fn execute(&self, session: &TerminalSession) {
        let mut out = self.screen.take_clear();
        out.extend_from_slice(b"\x1b[0m\x1b[?25h");
        self.writer.write(&out);
        self.engine.shut_down();
        self.reaper.kill_live_children(); // Windows has no atexit hook for running generators
        self.modes.restore();
        self.session_contexts.remove(session.id());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::completion::application::completion_request::CompletionRequest;
    use crate::history::domain::entities::history_entry::HistoryEntry;
    use crate::settings::domain::value_objects::settings::Settings;
    use crate::shared::domain::value_objects::shell::Shell;
    use crate::terminal_session::application::test_fixtures::{subs, Io, TerminalSessionHarness};
    use crate::terminal_session::domain::entities::session_context::SessionContext;
    use crate::terminal_session::domain::repositories::session_context_repository::InMemorySessionContextRepository;
    use crate::terminal_session::domain::terminal_session_error::TerminalSessionError;
    use crate::terminal_session::domain::value_objects::session_id::SessionId;
    use std::cell::RefCell;
    use std::collections::HashMap;
    use std::path::PathBuf;
    use std::time::{Duration, SystemTime};

    const SHOW_CURSOR: &[u8] = b"\x1b[0m\x1b[?25h";

    /// Test double for every port the use case ends: writes down which one was called, in the order of the calls.
    #[derive(Default)]
    struct Ledger {
        entries: RefCell<Vec<&'static str>>,
    }

    impl Ledger {
        fn note(&self, entry: &'static str) {
            self.entries.borrow_mut().push(entry);
        }
    }

    impl TerminalWriter for Ledger {
        fn write(&self, _bytes: &[u8]) {
            self.note("write");
        }
    }

    impl CompletionEngine for Ledger {
        fn complete(&self, _request: CompletionRequest) {}
        fn command_finished(&self, _entry: HistoryEntry) {}
        fn command_ended(&self) {}
        fn record_insertion(&self, _command: String, _name: String) {}
        fn request_llm_ghost_text(&self, _request: u64, _buffer: String, _cwd: PathBuf, _shell: Shell) {}
        fn apply_settings(&self, _settings: Settings) {}
        fn apply_session_context(&self, _env: HashMap<String, String>, _alias: Option<String>) {}
        fn toggle_fuzzy_search(&self) {}
        fn shut_down(&self) {
            self.note("shut_down");
        }
    }

    impl GeneratorProcessReaper for Ledger {
        fn kill_live_children(&self) {
            self.note("kill_live_children");
        }
    }

    impl TerminalModes for Ledger {
        fn restore(&self) {
            self.note("restore");
        }
    }

    impl SessionContextRepository for Ledger {
        fn save(&self, _context: &SessionContext) -> Result<(), TerminalSessionError> {
            Ok(())
        }
        fn find(&self, _id: &SessionId) -> Option<SessionContext> {
            None
        }
        fn last_modified(&self, _id: &SessionId) -> Option<SystemTime> {
            None
        }
        fn remove(&self, _id: &SessionId) {
            self.note("remove");
        }
        fn prune_older_than(&self, _max_age: Duration) {}
    }

    fn harness_showing_a_menu() -> TerminalSessionHarness {
        let mut harness = TerminalSessionHarness::new("{}");
        harness.prompt("git ch");
        harness.reply(subs("ch"), None);
        harness
    }

    #[test]
    fn should_clear_the_overlay_and_show_the_cursor_in_one_write() {
        let mut harness = harness_showing_a_menu();
        let reference = harness_showing_a_menu();
        let mut expected = reference.screen.take_clear();
        expected.extend_from_slice(SHOW_CURSOR);
        let events_before = harness.journal.events().len();

        harness.end_session();

        let writes: Vec<Io> = harness.journal.events().split_off(events_before);
        assert_eq!((expected.len() > SHOW_CURSOR.len(), writes), (true, vec![Io::Stdout(expected)]));
    }

    #[test]
    fn should_shut_the_engine_down_then_reap_the_generators_then_restore_the_modes_then_remove_the_context() {
        let harness = TerminalSessionHarness::new("{}");
        let ledger = Rc::new(Ledger::default());
        let use_case = EndSessionUseCase::new(
            harness.screen.clone(),
            ledger.clone(),
            ledger.clone(),
            ledger.clone(),
            ledger.clone(),
            ledger.clone(),
        );

        use_case.execute(&harness.session);

        assert_eq!(
            *ledger.entries.borrow(),
            ["write", "shut_down", "kill_live_children", "restore", "remove"]
        );
    }

    #[test]
    fn should_remove_the_context_of_the_session_only() {
        let contexts = Rc::new(InMemorySessionContextRepository::default());
        let ended = SessionId::new("ended".to_string()).unwrap();
        let other = SessionId::new("other".to_string()).unwrap();
        contexts.save(&SessionContext::new(ended.clone(), HashMap::new(), None, 0)).unwrap();
        contexts.save(&SessionContext::new(other.clone(), HashMap::new(), None, 0)).unwrap();
        let harness = TerminalSessionHarness::with_session_contexts(ended.clone(), contexts.clone());

        harness.end_session();

        assert_eq!((contexts.find(&ended).is_none(), contexts.find(&other).is_some()), (true, true));
    }
}
