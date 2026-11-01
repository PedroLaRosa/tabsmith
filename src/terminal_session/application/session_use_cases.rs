use crate::terminal_session::application::overlay_painter::OverlayPainter;
use crate::terminal_session::application::session_ports_parts::SessionPortsParts;
use std::rc::Rc;

/// Everything a session does, wired once over the ports it runs on. Production and the test harness build it the same
/// way, so a wiring mistake shows in a use-case test.
pub(crate) struct SessionUseCases {
    painter: Rc<OverlayPainter>,
}

impl SessionUseCases {
    pub(crate) fn new(ports: SessionPortsParts) -> Self {
        let painter = Rc::new(OverlayPainter::new(ports.screen, ports.writer, ports.clock));
        SessionUseCases { painter }
    }

    // interim: dies with 9.5g (the legacy `Session::frame`, `redraw` and `take_repaint` still draw through it)
    pub(crate) fn painter(&self) -> &OverlayPainter {
        &self.painter
    }
}
