use crate::modules::dropdown::domain::repositories::theme_repository::ThemeRepository;
use crate::modules::shared::application::ports::clock::Clock;
use crate::modules::terminal_session::application::ports::completion_engine::CompletionEngine;
use crate::modules::terminal_session::application::ports::generator_process_reaper::GeneratorProcessReaper;
use crate::modules::terminal_session::application::ports::live_settings::LiveSettings;
use crate::modules::terminal_session::application::ports::terminal_modes::TerminalModes;
use crate::modules::terminal_session::application::ports::terminal_screen::TerminalScreen;
use crate::modules::terminal_session::application::ports::terminal_writer::TerminalWriter;
use crate::modules::terminal_session::application::ports::working_directory::WorkingDirectory;
use crate::modules::terminal_session::application::ports::wrapped_shell::WrappedShell;
use crate::modules::terminal_session::domain::repositories::session_context_repository::SessionContextRepository;
use std::rc::Rc;
use std::sync::Arc;

/// The runtime ports one session runs on, handed to the constructor of `SessionUseCases`: the driver fills them
/// from the module and the per-session adapters, the test harness from its doubles.
#[cfg_attr(test, derive(Clone))]
pub(crate) struct SessionPortsParts {
    pub(crate) engine: Rc<dyn CompletionEngine>,
    pub(crate) screen: Rc<dyn TerminalScreen>,
    pub(crate) writer: Rc<dyn TerminalWriter>,
    pub(crate) shell: Rc<dyn WrappedShell>,
    pub(crate) reaper: Rc<dyn GeneratorProcessReaper>,
    pub(crate) modes: Rc<dyn TerminalModes>,
    pub(crate) live_settings: Rc<dyn LiveSettings>,
    pub(crate) session_contexts: Rc<dyn SessionContextRepository>,
    pub(crate) themes: Rc<dyn ThemeRepository>,
    pub(crate) working_directory: Rc<dyn WorkingDirectory>,
    pub(crate) clock: Arc<dyn Clock + Send + Sync>,
}
