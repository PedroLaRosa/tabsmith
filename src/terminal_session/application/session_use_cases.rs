use crate::terminal_session::application::completion_requester::CompletionRequester;
use crate::terminal_session::application::overlay_painter::OverlayPainter;
use crate::terminal_session::application::session_ports_parts::SessionPortsParts;
use std::rc::Rc;

/// Everything a session does, wired once over the ports it runs on. Production and the test harness build it the same
/// way, so a wiring mistake shows in a use-case test.
pub(crate) struct SessionUseCases {
    painter: Rc<OverlayPainter>,
    requester: Rc<CompletionRequester>,
}

impl SessionUseCases {
    pub(crate) fn new(ports: SessionPortsParts) -> Self {
        let painter = Rc::new(OverlayPainter::new(ports.screen.clone(), ports.writer, ports.clock.clone()));
        let requester = Rc::new(CompletionRequester::new(
            ports.engine,
            ports.screen,
            ports.working_directory,
            ports.clock,
        ));
        SessionUseCases { painter, requester }
    }

    // interim: dies with 9.5g (the legacy `Session::frame`, `redraw` and `take_repaint` still draw through it)
    pub(crate) fn painter(&self) -> &OverlayPainter {
        &self.painter
    }

    // interim: dies with 9.5g (the legacy `Session::refresh` and `cwd` still ask through it)
    pub(crate) fn requester(&self) -> &CompletionRequester {
        &self.requester
    }
}
