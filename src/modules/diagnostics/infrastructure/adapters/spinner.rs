use crate::modules::diagnostics::infrastructure::adapters::cursor_restorer::restore_cursor_on_ctrl_c;
use std::io::IsTerminal;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

/// `Running checks...` on stderr (Braille dots, 80 ms) with a hidden cursor; dropping it clears the line and
/// shows the cursor. Ctrl-C while doctor runs also restores the cursor (exit 1).
pub(crate) struct Spinner {
    stop: Arc<AtomicBool>,
    thread: Option<std::thread::JoinHandle<()>>,
}

impl Spinner {
    pub(crate) fn start() -> Option<Spinner> {
        if !std::io::stderr().is_terminal() {
            return None;
        }
        restore_cursor_on_ctrl_c();
        let stop = Arc::new(AtomicBool::new(false));
        let flag = stop.clone();
        let thread = std::thread::spawn(move || {
            for frame in "⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏".chars().cycle() {
                if flag.load(Ordering::Relaxed) {
                    break;
                }
                eprint!("\x1b[?25l\r{frame} Running checks...");
                std::thread::sleep(Duration::from_millis(80));
            }
        });
        Some(Spinner {
            stop,
            thread: Some(thread),
        })
    }
}

impl Drop for Spinner {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
        eprint!("\r\x1b[2K\x1b[?25h");
    }
}
