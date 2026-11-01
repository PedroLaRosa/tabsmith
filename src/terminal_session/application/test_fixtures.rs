#[cfg(test)]
use crate::completion::domain::test_fixtures::settings;
#[cfg(test)]
use crate::dropdown::domain::repositories::theme_repository::InMemoryThemeRepository;
#[cfg(test)]
use crate::shared::application::ports::clock::Clock;
#[cfg(test)]
use crate::shared::domain::value_objects::shell::Shell;
#[cfg(test)]
use crate::shared::infrastructure::system_clock::SystemClock;
#[cfg(test)]
use crate::terminal_session::application::ports::completion_engine::{CompletionEngineCall, RecordingCompletionEngine};
#[cfg(test)]
use crate::terminal_session::application::ports::generator_process_reaper::RecordingGeneratorProcessReaper;
#[cfg(test)]
use crate::terminal_session::application::ports::live_settings::InMemoryLiveSettings;
#[cfg(test)]
use crate::terminal_session::application::ports::terminal_modes::RecordingTerminalModes;
#[cfg(test)]
use crate::terminal_session::application::ports::terminal_writer::CapturingTerminalWriter;
#[cfg(test)]
use crate::terminal_session::application::ports::working_directory::FixedWorkingDirectory;
#[cfg(test)]
use crate::terminal_session::application::ports::wrapped_shell::CapturingWrappedShell;
#[cfg(test)]
use crate::terminal_session::application::session_ports_parts::SessionPortsParts;
#[cfg(test)]
use crate::terminal_session::application::session_use_cases::SessionUseCases;
#[cfg(test)]
use crate::terminal_session::domain::entities::marker_tracker::MarkerTracker;
#[cfg(test)]
use crate::terminal_session::domain::entities::terminal_session::{TerminalSession, TerminalSessionParts};
#[cfg(test)]
use crate::terminal_session::domain::repositories::session_context_repository::{
    InMemorySessionContextRepository, SessionContextRepository,
};
#[cfg(test)]
use crate::terminal_session::domain::value_objects::session_id::SessionId;
#[cfg(test)]
use crate::terminal_session::infrastructure::adapters::alacritty_terminal_screen::AlacrittyTerminalScreen;
use std::cell::RefCell;
#[cfg(test)]
use std::rc::Rc;
#[cfg(test)]
use std::sync::mpsc::{self, Receiver};
#[cfg(test)]
use std::sync::Arc;

/// The id of the session of the tests; the shell integration markers of a test carry it.
pub(crate) const SID: &str = "0123456789abcdef0123456789abcdef";

/// One thing the session sent out, in the order the doubles saw it.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Io {
    Stdout(Vec<u8>),
    ShellInput(Vec<u8>),
}

/// Test support: the order in which the user's terminal and the shell were written, across both doubles.
pub(crate) struct IoJournal {
    events: RefCell<Vec<Io>>,
}

impl IoJournal {
    pub(crate) fn new() -> Self {
        IoJournal { events: RefCell::new(Vec::new()) }
    }

    pub(crate) fn note(&self, event: Io) {
        self.events.borrow_mut().push(event);
    }

    pub(crate) fn events(&self) -> Vec<Io> {
        self.events.borrow().clone()
    }
}

/// A session over the real screen and in-memory doubles, wired through `SessionUseCases::new` exactly as production
/// wires it. The writer and the shell share one journal, so a test sees the order of what went out.
pub(crate) struct TerminalSessionHarness {
    pub(crate) session: TerminalSession,
    /// The environment of the tests: 80 x 24, the dark theme, no truecolor, no images.
    pub(crate) screen: Rc<AlacrittyTerminalScreen>,
    pub(crate) writer: Rc<CapturingTerminalWriter>,
    pub(crate) shell: Rc<CapturingWrappedShell>,
    pub(crate) journal: Rc<IoJournal>,
    pub(crate) engine_calls: Receiver<CompletionEngineCall>,
    pub(crate) live_settings: Rc<InMemoryLiveSettings>,
    pub(crate) reaper: Rc<RecordingGeneratorProcessReaper>,
    pub(crate) clock: Arc<dyn Clock + Send + Sync>,
    pub(crate) use_cases: SessionUseCases,
}

impl TerminalSessionHarness {
    /// On the real clock.
    pub(crate) fn new(settings_json: &str) -> Self {
        Self::with_clock(settings_json, Arc::new(SystemClock))
    }

    /// A session of `SID` at a bash prompt that was launched as bash, with the settings `settings_json`.
    pub(crate) fn with_clock(settings_json: &str, clock: Arc<dyn Clock + Send + Sync>) -> Self {
        let id = SessionId::new(SID.to_string()).unwrap();
        Self::assemble(settings_json, clock, id, Rc::new(InMemorySessionContextRepository::default()))
    }

    /// A session called `id` that finds its context in `session_contexts`; its shell integration markers still
    /// carry `SID`.
    pub(crate) fn with_session_contexts(id: SessionId, session_contexts: Rc<dyn SessionContextRepository>) -> Self {
        Self::assemble("{}", Arc::new(SystemClock), id, session_contexts)
    }

    fn assemble(
        settings_json: &str,
        clock: Arc<dyn Clock + Send + Sync>,
        id: SessionId,
        session_contexts: Rc<dyn SessionContextRepository>,
    ) -> Self {
        let settings = settings(settings_json);
        let journal = Rc::new(IoJournal::new());
        let screen = Rc::new(AlacrittyTerminalScreen::for_tests(80, 24));
        let writer = Rc::new(CapturingTerminalWriter::journaled(journal.clone()));
        let shell = Rc::new(CapturingWrappedShell::journaled(journal.clone()));
        let (calls, engine_calls) = mpsc::channel();
        let live_settings = Rc::new(InMemoryLiveSettings::new(settings.clone()));
        let reaper = Rc::new(RecordingGeneratorProcessReaper::new());
        let use_cases = SessionUseCases::new(SessionPortsParts {
            engine: Rc::new(RecordingCompletionEngine::new(calls)),
            screen: screen.clone(),
            writer: writer.clone(),
            shell: shell.clone(),
            reaper: reaper.clone(),
            modes: Rc::new(RecordingTerminalModes::new()),
            live_settings: live_settings.clone(),
            session_contexts,
            themes: Rc::new(InMemoryThemeRepository::default()),
            working_directory: Rc::new(FixedWorkingDirectory::new(std::env::current_dir().ok())),
            clock: clock.clone(),
        });
        let session = TerminalSession::new(TerminalSessionParts {
            id,
            launched_shell: Shell::Bash,
            settings,
            tracker: MarkerTracker::new(SessionId::new(SID.to_string()).unwrap(), Some(Shell::Bash), false),
            terminal_background: None,
            settings_modified: None,
            now: clock.now_instant(),
        });
        TerminalSessionHarness { session, screen, writer, shell, journal, engine_calls, live_settings, reaper, clock, use_cases }
    }

    /// State-only change: draw the frame again.
    pub(crate) fn redraw(&mut self) {
        self.use_cases.painter().redraw(&self.session);
    }

    /// What the user's terminal got since the last `clear_written`.
    pub(crate) fn written(&self) -> Vec<u8> {
        self.writer.written()
    }

    pub(crate) fn clear_written(&self) {
        self.writer.clear();
    }
}
