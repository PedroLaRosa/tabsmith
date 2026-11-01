use crate::shared::domain::services::shell_quoting;
use crate::shared::domain::value_objects::shell::Shell;
use crate::terminal_session::infrastructure::pty::pty_session::run_pty_session;
use crate::terminal_session::infrastructure::terminal_session_module::TerminalSessionModule;
use anyhow::{Context, Result};
use std::io::Write;
use std::path::Path;

/// `_ pty` with only the post integration loaded.
pub(crate) fn run_debug_shell_command(
    shell: Option<Shell>,
    module: &TerminalSessionModule,
    render_post_script: impl FnOnce(Shell) -> Result<String>,
) -> Result<i32> {
    let env_shell = std::env::var("SHELL").ok().filter(|s| !s.is_empty());
    let shell = match shell {
        Some(s) => s,
        None if cfg!(windows) => Shell::Pwsh,
        None => env_shell
            .as_deref()
            .and_then(Shell::from_name)
            .context("Could not determine current shell or shell not supported")?,
    };
    // The user's own binary when $SHELL is that shell, else the name (resolved on PATH by the wrapper).
    let exe = env_shell
        .filter(|s| Shell::from_name(s) == Some(shell))
        .unwrap_or_else(|| shell.name().to_string());
    let post = render_post_script(shell)? + "\n";
    let (dir, cmd) = debug_shell_files(&module.locate_data_root()?, shell, &exe, &post)?;
    if shell == Shell::Zsh {
        std::env::set_var("ZDOTDIR", &dir);
    }
    // The wrapper passes the current env to the child.
    std::env::set_var("TABSMITH_DEBUG_SHELL", "1");
    let _ = writeln!(std::io::stdout(), "Starting tabsmith debug shell\n");
    let code = run_pty_session(Some(exe), cmd, module);
    let _ = std::fs::remove_dir_all(&dir);
    // The terminal may be gone (window closed): no panic, the shell's code still counts.
    let _ = writeln!(std::io::stdout(), "\nEnding tabsmith debug shell\n");
    Ok(code)
}

/// Write the rc file `post` into a fresh private dir under `home` (the data dir, never a shared temp dir whose name
/// another user could claim first); returns the dir (zsh's ZDOTDIR) and the shell argv that loads the rc file.
fn debug_shell_files(
    home: &Path,
    shell: Shell,
    exe: &str,
    post: &str,
) -> Result<(std::path::PathBuf, Vec<String>)> {
    let dir = home
        .join("run")
        .join(format!("debug-shell-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir); // a stale dir of ours from a crashed run with the same pid
    let mut builder = std::fs::DirBuilder::new();
    builder.recursive(true);
    #[cfg(unix)]
    std::os::unix::fs::DirBuilderExt::mode(&mut builder, 0o700);
    builder.create(&dir)?;
    let rc = dir.join(match shell {
        Shell::Bash => "bashrc",
        Shell::Zsh => ".zshrc",
        Shell::Fish => "post.fish",
        Shell::Pwsh => "post.ps1",
    });
    std::fs::write(&rc, post)?;
    let rc = rc.display().to_string();
    let q = |s: &str| shell_quoting::quote(shell, s);
    let mut cmd = vec![exe.to_string()];
    match shell {
        Shell::Bash => cmd.extend(["--noprofile".into(), "--rcfile".into(), rc]),
        Shell::Zsh => {}
        // --no-config also skips fish's default theme: without an autosuggestion colour, tabsmith takes fish's
        // suggestion for typed text.
        Shell::Fish => cmd.extend([
            "--no-config".into(),
            "-C".into(),
            format!("set -q fish_color_autosuggestion[1]; or set -g fish_color_autosuggestion brblack; source {}", q(&rc)),
        ]),
        Shell::Pwsh => cmd.extend(["-NoProfile".into(), "-NoExit".into(), "-Command".into(), format!(". {}", q(&rc))]),
    }
    Ok((dir, cmd))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shared::infrastructure::testing::temp_dir::TempDir;

    /// The rc dir is private and fresh, and fish/pwsh load an rc path containing `'` (run when installed).
    #[test]
    fn debug_shell_rc_dir_and_quoting() {
        let temp_dir = TempDir::new("o'brien");
        let home = temp_dir.path().to_path_buf();
        let stale = home
            .join("run")
            .join(format!("debug-shell-{}", std::process::id()));
        std::fs::create_dir_all(&stale).unwrap();
        std::fs::write(stale.join("planted"), "").unwrap();
        let cases = [
            (Shell::Fish, "set -g __ts_m loaded\n", "; echo $__ts_m"),
            (Shell::Pwsh, "$__ts_m = 'loaded'\n", "; $__ts_m"),
        ];
        for (shell, post, show) in cases {
            let (dir, argv) = debug_shell_files(&home, shell, shell.name(), post).unwrap();
            assert_eq!(dir, stale);
            assert!(
                !dir.join("planted").exists(),
                "the dir is recreated, not reused"
            );
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                assert_eq!(
                    std::fs::metadata(&dir).unwrap().permissions().mode() & 0o777,
                    0o700
                );
            }
            if crate::shared::infrastructure::os::executable_lookup::which(shell.name()).is_none() {
                continue;
            }
            let load = format!("{}{show}", argv.last().unwrap());
            let args: Vec<&str> = match shell {
                Shell::Fish => vec!["--no-config", "-c", &load],
                _ => vec!["-NoProfile", "-Command", &load],
            };
            let out = std::process::Command::new(shell.name())
                .args(args)
                .output()
                .unwrap();
            let text = String::from_utf8_lossy(&out.stdout);
            assert!(
                text.trim() == "loaded",
                "{shell:?}: {text}{}",
                String::from_utf8_lossy(&out.stderr)
            );
            if shell == Shell::Fish {
                // --no-config skips the default theme: the debug shell still colours fish's autosuggestions.
                let load = format!("{}; echo $fish_color_autosuggestion", argv.last().unwrap());
                let out = std::process::Command::new("fish")
                    .args(["--no-config", "-c", &load])
                    .output()
                    .unwrap();
                assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), "brblack");
            }
        }
        let _ = std::fs::remove_dir_all(home);
    }

    /// Closing the terminal of a `debug shell` (its window goes away) ends it cleanly: printing the goodbye to the
    /// dead terminal does not panic, the shell's code is returned and the rc dir is removed. Re-runs this test in a
    /// child process on a PTY, then closes the PTY.
    #[cfg(unix)]
    #[test]
    fn debug_shell_survives_closed_terminal() {
        use portable_pty::{native_pty_system, CommandBuilder, PtySize};
        const NAME: &str = "terminal_session::infrastructure::cli::debug_shell_command::tests::debug_shell_survives_closed_terminal";
        if let Some(marker) = std::env::var_os("TABSMITH_TEST_DEBUG_SHELL") {
            let code = run_debug_shell_command(
                Some(Shell::Bash),
                &crate::bootstrap::cli::terminal_session_module(),
                crate::bootstrap::cli::debug_shell_post_script,
            );
            std::fs::write(marker, format!("{code:?}")).unwrap();
            return;
        }
        if crate::shared::infrastructure::os::executable_lookup::which("bash").is_none() {
            return;
        }
        let temp_dir = TempDir::new("dbgclose");
        let home = temp_dir.path().to_path_buf();
        let marker = home.join("marker");
        let pair = native_pty_system()
            .openpty(PtySize {
                rows: 24,
                cols: 80,
                pixel_width: 0,
                pixel_height: 0,
            })
            .unwrap();
        let mut cb = CommandBuilder::new(std::env::current_exe().unwrap());
        cb.args([NAME, "--exact", "--nocapture"]);
        cb.env_clear(); // no terminal integration (kitty, Ghostty, VS Code) of the terminal running the tests
        cb.env("PATH", std::env::var_os("PATH").unwrap_or_default());
        cb.env("TERM", "xterm-256color");
        cb.env("TABSMITH_TEST_DEBUG_SHELL", &marker);
        cb.env("TABSMITH_HOME", &home);
        cb.env("HOME", &home);
        cb.env("HISTFILE", "/dev/null");
        cb.env("TABSMITH_INIT_SNAPSHOT_TEST", "1"); // the post block calls `tabsmith`, not this test binary
        let mut child = pair.slave.spawn_command(cb).unwrap();
        drop(pair.slave);
        // Read the master itself (a cloned reader would keep the PTY open) until the wrapper runs (it sets the
        // title), then close it.
        let fd = pair.master.as_raw_fd().unwrap();
        let (mut out, mut b) = (Vec::new(), [0u8; 4096]);
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(20);
        while !String::from_utf8_lossy(&out).contains("\x1b]0;")
            && std::time::Instant::now() < deadline
        {
            let mut p = libc::pollfd {
                fd,
                events: libc::POLLIN,
                revents: 0,
            };
            // SAFETY: poll and read on the master fd we own, into a local buffer.
            if unsafe { libc::poll(&mut p, 1, 100) } == 1 {
                let n = unsafe { libc::read(fd, b.as_mut_ptr().cast(), b.len()) };
                if n <= 0 {
                    break;
                }
                out.extend_from_slice(&b[..n as usize]);
            }
        }
        let out = String::from_utf8_lossy(&out).into_owned();
        drop(pair.master);
        let exited = (0..200).any(|_| {
            std::thread::sleep(std::time::Duration::from_millis(50));
            child.try_wait().ok().flatten().is_some()
        });
        if !exited {
            let _ = child.kill();
        }
        assert!(exited, "{out:?}");
        assert!(
            std::fs::read_to_string(&marker)
                .unwrap_or_default()
                .starts_with("Ok("),
            "panicked: {out:?}"
        );
        let left: Vec<_> = std::fs::read_dir(home.join("run"))
            .into_iter()
            .flatten()
            .flatten()
            .map(|e| e.file_name())
            .collect();
        assert!(
            left.iter()
                .all(|n| !n.to_string_lossy().starts_with("debug-shell-")),
            "{left:?}"
        );
        let _ = std::fs::remove_dir_all(home);
    }
}
