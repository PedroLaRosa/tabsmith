//! Tiny file logger (SPEC §3.1 logs/). Never writes to the terminal from inside `_ pty`.
use crate::shared::domain::value_objects::data_directory::DataDirectory;
use std::io::Write;
use std::sync::{Mutex, OnceLock};

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub(crate) enum Level {
    Error = 1,
    Warn,
    Info,
    Debug,
    Trace,
}

struct Logger {
    file: Option<Mutex<std::fs::File>>,
    level: Level,
    stderr: bool,
    session: String,
}

static LOGGER: OnceLock<Logger> = OnceLock::new();

fn parse_level(s: &str) -> Option<Level> {
    Some(match s.to_ascii_lowercase().as_str() {
        "error" => Level::Error,
        "warn" => Level::Warn,
        "info" => Level::Info,
        "debug" => Level::Debug,
        "trace" => Level::Trace,
        _ => return None,
    })
}

/// `name` = log file stem (`cli` or `pty`); `verbose` = count of `-v`; `stderr` mirrors records when `-v` was given;
/// `data_directory` holds the logs dir; without a known root there is no log file.
pub(crate) fn init(name: &str, verbose: u8, stderr: bool, data_directory: &DataDirectory) {
    let level = match verbose {
        0 => std::env::var("TABSMITH_LOG_LEVEL")
            .ok()
            .and_then(|l| parse_level(&l))
            .unwrap_or(Level::Warn),
        1 => Level::Warn,
        2 => Level::Info,
        3 => Level::Debug,
        _ => Level::Trace,
    };
    let file = data_directory
        .logs_dir()
        .ok()
        .and_then(|dir| open_log(&dir, name));
    let session = std::env::var("TABSMITH_SESSION_ID").unwrap_or_else(|_| "-".into());
    let _ = LOGGER.set(Logger {
        file: file.map(Mutex::new),
        level,
        stderr: stderr && verbose > 0,
        session,
    });
}

/// `<dir>/<name>.log` for appending. Logs hold command text, so on Unix the dir is 0700 and the file 0600
/// (like history and run files, SPEC §3.1), also when they already existed.
fn open_log(dir: &std::path::Path, name: &str) -> Option<std::fs::File> {
    // ponytail: only under an existing data root, so a session still running after `uninstall --purge` does not
    // recreate ~/.tabsmith; the very first command before any data dir exists is not logged.
    if !dir.parent().is_some_and(|p| p.is_dir()) {
        return None;
    }
    let _ = std::fs::create_dir(dir);
    let mut opts = std::fs::OpenOptions::new();
    opts.create(true).append(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
        let _ = std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o700));
        opts.mode(0o600);
    }
    let file = opts.open(dir.join(format!("{name}.log"))).ok()?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = file.set_permissions(std::fs::Permissions::from_mode(0o600));
    }
    Some(file)
}

pub(crate) fn log(level: Level, msg: &str) {
    let Some(l) = LOGGER.get() else { return };
    if level > l.level {
        return;
    }
    let ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    let line = format!("{ms} {level:?} {} {msg}\n", l.session);
    if let Some(f) = &l.file {
        if let Ok(mut f) = f.lock() {
            let _ = f.write_all(line.as_bytes());
        }
    }
    if l.stderr {
        eprint!("{line}");
    }
}

pub(crate) fn error(msg: &str) {
    log(Level::Error, msg)
}
pub(crate) fn warn(msg: &str) {
    log(Level::Warn, msg)
}
pub(crate) fn info(msg: &str) {
    log(Level::Info, msg)
}
pub(crate) fn debug(msg: &str) {
    log(Level::Debug, msg)
}

#[cfg(all(test, unix))]
mod tests {
    use crate::shared::infrastructure::testing::temp_dir::TempDir;
    use std::os::unix::fs::PermissionsExt;

    #[test]
    fn log_files_are_private() {
        let temp_dir = TempDir::new("logger");
        let dir = temp_dir.path().join("logs");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o755)).unwrap();
        std::fs::write(dir.join("old.log"), "x\n").unwrap();
        std::fs::set_permissions(dir.join("old.log"), std::fs::Permissions::from_mode(0o644))
            .unwrap();
        let mode = |p: &std::path::Path| std::fs::metadata(p).unwrap().permissions().mode() & 0o777;
        for name in ["new", "old"] {
            assert!(super::open_log(&dir, name).is_some());
            assert_eq!(mode(&dir.join(format!("{name}.log"))), 0o600, "{name}.log");
        }
        assert_eq!(mode(&dir), 0o700);
        let _ = std::fs::remove_dir_all(dir.parent().unwrap());
    }
}
