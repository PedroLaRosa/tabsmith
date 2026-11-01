use crate::terminal_session::application::keystroke_handler::KeystrokeHandler;
use crate::terminal_session::domain::entities::terminal_session::TerminalSession;
use std::rc::Rc;

/// A stdin read arrives (SPEC §7.2.4).
pub(crate) struct HandleKeystrokesUseCase {
    handler: Rc<KeystrokeHandler>,
}

impl HandleKeystrokesUseCase {
    pub(crate) fn new(handler: Rc<KeystrokeHandler>) -> Self {
        HandleKeystrokesUseCase { handler }
    }

    pub(crate) fn execute(&self, session: &mut TerminalSession, chunk: &[u8]) {
        self.handler.handle(session, chunk);
    }
}

#[cfg(test)]
mod tests;
