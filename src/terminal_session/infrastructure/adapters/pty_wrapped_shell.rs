use crate::terminal_session::application::ports::wrapped_shell::WrappedShell;
use portable_pty::{MasterPty, PtySize};
use std::cell::RefCell;
use std::sync::mpsc::Sender;
use std::thread;
#[cfg(test)]
use std::sync::{Arc, Mutex};

/// The shell on the other end of the PTY: its input goes through the writer thread's channel, its size through the
/// master. Shared by the session and the use cases (`Rc`); the master sits in a `RefCell` that is only borrowed for
/// the length of one call, so a panic can never leave it borrowed.
pub(crate) struct PtyWrappedShell {
    input: Sender<Vec<u8>>,
    /// Dropped after the child exits on Windows to unblock the ConPTY reader.
    master: RefCell<Option<Box<dyn MasterPty + Send>>>,
}

impl PtyWrappedShell {
    pub(crate) fn new(input: Sender<Vec<u8>>, master: Box<dyn MasterPty + Send>) -> Self {
        PtyWrappedShell { input, master: RefCell::new(Some(master)) }
    }

    /// ConPTY does not EOF on child exit. Its close may wait until the final frame fits in the pipe, which the reader
    /// drains only with credits from the main loop: close it on a helper thread. Only the session runner calls this.
    pub(crate) fn release_pseudo_console(&self) {
        let released = self.master.borrow_mut().take_if(|_| cfg!(windows));
        if let Some(m) = released {
            thread::spawn(move || drop(m));
        }
    }
}

impl WrappedShell for PtyWrappedShell {
    fn write_input(&self, bytes: Vec<u8>) {
        let _ = self.input.send(bytes);
    }

    fn resize(&self, columns: u16, rows: u16) {
        let size = PtySize { rows, cols: columns, pixel_width: 0, pixel_height: 0 };
        let master = self.master.borrow();
        if let Some(m) = master.as_ref() {
            let _ = m.resize(size);
        }
    }
}

/// Test double for the PTY master: remembers every size it was resized to.
#[cfg(test)]
pub(crate) struct RecordingMasterPty {
    resizes: ResizeLog,
}

/// The sizes a [`RecordingMasterPty`] was resized to, readable after the master moved into a [`PtyWrappedShell`].
#[cfg(test)]
#[derive(Clone)]
pub(crate) struct ResizeLog(Arc<Mutex<Vec<PtySize>>>);

#[cfg(test)]
impl ResizeLog {
    pub(crate) fn all(&self) -> Vec<PtySize> {
        self.0.lock().unwrap().clone()
    }
}

#[cfg(test)]
impl RecordingMasterPty {
    pub(crate) fn new() -> Self {
        RecordingMasterPty { resizes: ResizeLog(Arc::new(Mutex::new(Vec::new()))) }
    }

    pub(crate) fn resizes(&self) -> ResizeLog {
        self.resizes.clone()
    }
}

#[cfg(test)]
impl MasterPty for RecordingMasterPty {
    fn resize(&self, size: PtySize) -> anyhow::Result<()> {
        self.resizes.0.lock().unwrap().push(size);
        Ok(())
    }

    fn get_size(&self) -> anyhow::Result<PtySize> {
        Ok(self.resizes.0.lock().unwrap().last().copied().unwrap_or_default())
    }

    fn try_clone_reader(&self) -> anyhow::Result<Box<dyn std::io::Read + Send>> {
        anyhow::bail!("unused")
    }

    fn take_writer(&self) -> anyhow::Result<Box<dyn std::io::Write + Send>> {
        anyhow::bail!("unused")
    }

    #[cfg(unix)]
    fn process_group_leader(&self) -> Option<libc::pid_t> {
        None
    }

    #[cfg(unix)]
    fn as_raw_fd(&self) -> Option<std::os::fd::RawFd> {
        None
    }

    #[cfg(unix)]
    fn tty_name(&self) -> Option<std::path::PathBuf> {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc;

    #[test]
    fn should_send_the_input_bytes_to_the_writer_in_order() {
        let (input, writer) = mpsc::channel();
        let shell = PtyWrappedShell::new(input, Box::new(RecordingMasterPty::new()));

        shell.write_input(b"ab".to_vec());
        shell.write_input(b"c".to_vec());

        assert_eq!(writer.try_iter().collect::<Vec<_>>(), vec![b"ab".to_vec(), b"c".to_vec()]);
    }

    #[test]
    fn should_ignore_input_once_the_writer_is_gone() {
        let (input, writer) = mpsc::channel();
        let master = RecordingMasterPty::new();
        let resizes = master.resizes();
        let shell = PtyWrappedShell::new(input, Box::new(master));
        drop(writer);

        shell.write_input(b"lost".to_vec());
        shell.resize(80, 24);

        assert_eq!(resizes.all().len(), 1);
    }

    #[test]
    fn should_resize_the_master_to_the_given_columns_and_rows() {
        let master = RecordingMasterPty::new();
        let resizes = master.resizes();
        let shell = PtyWrappedShell::new(mpsc::channel().0, Box::new(master));

        shell.resize(100, 30);

        assert_eq!(
            resizes.all(),
            vec![PtySize { rows: 30, cols: 100, pixel_width: 0, pixel_height: 0 }]
        );
    }

    #[test]
    fn should_keep_the_master_open_unless_on_windows() {
        let master = RecordingMasterPty::new();
        let resizes = master.resizes();
        let shell = PtyWrappedShell::new(mpsc::channel().0, Box::new(master));

        shell.release_pseudo_console();
        shell.resize(100, 30);

        assert_eq!(resizes.all().len(), if cfg!(windows) { 0 } else { 1 });
    }
}
