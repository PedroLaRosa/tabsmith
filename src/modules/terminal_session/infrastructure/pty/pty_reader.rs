use crate::modules::terminal_session::infrastructure::pty::session_message::SessionMessage;
use std::io::Read;
use std::sync::mpsc::{self, Receiver, Sender, SyncSender};
use std::thread;

/// PTY chunks in flight between the reader thread and the main loop (backpressure for a flooding child).
pub(crate) const PTY_CREDITS: usize = 4;

/// PTY reader (SPEC §4.3). Every read takes a credit, returned once the main loop handled the chunk, so at most
/// `PTY_CREDITS` chunks are queued: a flooding child feels backpressure and Ctrl-C is never stuck behind its output.
fn pump(mut reader: impl Read, credits: Receiver<()>, tx: Sender<SessionMessage>) {
    let mut buf = vec![0u8; 16 * 1024];
    while credits.recv().is_ok() {
        let n = loop {
            match reader.read(&mut buf) {
                Err(e) if e.kind() == std::io::ErrorKind::Interrupted => {}
                r => break r.unwrap_or(0), // Linux: EIO once the slave side is gone
            }
        };
        if n == 0 || tx.send(SessionMessage::Pty(buf[..n].to_vec())).is_err() {
            break;
        }
    }
    let _ = tx.send(SessionMessage::PtyEof);
}

/// Starts the reader thread and returns the credit sender: `PTY_CREDITS` credits are handed out at once, the main
/// loop gives one back per handled chunk.
pub(crate) fn spawn_pty_reader(reader: impl Read + Send + 'static, replies: Sender<SessionMessage>) -> SyncSender<()> {
    let (credits, credit_rx) = mpsc::sync_channel(PTY_CREDITS);
    for _ in 0..PTY_CREDITS {
        let _ = credits.send(());
    }
    thread::spawn(move || pump(reader, credit_rx, replies));
    credits
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc::RecvTimeoutError;
    use std::time::Duration;

    #[test]
    fn pty_reader_waits_for_credits() {
        let (credits, credit_rx) = mpsc::sync_channel(PTY_CREDITS);
        let (tx, rx) = mpsc::channel();
        credits.send(()).unwrap();
        credits.send(()).unwrap();
        std::thread::spawn(move || pump(std::io::repeat(b'y'), credit_rx, tx));
        let pty =
            |r: Result<SessionMessage, RecvTimeoutError>| matches!(r, Ok(SessionMessage::Pty(b)) if b.len() == 16 * 1024);
        assert!(pty(rx.recv_timeout(Duration::from_secs(5))));
        assert!(pty(rx.recv_timeout(Duration::from_secs(5))));
        assert!(
            rx.recv_timeout(Duration::from_millis(100)).is_err(),
            "an endless child is not read ahead"
        );
        credits.send(()).unwrap();
        assert!(pty(rx.recv_timeout(Duration::from_secs(5))));
        drop(credits);
        assert!(matches!(
            rx.recv_timeout(Duration::from_secs(5)),
            Ok(SessionMessage::PtyEof)
        ));
    }
}
