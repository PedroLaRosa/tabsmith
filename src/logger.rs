//! Tiny file logger (SPEC §3.1 logs/). Never writes to the terminal from inside `_ pty`.
use std::io::Write;
use std::sync::{Mutex, OnceLock};

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub enum Level {
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

/// `name` = log file stem (`cli` or `pty`); `verbose` = count of `-v`; `stderr` mirrors records when `-v` was given.
pub fn init(name: &str, verbose: u8, stderr: bool) {
    let level = match verbose {
        0 => std::env::var("TABSMITH_LOG_LEVEL").ok().and_then(|l| parse_level(&l)).unwrap_or(Level::Warn),
        1 => Level::Warn,
        2 => Level::Info,
        3 => Level::Debug,
        _ => Level::Trace,
    };
    let file = crate::settings::home().ok().and_then(|home| {
        let dir = home.join("logs");
        std::fs::create_dir_all(&dir).ok()?;
        std::fs::OpenOptions::new().create(true).append(true).open(dir.join(format!("{name}.log"))).ok()
    });
    let session = std::env::var("TABSMITH_SESSION_ID").unwrap_or_else(|_| "-".into());
    let _ = LOGGER.set(Logger { file: file.map(Mutex::new), level, stderr: stderr && verbose > 0, session });
}

pub fn enabled(level: Level) -> bool {
    LOGGER.get().is_some_and(|l| level <= l.level)
}

pub fn log(level: Level, msg: &str) {
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

pub fn error(msg: &str) {
    log(Level::Error, msg)
}
pub fn warn(msg: &str) {
    log(Level::Warn, msg)
}
pub fn info(msg: &str) {
    log(Level::Info, msg)
}
pub fn debug(msg: &str) {
    log(Level::Debug, msg)
}
pub fn trace(msg: &str) {
    log(Level::Trace, msg)
}
