#[cfg(test)]
use crate::completion::application::completion_request::CompletionRequest;
#[cfg(test)]
use crate::completion::domain::test_fixtures::{comp, item, settings};
#[cfg(test)]
use crate::completion::domain::value_objects::completion_result::CompletionResult;
#[cfg(test)]
use crate::completion::domain::value_objects::suggestion::Suggestion;
#[cfg(test)]
use crate::dropdown::domain::repositories::theme_repository::InMemoryThemeRepository;
#[cfg(test)]
use crate::history::domain::entities::history_entry::HistoryEntry;
#[cfg(test)]
use crate::settings::domain::value_objects::settings::Settings;
#[cfg(test)]
use crate::shared::application::ports::clock::Clock;
#[cfg(test)]
use crate::shared::domain::value_objects::shell::Shell;
#[cfg(test)]
use crate::shared::infrastructure::system_clock::SystemClock;
#[cfg(test)]
use crate::specs::domain::value_objects::suggestion_kind::SuggestionKind;
#[cfg(test)]
use crate::terminal_session::application::ports::completion_engine::{
    CompletionEngine, CompletionEngineCall, RecordingCompletionEngine,
};
#[cfg(test)]
use crate::terminal_session::application::ports::generator_process_reaper::RecordingGeneratorProcessReaper;
#[cfg(test)]
use crate::terminal_session::application::ports::live_settings::InMemoryLiveSettings;
#[cfg(test)]
use crate::terminal_session::application::ports::terminal_modes::RecordingTerminalModes;
#[cfg(test)]
use crate::terminal_session::application::ports::terminal_screen::TerminalScreen;
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
use crate::terminal_session::application::shell_end::ShellEnd;
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
use std::cell::{Cell, RefCell};
#[cfg(test)]
use std::collections::HashMap;
#[cfg(test)]
use std::path::PathBuf;
#[cfg(test)]
use std::rc::Rc;
#[cfg(test)]
use std::sync::mpsc::{self, Receiver};
#[cfg(test)]
use std::sync::Arc;

/// The id of the session of the tests; the shell integration markers of a test carry it.
pub(crate) const SID: &str = "0123456789abcdef0123456789abcdef";

thread_local!(static PANIC_IN_KEY_HANDLING: Cell<bool> = const { Cell::new(false) });

/// Fault injection: a panic while keys are handled (decoder, menu, insertion). The handler calls it first in the loop
/// over the keys of a read.
pub(crate) fn maybe_panic() {
    if PANIC_IN_KEY_HANDLING.with(|on| on.get()) {
        panic!("injected");
    }
}

/// Switches the fault injection of `maybe_panic` on or off for the current thread.
pub(crate) fn set_panic_in_key_handling(on: bool) {
    PANIC_IN_KEY_HANDLING.with(|switch| switch.set(on));
}

/// The two subcommands the tests complete `query` to.
pub(crate) fn subs(query: &str) -> Vec<Suggestion> {
    ["cherry-pick", "checkout"]
        .iter()
        .map(|name| item(name, SuggestionKind::Subcommand, query))
        .collect()
}

/// The shell line after `line` receives what the wrapper wrote (DEL erases, printable bytes append).
pub(crate) fn shell_line(line: &str, bytes: &[u8]) -> String {
    let mut shown = line.to_string();
    for &byte in bytes {
        match byte {
            0x7f => drop(shown.pop()),
            0x20..=0x7e => shown.push(byte as char),
            _ => {}
        }
    }
    shown
}

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
        Self::assemble(settings_json, clock, id, Rc::new(InMemorySessionContextRepository::default()), None)
    }

    /// The session of `new`, asking `engine` instead of the recording one; `engine_calls` stays empty.
    pub(crate) fn with_engine(settings_json: &str, engine: Rc<dyn CompletionEngine>) -> Self {
        let id = SessionId::new(SID.to_string()).unwrap();
        let contexts = Rc::new(InMemorySessionContextRepository::default());
        Self::assemble(settings_json, Arc::new(SystemClock), id, contexts, Some(engine))
    }

    /// A session called `id` that finds its context in `session_contexts`; its shell integration markers still
    /// carry `SID`.
    pub(crate) fn with_session_contexts(id: SessionId, session_contexts: Rc<dyn SessionContextRepository>) -> Self {
        Self::assemble("{}", Arc::new(SystemClock), id, session_contexts, None)
    }

    fn assemble(
        settings_json: &str,
        clock: Arc<dyn Clock + Send + Sync>,
        id: SessionId,
        session_contexts: Rc<dyn SessionContextRepository>,
        engine: Option<Rc<dyn CompletionEngine>>,
    ) -> Self {
        let settings = settings(settings_json);
        let journal = Rc::new(IoJournal::new());
        let screen = Rc::new(AlacrittyTerminalScreen::for_tests(80, 24));
        let writer = Rc::new(CapturingTerminalWriter::journaled(journal.clone()));
        let shell = Rc::new(CapturingWrappedShell::journaled(journal.clone()));
        let (calls, engine_calls) = mpsc::channel();
        let engine = engine.unwrap_or_else(|| Rc::new(RecordingCompletionEngine::new(calls)));
        let live_settings = Rc::new(InMemoryLiveSettings::new(settings.clone()));
        let reaper = Rc::new(RecordingGeneratorProcessReaper::new());
        let use_cases = SessionUseCases::new(SessionPortsParts {
            engine,
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

    /// A stdin read arrives.
    pub(crate) fn input(&mut self, bytes: &[u8]) {
        self.use_cases.handle_keystrokes.execute(&mut self.session, bytes);
    }

    /// A batch of the shell's output arrives.
    pub(crate) fn output(&mut self, batch: &[u8]) {
        self.use_cases.process_shell_output.execute(&mut self.session, batch);
    }

    /// The user's terminal is resized to `columns` x `rows`.
    pub(crate) fn resize(&mut self, columns: u16, rows: u16) {
        self.use_cases.resize_terminal.execute(&mut self.session, columns, rows);
    }

    /// The shell's side of the session ends; true while the loop goes on.
    pub(crate) fn end(&mut self, end: ShellEnd) -> bool {
        self.use_cases.handle_shell_exit.execute(&mut self.session, end)
    }

    /// A bash prompt `$ ` with the integration markers, then `text` echoed.
    pub(crate) fn prompt(&mut self, text: &str) {
        self.output(format!("\x1b]697;StartPrompt\x07$ \x1b]697;EndPrompt\x07\x1b]697;NewCmd={SID}\x07{text}").as_bytes());
    }

    /// The shell redraws its prompt (SIGWINCH, `zle reset-prompt`): PS1 with NewCmd=<sid>, then `text`.
    pub(crate) fn redraw_prompt(&mut self, text: &str) {
        self.output(
            format!("\r\x1b[K\x1b]697;StartPrompt\x07$ \x1b]697;EndPrompt\x07\x1b]697;NewCmd={SID}\x07{text}").as_bytes(),
        );
    }

    /// The engine's reply arrives.
    pub(crate) fn complete(&mut self, result: CompletionResult) {
        self.use_cases.apply_completion_result.execute(&mut self.session, result);
    }

    /// Engine reply for the latest request (twice: the first one after a reset only arms the menu).
    pub(crate) fn reply(&mut self, items: Vec<Suggestion>, ghost: Option<&str>) {
        let (buffer, _) = self.session.sent().expect("a request was sent");
        let result = comp(buffer, 1, items)
            .with_request_id(self.session.request_id())
            .with_ghost(ghost.map(String::from));
        self.complete(result.clone());
        self.complete(result);
    }

    /// The LLM's ghost text for `request` arrives.
    pub(crate) fn llm_reply(&mut self, request: u64, text: Option<&str>) {
        self.use_cases.apply_llm_ghost_text.execute(&mut self.session, request, text.map(String::from));
    }

    /// State-only change: draw the frame again.
    pub(crate) fn redraw(&mut self) {
        self.use_cases.painter().redraw(&self.session);
    }

    /// What the shell was sent since the last call.
    pub(crate) fn sent(&self) -> Vec<u8> {
        self.shell.take_input()
    }

    /// The calls to the engine since the last call, in order.
    pub(crate) fn engine_calls(&self) -> Vec<CompletionEngineCall> {
        self.engine_calls.try_iter().collect()
    }

    /// No render guard holds, so a menu or ghost can be on screen and keys may act on it.
    pub(crate) fn drawable(&self) -> bool {
        self.session.drawable(&self.screen.state())
    }

    /// The ghost text: `typed` = after the keys typed ahead of their echo, else after the buffer on screen.
    pub(crate) fn ghost_text(&self, typed: bool) -> Option<String> {
        self.session.ghost_text(typed, &self.screen.state())
    }

    /// What the user's terminal got since the last `clear_written`.
    pub(crate) fn written(&self) -> Vec<u8> {
        self.writer.written()
    }

    pub(crate) fn clear_written(&self) {
        self.writer.clear();
    }
}

/// Test double: panics when it is asked for completions and ignores everything else.
#[cfg(test)]
pub(crate) struct PanickingCompletionEngine;

#[cfg(test)]
impl CompletionEngine for PanickingCompletionEngine {
    fn complete(&self, _request: CompletionRequest) {
        panic!("the engine was asked");
    }
    fn command_finished(&self, _entry: HistoryEntry) {}
    fn command_ended(&self) {}
    fn record_insertion(&self, _command: String, _name: String) {}
    fn request_llm_ghost_text(&self, _request: u64, _buffer: String, _cwd: PathBuf, _shell: Shell) {}
    fn apply_settings(&self, _settings: Settings) {}
    fn apply_session_context(&self, _env: HashMap<String, String>, _alias: Option<String>) {}
    fn toggle_fuzzy_search(&self) {}
    fn shut_down(&self) {}
}
