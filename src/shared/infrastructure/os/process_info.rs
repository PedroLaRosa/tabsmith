use crate::shared::infrastructure::os::executable_path_error::ExecutablePathError;

/// Unix euid 0.
pub(crate) fn is_root() -> bool {
    #[cfg(unix)]
    {
        // SAFETY: geteuid has no preconditions.
        unsafe { libc::geteuid() == 0 }
    }
    #[cfg(not(unix))]
    {
        false
    }
}

/// `GetConsoleMode` succeeds on `handle`.
#[cfg(windows)]
pub(crate) fn is_console(handle: std::os::windows::io::RawHandle) -> bool {
    let mut mode = 0;
    // SAFETY: plain query on a handle we own for the process lifetime; `mode` is a valid out pointer.
    unsafe { windows_sys::Win32::System::Console::GetConsoleMode(handle as _, &mut mode) != 0 }
}

/// Absolute path of the running tabsmith binary.
pub(crate) fn exe_path() -> Result<String, ExecutablePathError> {
    Ok(std::env::current_exe()?.to_string_lossy().into_owned())
}

/// Executable path of the parent process (SPEC §2.11 `_ get-shell`).
pub(crate) fn parent_exe() -> Option<String> {
    #[cfg(target_os = "linux")]
    {
        // SAFETY: getppid has no preconditions.
        let ppid = unsafe { libc::getppid() };
        std::fs::read_link(format!("/proc/{ppid}/exe"))
            .ok()
            .map(|p| p.to_string_lossy().into_owned())
    }
    #[cfg(target_os = "macos")]
    {
        // SAFETY: getppid has no preconditions; proc_pidpath writes at most `buf.len()` bytes into `buf`.
        let ppid = unsafe { libc::getppid() };
        let mut buf = vec![0u8; libc::PROC_PIDPATHINFO_MAXSIZE as usize];
        let n = unsafe { libc::proc_pidpath(ppid, buf.as_mut_ptr().cast(), buf.len() as u32) };
        (n > 0).then(|| String::from_utf8_lossy(&buf[..n as usize]).into_owned())
    }
    #[cfg(windows)]
    {
        windows_parent_exe()
    }
    #[cfg(not(any(target_os = "linux", target_os = "macos", windows)))]
    {
        None
    }
}

/// Parent PID from a Toolhelp32 process snapshot, then its full image path.
/// ponytail: Windows never re-parents, so a parent that already exited may have had its PID reused.
#[cfg(windows)]
fn windows_parent_exe() -> Option<String> {
    use windows_sys::Win32::Foundation::{CloseHandle, INVALID_HANDLE_VALUE};
    use windows_sys::Win32::System::Diagnostics::ToolHelp::{
        CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W,
        TH32CS_SNAPPROCESS,
    };
    use windows_sys::Win32::System::Threading::{
        GetCurrentProcessId, OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_WIN32,
        PROCESS_QUERY_LIMITED_INFORMATION,
    };
    // SAFETY: plain Win32 calls; `entry.dwSize` is set as Process32FirstW requires, `buf`/`len` describe a valid
    // buffer, and every handle opened here is closed before returning.
    unsafe {
        let snap = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0);
        if snap == INVALID_HANDLE_VALUE {
            return None;
        }
        let me = GetCurrentProcessId();
        let mut entry: PROCESSENTRY32W = std::mem::zeroed();
        entry.dwSize = std::mem::size_of::<PROCESSENTRY32W>() as u32;
        let mut found = None;
        let mut ok = Process32FirstW(snap, &mut entry) != 0;
        while ok {
            if entry.th32ProcessID == me {
                found = Some(entry.th32ParentProcessID);
                break;
            }
            ok = Process32NextW(snap, &mut entry) != 0;
        }
        CloseHandle(snap);
        let proc = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, found?);
        if proc.is_null() {
            return None;
        }
        let mut buf = vec![0u16; 32768];
        let mut len = buf.len() as u32;
        let ok =
            QueryFullProcessImageNameW(proc, PROCESS_NAME_WIN32, buf.as_mut_ptr(), &mut len) != 0;
        CloseHandle(proc);
        ok.then(|| String::from_utf16_lossy(&buf[..len as usize]))
    }
}
