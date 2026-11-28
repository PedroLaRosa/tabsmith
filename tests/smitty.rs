//! Smitty, the tabsmith mascot, signs off `setup` and `uninstall` when they run on a terminal.
#![cfg(unix)]

use portable_pty::{native_pty_system, CommandBuilder, PtySize};
use std::io::Read;
use std::path::PathBuf;
use std::sync::mpsc;
use std::time::{Duration, Instant};

/// A fresh, empty home for tabsmith to set itself up in; removed on drop.
struct ThrowawayHome(PathBuf);

impl ThrowawayHome {
    fn new(tag: &str) -> Self {
        let home = std::env::temp_dir().join(format!("tabsmith-smitty-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&home);
        std::fs::create_dir_all(&home).unwrap();
        ThrowawayHome(home)
    }

    /// Runs the tabsmith under test with `args` on a terminal of its own and gives back everything it printed.
    fn run_on_terminal(&self, args: &[&str]) -> String {
        let pair = native_pty_system()
            .openpty(PtySize {
                rows: 40,
                cols: 120,
                pixel_width: 0,
                pixel_height: 0,
            })
            .unwrap();
        let mut command = CommandBuilder::new(env!("CARGO_BIN_EXE_tabsmith"));
        command.args(args);
        command.env_clear();
        command.env("PATH", std::env::var_os("PATH").unwrap_or_default());
        command.env("TERM", "xterm-256color");
        command.env("HOME", &self.0);
        command.env("TABSMITH_HOME", self.0.join(".tabsmith"));
        let mut child = pair.slave.spawn_command(command).unwrap();
        drop(pair.slave);
        let mut reader = pair.master.try_clone_reader().unwrap();
        let (chunks, received) = mpsc::channel::<Vec<u8>>();
        std::thread::spawn(move || {
            let mut buffer = [0u8; 4096];
            while let Ok(read @ 1..) = reader.read(&mut buffer) {
                if chunks.send(buffer[..read].to_vec()).is_err() {
                    break;
                }
            }
        });
        let deadline = Instant::now() + Duration::from_secs(40);
        let mut printed = Vec::new();
        while child.try_wait().unwrap().is_none() {
            assert!(Instant::now() < deadline, "tabsmith {args:?} did not exit");
            if let Ok(chunk) = received.recv_timeout(Duration::from_millis(50)) {
                printed.extend(chunk);
            }
        }
        std::thread::sleep(Duration::from_millis(100));
        printed.extend(received.try_iter().flatten());
        String::from_utf8_lossy(&printed).into_owned()
    }
}

impl Drop for ThrowawayHome {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[test]
fn should_end_setup_with_smitty_saying_hi() {
    let home = ThrowawayHome::new("setup");

    let printed = home.run_on_terminal(&["setup", "--no-confirm", "--force"]);

    assert!(printed.contains("Hi, I'm Smitty!"), "{printed}");
}

#[test]
fn should_end_uninstall_with_smitty_waving_goodbye() {
    let home = ThrowawayHome::new("uninstall");
    home.run_on_terminal(&["setup", "--no-confirm", "--force"]);

    let printed = home.run_on_terminal(&["uninstall", "--no-confirm"]);

    assert!(printed.contains("Bye for now!"), "{printed}");
}
