use crate::modules::shared::infrastructure::logger;
use crate::modules::dropdown::domain::services::theme_resolver::resolve_theme;
use crate::modules::terminal_session::application::ports::terminal_modes::TerminalModes;
use crate::modules::terminal_session::application::ports::terminal_screen::TerminalScreen;
use crate::modules::terminal_session::application::ports::terminal_writer::TerminalWriter;
use crate::modules::terminal_session::application::ports::wrapped_shell::WrappedShell;
use crate::modules::terminal_session::application::session_ports_parts::SessionPortsParts;
use crate::modules::terminal_session::application::session_use_cases::SessionUseCases;
use crate::modules::terminal_session::domain::entities::marker_tracker::MarkerTracker;
use crate::modules::terminal_session::domain::entities::terminal_session::{TerminalSession, TerminalSessionParts};
use crate::modules::terminal_session::infrastructure::adapters::alacritty_terminal_screen::AlacrittyTerminalScreen;
use crate::modules::terminal_session::infrastructure::adapters::console_terminal_modes::ConsoleTerminalModes;
use crate::modules::terminal_session::infrastructure::adapters::overlay_renderer::OverlayRenderer;
use crate::modules::terminal_session::infrastructure::adapters::pty_wrapped_shell::PtyWrappedShell;
use crate::modules::terminal_session::infrastructure::adapters::stdout_terminal_writer::StdoutTerminalWriter;
use crate::modules::shared::infrastructure::data_directory_from_env::user_home_from_env;
use crate::modules::shared::domain::value_objects::shell::Shell;
use crate::modules::terminal_session::domain::value_objects::session_id::SessionId;
use crate::modules::terminal_session::domain::value_objects::terminal_background::TerminalBackground;
use crate::modules::terminal_session::infrastructure::adapters::kitty_graphics;
use crate::modules::terminal_session::domain::value_objects::wrapped_shell_command::WrappedShellCommand;
use crate::modules::terminal_session::infrastructure::terminal_session_module::TerminalSessionModule;
use crate::modules::terminal_session::infrastructure::pty::pty_reader::spawn_pty_reader;
use crate::modules::terminal_session::infrastructure::pty::terminal_probe::{probe_terminal, PROBE_TIMEOUT};
use crate::modules::terminal_session::infrastructure::pty::session_message::SessionMessage;
use crate::modules::terminal_session::infrastructure::pty::session_runner::{SessionRunner, SessionRunnerParts};
use anyhow::{bail, Context, Result};
use portable_pty::{CommandBuilder, PtySize};
use std::io::{IsTerminal, Write};
use std::rc::Rc;
use std::sync::mpsc;
use std::thread;
use std::time::{Instant, SystemTime, UNIX_EPOCH};
#[cfg(unix)]
use crate::modules::terminal_session::infrastructure::pty::platform_unix as sys;
#[cfg(windows)]
use crate::modules::terminal_session::infrastructure::pty::platform_windows as sys;

/// SPEC §4.4 steps 1–9. `modes` receives the saved terminal modes as soon as they change.
pub(crate) fn launch_session(command: &WrappedShellCommand, modes: &mut Option<Rc<dyn TerminalModes>>, module: &TerminalSessionModule) -> Result<SessionRunner> {
    if !std::io::stdin().is_terminal() || !std::io::stdout().is_terminal() {
        bail!("stdin/stdout is not a terminal");
    }
    let program = command
        .program()
        .context("no shell to run (set SHELL or pass --shell)")?;
    let output: Rc<dyn TerminalWriter> = Rc::new(StdoutTerminalWriter);
    let live_settings = module.live_settings();
    let settings = live_settings.load();
    let (saved_modes, lf_to_ind) = sys::enter()?;
    let terminal_modes: Rc<dyn TerminalModes> = Rc::new(ConsoleTerminalModes::new(saved_modes));
    *modes = Some(terminal_modes.clone());
    let (cols, rows) = crossterm::terminal::size().context("terminal size")?;
    let (cols, rows) = (cols.max(1), rows.max(1));
    let id = generate_session_id();
    let shell = Shell::from_name(program).unwrap_or(if cfg!(windows) {
        Shell::Pwsh
    } else {
        Shell::Bash
    });

    // Sync the model with the real cursor; always ask for the background, the theme may become `system` later.
    let (cpr, terminal_bg, typeahead) = probe_terminal(&*output, PROBE_TIMEOUT, sys::read_timeout);
    // ponytail: a reply cut by the timeout is forwarded as typeahead; only a whole late reply is dropped.
    let probe_ended_at = cpr.is_none().then(Instant::now);
    if cpr.is_none() {
        output.write(b"\x1b[H\x1b[2J");
    }

    // Everything that can fail goes before the child exists, so the fallback never orphans a shell.
    let truecolor = matches!(
        std::env::var("COLORTERM").as_deref(),
        Ok("truecolor" | "24bit")
    ) || std::env::var_os("WT_SESSION").is_some();
    let themes = module.themes();
    let renderer = OverlayRenderer::new(resolve_theme(&settings, terminal_bg, &*themes), truecolor)
        .with_images(kitty_graphics::detect());
    let marker = MarkerTracker::new(
        id.clone(),
        Shell::from_name(program),
        cfg!(windows),
    );
    let screen = AlacrittyTerminalScreen::new(cols as usize, rows as usize, renderer, lf_to_ind);
    // Sync the model with the real cursor, through the same parser as the shell's output.
    if let Some((r, c)) = cpr {
        screen.advance_model(format!("\x1b[{r};{c}H").as_bytes());
    }
    let screen: Rc<dyn TerminalScreen> = Rc::new(screen);
    let clock = module.clock();
    let (tx, rx) = mpsc::channel();
    sys::spawn_resize(tx.clone())?;
    let engine = module.start_completion_engine()(tx.clone(), settings.clone());

    let pair = portable_pty::native_pty_system().openpty(PtySize {
        rows,
        cols,
        pixel_width: 0,
        pixel_height: 0,
    })?;
    let command = command
        .clone()
        .in_session(env!("CARGO_PKG_VERSION"), std::env::var_os("TMUX").is_some(), &id);
    let mut cb = CommandBuilder::from_argv(command.argv().iter().map(Into::into).collect());
    let cwd = std::env::current_dir()
        .ok()
        .filter(|d| d.is_dir())
        .or_else(|| user_home_from_env().ok());
    if let Some(cwd) = cwd {
        cb.cwd(cwd);
    }
    for (name, value) in command.env_to_set() {
        cb.env(name, value);
    }
    for name in command.env_to_remove() {
        cb.env_remove(name);
    }
    let mut child = pair.slave.spawn_command(cb)?;
    drop(pair.slave);
    let reader = pair.master.try_clone_reader()?;
    let mut pty_writer = pair.master.take_writer()?;
    logger::info(&format!(
        "pty: {:?} {cols}x{rows} sid {} cpr {cpr:?}",
        command.argv(),
        id.as_str()
    ));

    output.write(&command.window_title());

    // Threads (SPEC §4.3).
    let credits = spawn_pty_reader(reader, tx.clone());
    let (input, wrx) = mpsc::channel::<Vec<u8>>();
    thread::spawn(move || {
        for b in wrx {
            if pty_writer
                .write_all(&b)
                .and_then(|_| pty_writer.flush())
                .is_err()
            {
                break;
            }
        }
    });
    let t = tx.clone();
    thread::spawn(move || {
        let _ = t.send(SessionMessage::ChildExit(sys::wait(&mut *child)));
    });
    sys::spawn_input(tx);
    let pty = Rc::new(PtyWrappedShell::new(input, pair.master));
    if !typeahead.is_empty() {
        pty.write_input(typeahead);
    }

    let session_contexts = module.session_contexts();
    let mut session = TerminalSession::new(TerminalSessionParts {
        id,
        launched_shell: shell,
        settings,
        tracker: marker,
        terminal_background: terminal_bg.map(TerminalBackground::new),
        settings_modified: live_settings.last_modified(),
        now: Instant::now(),
    });
    if let Some(probe_ended_at) = probe_ended_at {
        session.expect_late_probe_reply(probe_ended_at);
    }
    let working_directory = module.working_directory();
    let generator_processes = module.generator_processes();
    let use_cases = SessionUseCases::new(SessionPortsParts {
        engine: engine.clone(),
        screen: screen.clone(),
        writer: output.clone(),
        shell: pty.clone(),
        reaper: generator_processes.clone(),
        modes: terminal_modes.clone(),
        live_settings: live_settings.clone(),
        session_contexts: session_contexts.clone(),
        themes: themes.clone(),
        working_directory: working_directory.clone(),
        clock: clock.clone(),
    });
    Ok(SessionRunner::new(SessionRunnerParts {
        inbox: rx,
        credits,
        pty,
        session,
        screen,
        output,
        use_cases,
        clock,
    }))
}

fn generate_session_id() -> SessionId {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0);
    SessionId::from_entropy(nanos, std::process::id())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn should_generate_a_session_id_of_32_lowercase_hex_digits() {
        let id = generate_session_id();
        let id = id.as_str();

        assert_eq!(id.len(), 32);
        assert!(id.chars().all(|c| matches!(c, '0'..='9' | 'a'..='f')), "{id}");
        assert!(SessionId::new(id.to_string()).is_ok(), "{id}");
    }
}
