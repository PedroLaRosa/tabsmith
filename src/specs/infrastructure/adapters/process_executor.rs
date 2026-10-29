#[cfg(all(test, unix))]
use crate::shared::infrastructure::testing::temp_dir::TempDir;
use crate::specs::domain::value_objects::exec_request::ExecRequest;
use crate::specs::domain::value_objects::generator_context::GeneratorContext;
use crate::specs::domain::value_objects::process_output::ProcessOutput;
use crate::specs::infrastructure::adapters::native_ls::native_ls;
use std::ffi::OsString;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc;
use std::sync::{Mutex, MutexGuard};
use std::time::{Duration, Instant};

/// Run a process with RunProcess semantics (SPEC §9.3), blocking. Used by exec workers.
/// `default_timeout_ms` = `autocomplete.scriptTimeout` (5000; Windows 20000).
pub(crate) fn exec(
    input: &ExecRequest,
    ctx: &GeneratorContext,
    default_timeout_ms: u64,
) -> Result<ProcessOutput, String> {
    let cwd = [input.cwd().as_deref(), Some(ctx.cwd())]
        .into_iter()
        .flatten()
        .map(PathBuf::from)
        .find(|p| !p.as_os_str().is_empty() && p.is_dir())
        .or_else(|| std::env::current_dir().ok())
        .unwrap_or_else(|| PathBuf::from(if cfg!(windows) { "C:\\" } else { "/" }));
    if input.command() == "ls" && input.args().iter().map(|a| a.trim()).eq(["-1ApL"]) {
        // The listed dir itself, never the fallback: `cd nonexist/` must list nothing, not the shell cwd.
        let dir = input
            .cwd()
            .as_deref()
            .filter(|c| !c.is_empty())
            .map_or(cwd, |c| Path::new(ctx.cwd()).join(c));
        return Ok(native_ls(&dir));
    }
    if ["fig", "tabsmith"].contains(&input.command()) {
        return Ok(ProcessOutput::new(String::new(), String::new(), 1)); // bundled ai() helper
    }
    // Effective value of a variable in the child's env (input.env > session env > wrapper env).
    let var = |key: &str| -> Option<OsString> {
        if let Some((_, v)) = input.env().iter().find(|(k, _)| key_eq(k, key)) {
            return v.as_ref().map(OsString::from);
        }
        if ctx.env().is_empty() {
            std::env::var_os(key)
        } else {
            ctx.env()
                .iter()
                .find(|(k, _)| key_eq(k, key))
                .map(|(_, v)| v.into())
        }
    };
    let mut cmd = build_command(input, &cwd, &var)?;
    if !ctx.env().is_empty() {
        cmd.env_clear().envs(ctx.env());
    }
    for k in ["LS_COLORS", "CLICOLOR_FORCE", "CLICOLOR", "COLORTERM"] {
        cmd.env_remove(k);
    }
    cmd.envs([
        ("PROCESS_LAUNCHED_BY_TABSMITH", "1"),
        ("HISTFILE", ""),
        ("HISTCONTROL", "ignoreboth"),
        ("TERM", "xterm-256color"),
        ("NO_COLOR", "1"),
    ]);
    for (k, v) in input.env() {
        match v {
            Some(v) => cmd.env(k, v),
            None => cmd.env_remove(k),
        };
    }
    // cmd.exe runs bare names from its cwd before PATH (a repo's `npm.cmd`); this makes it skip the cwd.
    if Path::new(cmd.get_program())
        .file_stem()
        .is_some_and(|s| s.eq_ignore_ascii_case("cmd"))
    {
        cmd.env("NoDefaultCurrentDirectoryInExePath", "1");
    }
    cmd.current_dir(&cwd)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        // SAFETY: runs in the forked child before exec; setsid is async-signal-safe.
        unsafe {
            cmd.pre_exec(|| {
                if libc::setsid() == -1 {
                    Err(std::io::Error::last_os_error())
                } else {
                    Ok(())
                }
            });
        }
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(windows_sys::Win32::System::Threading::CREATE_NO_WINDOW);
    }
    let timeout = default_timeout_ms.max(input.timeout().unwrap_or(0));
    let child = cmd
        .spawn()
        .map_err(|e| format!("failed running executable ({}): {e}", input.command()))?;
    wait_output(child, timeout, input.command())
}

fn key_eq(a: &str, b: &str) -> bool {
    if cfg!(windows) {
        a.eq_ignore_ascii_case(b)
    } else {
        a == b
    }
}

/// `Command` for `input` (no shell). Windows: the legacy `sh -c s` form runs through Git Bash, else `cmd /C`.
fn build_command(
    input: &ExecRequest,
    cwd: &Path,
    var: &dyn Fn(&str) -> Option<OsString>,
) -> Result<Command, String> {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        if input.command() == "sh"
            && input.args().len() == 2
            && input.args()[0] == "-c"
            && find_exe("sh", cwd, var).is_none()
        {
            let mut c;
            match git_bash(cwd, var) {
                Some(bash) => {
                    c = Command::new(bash);
                    c.args(input.args());
                }
                None => {
                    c = Command::new(var("ComSpec").unwrap_or_else(|| "cmd.exe".into()));
                    c.arg("/C").raw_arg(&input.args()[1]); // raw: cmd does its own (non-MSVC) parsing
                }
            }
            return Ok(c);
        }
    }
    let exe = find_exe(input.command(), cwd, var)
        .ok_or_else(|| format!("failed running executable ({}): not found", input.command()))?;
    let mut c = Command::new(exe);
    c.args(input.args());
    Ok(c)
}

/// `which`: absolute → as is; with a separator → relative to `cwd`; else each `PATH` dir (Windows: `PATHEXT`).
fn find_exe(name: &str, cwd: &Path, var: &dyn Fn(&str) -> Option<OsString>) -> Option<PathBuf> {
    let p = Path::new(name);
    if p.is_absolute() {
        return Some(p.to_path_buf());
    }
    let dirs: Vec<PathBuf> = if p.components().count() > 1 {
        vec![cwd.to_path_buf()]
    } else {
        var("PATH")
            .map(|v| std::env::split_paths(&v).collect())
            .unwrap_or_default()
    };
    let exts: Vec<String> = if cfg!(windows) {
        let pathext = var("PATHEXT").map(|v| v.to_string_lossy().into_owned());
        let mut exts: Vec<String> = if p.extension().is_some() {
            vec![String::new()]
        } else {
            vec![]
        };
        exts.extend(
            pathext
                .as_deref()
                .unwrap_or(".COM;.EXE;.BAT;.CMD")
                .split(';')
                .filter(|e| !e.is_empty())
                .map(String::from),
        );
        exts
    } else {
        vec![String::new()]
    };
    dirs.iter()
        .flat_map(|d| exts.iter().map(move |e| d.join(format!("{name}{e}"))))
        .find(|c| is_executable(c))
}

fn is_executable(p: &Path) -> bool {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::metadata(p).is_ok_and(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
    }
    #[cfg(not(unix))]
    {
        p.is_file()
    }
}

/// Git for Windows' `bash.exe` (ish `gitBashPath`): next to `git.exe` on PATH, else the usual install dirs.
#[cfg(windows)]
fn git_bash(cwd: &Path, var: &dyn Fn(&str) -> Option<OsString>) -> Option<PathBuf> {
    let mut roots: Vec<PathBuf> = find_exe("git", cwd, var)
        .and_then(|g| Some(g.parent()?.parent()?.to_path_buf()))
        .into_iter()
        .collect();
    for v in ["ProgramW6432", "ProgramFiles", "ProgramFiles(x86)"] {
        roots.extend(var(v).map(|d| PathBuf::from(d).join("Git")));
    }
    roots.extend(var("LOCALAPPDATA").map(|d| PathBuf::from(d).join("Programs").join("Git")));
    roots
        .iter()
        .flat_map(|r| {
            [
                r.join("bin").join("bash.exe"),
                r.join("usr").join("bin").join("bash.exe"),
            ]
        })
        .find(|p| p.is_file())
}

/// Collect stdout/stderr (reader threads, so full pipes never block the child) while polling for exit every
/// 5 ms; on timeout kill the child (Unix: its whole session/process group; Windows: its job object) and fail.
fn wait_output(mut child: Child, timeout_ms: u64, label: &str) -> Result<ProcessOutput, String> {
    #[cfg(unix)]
    let _live = Live::new(child.id() as isize);
    #[cfg(windows)]
    let job = job::Job::assign(&child);
    #[cfg(windows)]
    let _live = job.as_ref().map(|j| Live::new(j.handle() as isize)); // dropped before `job` closes the handle
    let (otx, orx) = mpsc::channel::<(usize, Vec<u8>)>();
    let pipes: [Option<Box<dyn Read + Send>>; 2] = [
        child
            .stdout
            .take()
            .map(|p| Box::new(p) as Box<dyn Read + Send>),
        child
            .stderr
            .take()
            .map(|p| Box::new(p) as Box<dyn Read + Send>),
    ];
    for (i, pipe) in pipes.into_iter().enumerate() {
        let otx = otx.clone();
        std::thread::spawn(move || {
            let mut buf = Vec::new();
            if let Some(mut p) = pipe {
                let _ = p.read_to_end(&mut buf);
            }
            let _ = otx.send((i, buf));
        });
    }
    let deadline = Instant::now() + Duration::from_millis(timeout_ms);
    let mut out: [Option<Vec<u8>>; 2] = [None, None];
    let mut status = None;
    loop {
        if status.is_none() {
            status = child
                .try_wait()
                .map_err(|e| format!("waiting for {label}: {e}"))?;
        }
        if let (Some(s), [Some(o), Some(e)]) = (status, &out) {
            return Ok(ProcessOutput::new(clean(o), clean(e), s.code().unwrap_or(1)));
        }
        if Instant::now() >= deadline {
            #[cfg(unix)]
            // SAFETY: plain syscall; the child called setsid, so its pid is its process-group id.
            unsafe {
                libc::kill(-(child.id() as libc::pid_t), libc::SIGKILL);
            }
            #[cfg(windows)]
            if let Some(job) = &job {
                job.kill();
            }
            let _ = child.kill();
            let _ = child.wait();
            return Err(format!(
                "timed out after {timeout_ms} ms running executable ({label})"
            ));
        }
        if let Ok((i, b)) = orx.recv_timeout(Duration::from_millis(5)) {
            out[i] = Some(b);
        }
    }
}

/// Exec children still running: Unix process-group ids (each child called setsid), Windows job handles.
static LIVE: Mutex<Vec<isize>> = Mutex::new(Vec::new());

fn live() -> MutexGuard<'static, Vec<isize>> {
    LIVE.lock().unwrap_or_else(|e| e.into_inner())
}

/// Registers an exec child in `LIVE` while it runs.
struct Live(isize);

impl Live {
    fn new(id: isize) -> Live {
        #[cfg(unix)]
        {
            extern "C" fn at_exit() {
                kill_live_children();
            }
            static AT_EXIT: std::sync::Once = std::sync::Once::new();
            // SAFETY: registers a plain `extern "C" fn` that only takes a lock and sends signals.
            AT_EXIT.call_once(|| unsafe {
                libc::atexit(at_exit);
            });
        }
        live().push(id);
        Live(id)
    }
}

impl Drop for Live {
    fn drop(&mut self) {
        live().retain(|&g| g != self.0);
    }
}

/// Kill every exec child still running, with everything it started, so no generator outlives the session (a hanging
/// one would run forever, reparented to init). Unix: also runs at process exit (`atexit`, i.e. `std::process::exit`).
pub(crate) fn kill_live_children() {
    for &id in live().iter() {
        #[cfg(unix)]
        // SAFETY: plain syscall; `id` is the process-group id of a child that called setsid.
        unsafe {
            libc::kill(-(id as libc::pid_t), libc::SIGKILL);
        }
        #[cfg(windows)]
        job::terminate(id as windows_sys::Win32::Foundation::HANDLE);
    }
}

/// Windows job object holding an exec child and every process it starts, so a timeout kills the whole tree (std's
/// `Child::kill` only ends the direct child; grandchildren would keep the output pipes and reader threads alive).
#[cfg(windows)]
mod job {
    use std::ffi::c_void;
    use std::os::windows::io::AsRawHandle;
    use windows_sys::Win32::Foundation::{CloseHandle, HANDLE};

    // ponytail: declared here because windows-sys only has them behind its `Win32_System_JobObjects` feature.
    #[link(name = "kernel32")]
    extern "system" {
        fn CreateJobObjectW(attrs: *const c_void, name: *const u16) -> HANDLE;
        fn AssignProcessToJobObject(job: HANDLE, process: HANDLE) -> i32;
        fn TerminateJobObject(job: HANDLE, exit_code: u32) -> i32;
    }

    pub(crate) struct Job(HANDLE);

    impl Job {
        /// `None` if no job could be created or assigned (then a timeout kills only the child).
        /// ponytail: assigned right after spawn, so a process the child starts in that instant escapes; closing the
        /// gap needs CREATE_SUSPENDED + resuming the main thread, which std's `Command` does not expose.
        pub(crate) fn assign(child: &std::process::Child) -> Option<Job> {
            // SAFETY: plain Win32 calls; the child's process handle is valid while `child` lives.
            unsafe {
                let job = CreateJobObjectW(std::ptr::null(), std::ptr::null());
                if job.is_null() {
                    return None;
                }
                let job = Job(job);
                (AssignProcessToJobObject(job.0, child.as_raw_handle() as HANDLE) != 0)
                    .then_some(job)
            }
        }

        pub(crate) fn kill(&self) {
            terminate(self.0);
        }

        pub(crate) fn handle(&self) -> HANDLE {
            self.0
        }
    }

    /// `job` must be an open job handle (`kill_live_children` holds the `LIVE` lock, so it is not closed meanwhile).
    pub(crate) fn terminate(job: HANDLE) {
        // SAFETY: see above.
        unsafe { TerminateJobObject(job, 1) };
    }

    impl Drop for Job {
        fn drop(&mut self) {
            // SAFETY: closed exactly once; no kill-on-close limit, so processes still running are left alone.
            unsafe { CloseHandle(self.0) };
        }
    }
}

/// Cleans exec output: lossy UTF-8, `\r\n` → `\n`, drop `ESC[?25h`, strip leading/trailing newlines.
fn clean(b: &[u8]) -> String {
    String::from_utf8_lossy(b)
        .replace("\r\n", "\n")
        .replace("\x1b[?25h", "")
        .trim_matches('\n')
        .to_string()
}

/// A temp dir holding executable shell scripts, for hermetic "fake" commands.
#[cfg(all(test, unix))]
pub(crate) fn fake_bin(scripts: &[(&str, &str)]) -> TempDir {
    use std::os::unix::fs::PermissionsExt;
    let dir = TempDir::new("bin");
    for (name, body) in scripts {
        let p = dir.path().join(name);
        std::fs::write(&p, format!("#!/bin/sh\n{body}\n")).unwrap();
        std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o755)).unwrap();
    }
    dir
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shared::infrastructure::testing::temp_dir::TempDir;
    use std::collections::HashMap;

    #[test]
    fn native_ls_and_short_circuits() {
        let dir = TempDir::new("ls");
        std::fs::write(dir.path().join("a.txt"), "").unwrap();
        std::fs::write(dir.path().join(".hidden"), "").unwrap();
        std::fs::create_dir(dir.path().join("sub")).unwrap();
        #[cfg(unix)]
        std::os::unix::fs::symlink(dir.path().join("sub"), dir.path().join("link")).unwrap();
        let mut got: Vec<String> = native_ls(dir.path())
            .stdout()
            .lines()
            .map(String::from)
            .collect();
        got.sort();
        let mut want = vec![".hidden", "a.txt", "sub/"];
        if cfg!(unix) {
            want.insert(2, "link/");
        }
        assert_eq!(got, want);
        let missing = native_ls(&dir.path().join("nope"));
        assert_eq!((missing.status(), missing.stdout()), (2, ""));

        // exec answers `ls -1ApL` natively (args trimmed; no input.cwd → ctx cwd; relative → under ctx cwd; missing
        // dir → error, not the ctx cwd's listing) and stubs fig/tabsmith.
        let ctx = GeneratorContext::new(
            dir.path().display().to_string(),
            HashMap::new(),
            String::new(),
            String::new(),
            false,
        );
        let ls = |cwd: Option<&str>| {
            let input = ExecRequest::new(
                "ls".into(),
                vec!["-1ApL ".into()],
                cwd.map(String::from),
                HashMap::new(),
                None,
            );
            exec(&input, &ctx, 5000).unwrap()
        };
        assert_eq!(ls(None), native_ls(dir.path()));
        assert_eq!(ls(Some("sub/")), native_ls(&dir.path().join("sub")));
        let missing = ls(Some(&dir.path().join("nonexist/").display().to_string()));
        assert_eq!((missing.status(), missing.stdout()), (2, ""));
        for cmd in ["fig", "tabsmith"] {
            let input = ExecRequest::new(cmd.into(), vec![], None, HashMap::new(), None);
            assert_eq!(
                exec(&input, &ctx, 5000),
                Ok(ProcessOutput::new(String::new(), String::new(), 1))
            );
        }
    }

    #[test]
    fn exec_reports_an_unknown_command_instead_of_stubbing_it() {
        let empty_path = TempDir::new("empty-path");
        let ctx = GeneratorContext::new(
            String::new(),
            HashMap::from([("PATH".to_string(), empty_path.path().display().to_string())]),
            String::new(),
            String::new(),
            false,
        );

        let input = ExecRequest::new("no-such-cli".into(), vec![], None, HashMap::new(), None);

        let result = exec(&input, &ctx, 5000);

        assert!(result.unwrap_err().contains("not found"));
    }

    #[cfg(unix)]
    #[test]
    fn exec_status_output_and_timeout() {
        let sh = |script: &str, timeout: Option<u64>| {
            let input = ExecRequest::new(
                "sh".into(),
                vec!["-c".into(), script.into()],
                None,
                HashMap::new(),
                timeout,
            );
            exec(&input, &GeneratorContext::default(), 100)
        };
        assert_eq!(
            sh(
                "printf '\\n\\nout\\r\\nx\\033[?25h\\n\\n'; echo err >&2; exit 3",
                None
            ),
            Ok(ProcessOutput::new("out\nx".into(), "err".into(), 3))
        );
        assert_eq!(
            sh("kill -9 $$", None).unwrap().status(),
            1,
            "killed by a signal → 1"
        );
        assert_eq!(
            sh("echo $TERM $CLICOLOR", None).unwrap().stdout(),
            "xterm-256color"
        );
        let t0 = Instant::now();
        let e = sh("sleep 5; echo late", None).unwrap_err();
        assert!(e.contains("timed out after 100 ms"), "{e}");
        assert!(t0.elapsed() < Duration::from_secs(2));
        assert_eq!(
            sh("sleep 0.2; echo ok", Some(3000)).unwrap().stdout(),
            "ok",
            "input.timeout raises the floor"
        );
    }

    /// cmd.exe looks for bare names in its cwd before PATH unless `NoDefaultCurrentDirectoryInExePath` is set.
    #[cfg(unix)]
    #[test]
    fn cmd_gets_no_cwd_search_env() {
        let bin = fake_bin(&[("cmd", r#"printf %s "$NoDefaultCurrentDirectoryInExePath""#)]);
        let ctx = GeneratorContext::new(
            String::new(),
            [("PATH".to_string(), bin.path().display().to_string())].into(),
            String::new(),
            String::new(),
            false,
        );
        let input = ExecRequest::new(
            "cmd".into(),
            vec!["/C".into(), "git".into()],
            None,
            HashMap::new(),
            None,
        );
        assert_eq!(exec(&input, &ctx, 5000).unwrap().stdout(), "1");
    }

    /// A generator still running when the wrapper exits dies with it (re-runs this test in a child process that
    /// calls `std::process::exit` mid-exec, like the wrapper at the end of a session).
    #[cfg(unix)]
    #[test]
    fn exec_children_die_with_the_wrapper() {
        const NAME: &str = "specs::infrastructure::adapters::process_executor::tests::exec_children_die_with_the_wrapper";
        if std::env::var_os("TABSMITH_TEST_EXIT_MID_EXEC").is_some() {
            std::thread::spawn(|| {
                let args = vec!["-c".into(), "sleep 30; echo never".into()];
                exec(
                    &ExecRequest::new("/bin/sh".into(), args, None, HashMap::new(), None),
                    &GeneratorContext::default(),
                    60_000,
                )
            });
            let pgid = loop {
                if let Some(&g) = live().first() {
                    break g;
                }
                std::thread::sleep(Duration::from_millis(5));
            };
            std::thread::sleep(Duration::from_millis(100)); // let sh start `sleep`
            println!("pgid={pgid}");
            std::process::exit(0);
        }
        let out = Command::new(std::env::current_exe().unwrap())
            .args([NAME, "--exact", "--nocapture"])
            .env("TABSMITH_TEST_EXIT_MID_EXEC", "1")
            .output()
            .unwrap();
        let stdout = String::from_utf8_lossy(&out.stdout);
        let pgid: libc::pid_t = stdout
            .split("pgid=")
            .nth(1)
            .and_then(|r| r.split_whitespace().next())
            .unwrap()
            .parse()
            .unwrap();
        // SAFETY: plain syscalls on the child's process group.
        let gone = (0..300).any(|_| {
            std::thread::sleep(Duration::from_millis(10));
            (unsafe { libc::kill(-pgid, 0) }) == -1
        });
        if !gone {
            unsafe { libc::kill(-pgid, libc::SIGKILL) };
        }
        assert!(gone, "process group {pgid} outlived the wrapper");
    }

    /// A timeout kills the whole tree: the grandchild (inner cmd) never writes its marker.
    #[cfg(windows)]
    #[test]
    fn timeout_kills_grandchildren() {
        let dir = TempDir::new("tree");
        let args = [
            "/C",
            "cmd",
            "/C",
            "ping",
            "-n",
            "3",
            "127.0.0.1",
            "^>nul",
            "^&",
            "echo",
            "x^>marker",
        ];
        let input = ExecRequest::new(
            "cmd".into(),
            args.map(String::from).to_vec(),
            Some(dir.path().display().to_string()),
            HashMap::new(),
            None,
        );
        assert!(exec(&input, &GeneratorContext::default(), 300)
            .unwrap_err()
            .contains("timed out"));
        std::thread::sleep(Duration::from_secs(4));
        assert!(
            !dir.path().join("marker").exists(),
            "grandchild survived the timeout"
        );
    }
}
