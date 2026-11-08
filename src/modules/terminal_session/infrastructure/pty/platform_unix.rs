use crate::modules::terminal_session::infrastructure::pty::session_message::SessionMessage;
use anyhow::Result;
use std::io::Read;
use std::sync::mpsc::Sender;
use std::time::Duration;

#[derive(Clone, Copy, Default)]
pub(crate) struct Modes;

impl Modes {
    pub(crate) fn restore(&self) {
        let _ = crossterm::terminal::disable_raw_mode();
    }
}

/// Raw mode (cfmakeraw, TCSANOW: ISIG off so Ctrl-C/Z become bytes). Bare LF never needs rewriting here.
pub(crate) fn enter() -> Result<(Modes, bool)> {
    crossterm::terminal::enable_raw_mode()?;
    Ok((Modes, false))
}

/// One read from stdin, waiting at most `t` (startup probe). `select`, since macOS `poll` rejects devices.
pub(crate) fn read_timeout(t: Duration) -> Vec<u8> {
    let mut buf = [0u8; 1024];
    // SAFETY: plain libc calls on fd 0 with stack-owned buffers.
    let n = unsafe {
        let mut set: libc::fd_set = std::mem::zeroed();
        libc::FD_ZERO(&mut set);
        libc::FD_SET(0, &mut set);
        let us = t.as_micros();
        let mut tv = libc::timeval {
            tv_sec: (us / 1_000_000) as _,
            tv_usec: (us % 1_000_000) as _,
        };
        if libc::select(
            1,
            &mut set,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            &mut tv,
        ) <= 0
        {
            return Vec::new();
        }
        libc::read(0, buf.as_mut_ptr().cast(), buf.len())
    };
    if n > 0 {
        buf[..n as usize].to_vec()
    } else {
        Vec::new()
    }
}

pub(crate) fn spawn_input(tx: Sender<SessionMessage>) {
    std::thread::spawn(move || {
        let mut buf = [0u8; 4096];
        let mut stdin = std::io::stdin();
        loop {
            match stdin.read(&mut buf) {
                Ok(0) => break,
                Ok(n) => {
                    if tx.send(SessionMessage::Stdin(buf[..n].to_vec())).is_err() {
                        break;
                    }
                }
                Err(e) if e.kind() == std::io::ErrorKind::Interrupted => {}
                Err(_) => break,
            }
        }
    });
}

/// The child's exit code as a shell reports it: 128 + signal when killed (portable-pty maps that to 1).
pub(crate) fn wait(child: &mut dyn portable_pty::Child) -> i32 {
    match child.process_id() {
        Some(pid) => wait_pid(pid as libc::pid_t),
        None => child.wait().map_or(1, |s| s.exit_code() as i32),
    }
}

pub(crate) fn wait_pid(pid: libc::pid_t) -> i32 {
    let mut st = 0;
    // SAFETY: waitpid on our own child with a stack-owned status.
    while unsafe { libc::waitpid(pid, &mut st, 0) } != pid {
        if std::io::Error::last_os_error().kind() != std::io::ErrorKind::Interrupted {
            return 1;
        }
    }
    if libc::WIFSIGNALED(st) {
        128 + libc::WTERMSIG(st)
    } else {
        libc::WEXITSTATUS(st)
    }
}

/// SIGWINCH → Resize, SIGTERM/SIGHUP → Term.
pub(crate) fn spawn_resize(tx: Sender<SessionMessage>) -> Result<()> {
    use signal_hook::consts::signal::{SIGHUP, SIGTERM, SIGWINCH};
    let mut signals = signal_hook::iterator::Signals::new([SIGWINCH, SIGTERM, SIGHUP])?;
    std::thread::spawn(move || {
        for sig in signals.forever() {
            let msg = match sig {
                SIGWINCH => match crossterm::terminal::size() {
                    Ok((c, r)) => SessionMessage::Resize(c, r),
                    Err(_) => continue,
                },
                _ => SessionMessage::Term,
            };
            if tx.send(msg).is_err() {
                break;
            }
        }
    });
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn signal_death_exit_code() {
        let sh = |s: &str| {
            std::process::Command::new("/bin/sh")
                .args(["-c", s])
                .spawn()
                .unwrap()
                .id() as libc::pid_t
        };
        assert_eq!(wait_pid(sh("kill -9 $$")), 137);
        assert_eq!(wait_pid(sh("exit 3")), 3);
    }
}
