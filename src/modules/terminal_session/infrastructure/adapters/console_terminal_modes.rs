use crate::modules::terminal_session::application::ports::terminal_modes::TerminalModes;
#[cfg(unix)]
use crate::modules::terminal_session::infrastructure::pty::platform_unix as platform;
#[cfg(windows)]
use crate::modules::terminal_session::infrastructure::pty::platform_windows as platform;

/// The console the session put into raw mode, given back as the platform saved it.
pub(crate) struct ConsoleTerminalModes {
    saved: platform::Modes,
}

impl ConsoleTerminalModes {
    pub(crate) fn new(saved: platform::Modes) -> Self {
        ConsoleTerminalModes { saved }
    }
}

impl TerminalModes for ConsoleTerminalModes {
    fn restore(&self) {
        self.saved.restore();
    }
}
