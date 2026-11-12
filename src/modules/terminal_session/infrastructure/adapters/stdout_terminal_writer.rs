use crate::modules::terminal_session::application::ports::terminal_writer::TerminalWriter;
use std::io::Write;

/// The user's terminal is this process's stdout.
pub(crate) struct StdoutTerminalWriter;

impl TerminalWriter for StdoutTerminalWriter {
    fn write(&self, b: &[u8]) {
        if b.is_empty() {
            return;
        }
        let mut o = std::io::stdout().lock();
        let _ = o.write_all(b);
        let _ = o.flush();
    }
}
