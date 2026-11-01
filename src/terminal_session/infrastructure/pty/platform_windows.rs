use crate::terminal_session::infrastructure::pty::session_message::SessionMessage;
use anyhow::{bail, Result};
use std::sync::mpsc::Sender;
use std::time::Duration;
use windows_sys::Win32::Foundation::{HANDLE, WAIT_OBJECT_0};
use windows_sys::Win32::System::Console::{
    GetConsoleCP, GetConsoleMode, GetConsoleOutputCP, GetStdHandle, ReadConsoleInputW,
    SetConsoleCP, SetConsoleMode, SetConsoleOutputCP, DISABLE_NEWLINE_AUTO_RETURN,
    ENABLE_ECHO_INPUT, ENABLE_LINE_INPUT, ENABLE_PROCESSED_INPUT,
    ENABLE_VIRTUAL_TERMINAL_INPUT, ENABLE_VIRTUAL_TERMINAL_PROCESSING, ENABLE_WINDOW_INPUT,
    INPUT_RECORD, KEY_EVENT, STD_INPUT_HANDLE, STD_OUTPUT_HANDLE, WINDOW_BUFFER_SIZE_EVENT,
};
use windows_sys::Win32::System::Threading::WaitForSingleObject;

const CP_UTF8: u32 = 65001;

/// Saved console state, restored on exit.
#[derive(Clone, Copy, Default)]
pub(crate) struct Modes {
    input: u32,
    output: u32,
    in_cp: u32,
    out_cp: u32,
}

fn handles() -> (HANDLE, HANDLE) {
    // SAFETY: GetStdHandle has no preconditions.
    unsafe {
        (
            GetStdHandle(STD_INPUT_HANDLE),
            GetStdHandle(STD_OUTPUT_HANDLE),
        )
    }
}

impl Modes {
    pub(crate) fn restore(&self) {
        let (i, o) = handles();
        // SAFETY: plain console calls with values previously read from the same handles.
        unsafe {
            SetConsoleMode(i, self.input);
            SetConsoleMode(o, self.output);
            SetConsoleCP(self.in_cp);
            SetConsoleOutputCP(self.out_cp);
        }
    }
}

/// Console modes (SPEC §4.4 step 4). Returns whether bare LF must be rewritten to `ESC D`
/// (`DISABLE_NEWLINE_AUTO_RETURN` unsupported).
pub(crate) fn enter() -> Result<(Modes, bool)> {
    let (i, o) = handles();
    let (mut input, mut output) = (0u32, 0u32);
    // SAFETY: plain console calls; out-params point to locals.
    unsafe {
        if GetConsoleMode(i, &mut input) == 0 || GetConsoleMode(o, &mut output) == 0 {
            bail!("not a Windows console");
        }
        let m = Modes {
            input,
            output,
            in_cp: GetConsoleCP(),
            out_cp: GetConsoleOutputCP(),
        };
        let raw = (input & !(ENABLE_ECHO_INPUT | ENABLE_LINE_INPUT | ENABLE_PROCESSED_INPUT))
            | ENABLE_VIRTUAL_TERMINAL_INPUT
            | ENABLE_WINDOW_INPUT;
        if SetConsoleMode(i, raw) == 0 {
            bail!("cannot set the console input mode");
        }
        let vt = output | ENABLE_VIRTUAL_TERMINAL_PROCESSING;
        let lf_to_ind = SetConsoleMode(o, vt | DISABLE_NEWLINE_AUTO_RETURN) == 0;
        if lf_to_ind && SetConsoleMode(o, vt) == 0 {
            m.restore();
            bail!("the console has no VT support (Windows 10 1809+ required)");
        }
        SetConsoleCP(CP_UTF8);
        SetConsoleOutputCP(CP_UTF8);
        Ok((m, lf_to_ind))
    }
}

/// Read input records: key-down chars → UTF-8 (surrogate pairs joined across reads), plus "a resize happened".
fn read_console(h: HANDLE, high: &mut Option<u16>) -> Option<(Vec<u8>, bool)> {
    // SAFETY: INPUT_RECORD is plain old data; ReadConsoleInputW fills at most `len` records.
    let mut recs: [INPUT_RECORD; 64] = unsafe { std::mem::zeroed() };
    let mut n = 0u32;
    if unsafe { ReadConsoleInputW(h, recs.as_mut_ptr(), recs.len() as u32, &mut n) } == 0 {
        return None;
    }
    let mut units: Vec<u16> = high.take().into_iter().collect();
    let mut resized = false;
    for r in &recs[..n as usize] {
        match r.EventType as u32 {
            KEY_EVENT => {
                // SAFETY: EventType says the union holds a KEY_EVENT_RECORD.
                let k = unsafe { r.Event.KeyEvent };
                let ch = unsafe { k.uChar.UnicodeChar };
                if k.bKeyDown != 0 && ch != 0 {
                    units.extend(std::iter::repeat_n(ch, k.wRepeatCount.max(1) as usize));
                }
            }
            WINDOW_BUFFER_SIZE_EVENT => resized = true,
            _ => {}
        }
    }
    if units.last().is_some_and(|u| (0xd800..0xdc00).contains(u)) {
        *high = units.pop();
    }
    Some((String::from_utf16_lossy(&units).into_bytes(), resized))
}

/// Input available within `t` (startup probe), read without a thread so a fallback shell keeps its input.
pub(crate) fn read_timeout(t: Duration) -> Vec<u8> {
    let (i, _) = handles();
    // SAFETY: waiting on the console input handle.
    if unsafe { WaitForSingleObject(i, t.as_millis() as u32) } != WAIT_OBJECT_0 {
        return Vec::new();
    }
    read_console(i, &mut None)
        .map(|(b, _)| b)
        .unwrap_or_default()
}

pub(crate) fn spawn_input(tx: Sender<SessionMessage>) {
    std::thread::spawn(move || {
        let (i, _) = handles();
        let mut high = None;
        while let Some((bytes, resized)) = read_console(i, &mut high) {
            if !bytes.is_empty() && tx.send(SessionMessage::Stdin(bytes)).is_err() {
                break;
            }
            if resized {
                if let Ok((c, r)) = crossterm::terminal::size() {
                    let _ = tx.send(SessionMessage::Resize(c, r));
                }
            }
        }
    });
}

pub(crate) fn wait(child: &mut dyn portable_pty::Child) -> i32 {
    child.wait().map_or(1, |s| s.exit_code() as i32)
}

/// No SIGWINCH on Windows: also poll the size every 250 ms (legacy conhost may not send the event).
pub(crate) fn spawn_resize(tx: Sender<SessionMessage>) -> Result<()> {
    std::thread::spawn(move || {
        let mut last = crossterm::terminal::size().ok();
        loop {
            std::thread::sleep(Duration::from_millis(250));
            let now = crossterm::terminal::size().ok();
            if now != last {
                last = now;
                if let Some((c, r)) = now {
                    if tx.send(SessionMessage::Resize(c, r)).is_err() {
                        break;
                    }
                }
            }
        }
    });
    Ok(())
}
