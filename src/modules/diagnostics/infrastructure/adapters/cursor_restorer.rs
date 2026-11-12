#[cfg(unix)]
pub(crate) fn restore_cursor_on_ctrl_c() {
    if let Ok(mut signals) = signal_hook::iterator::Signals::new([libc::SIGINT]) {
        std::thread::spawn(move || {
            if signals.forever().next().is_some() {
                eprint!("\x1b[?25h");
                std::process::exit(1);
            }
        });
    }
}

#[cfg(windows)]
pub(crate) fn restore_cursor_on_ctrl_c() {
    unsafe extern "system" fn handler(_ctrl: u32) -> i32 {
        eprint!("\x1b[?25h");
        std::process::exit(1)
    }
    // SAFETY: registers a plain function pointer that lives for the whole process.
    unsafe {
        windows_sys::Win32::System::Console::SetConsoleCtrlHandler(Some(handler), 1);
    }
}
