use crate::settings::domain::value_objects::settings::Settings;
use crate::terminal_session::application::ports::completion_engine::CompletionEngine;
use crate::terminal_session::infrastructure::pty::session_message::SessionMessage;
use std::rc::Rc;
use std::sync::mpsc::Sender;

/// Starts the completion engine, its replies going to the session's inbox; called once, inside `Session::start`.
pub(crate) type CompletionEngineStarter = fn(Sender<SessionMessage>, Settings) -> Rc<dyn CompletionEngine>;
