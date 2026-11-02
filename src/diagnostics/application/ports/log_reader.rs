#[cfg(test)]
use crate::diagnostics::application::test_fixtures::{logged_path, EventLog};
#[cfg(test)]
use std::collections::HashMap;
use std::path::Path;
#[cfg(test)]
use std::path::PathBuf;

/// Reads a log file as the bytes it holds.
pub(crate) trait LogReader {
    /// The content of the file; `None` when it cannot be read for any reason.
    fn read(&self, path: &Path) -> Option<Vec<u8>>;
}

/// Answers from the files a test put in it, keyed by path; any other path cannot be read.
/// Each read is logged when the double was given a log to share.
#[cfg(test)]
#[derive(Default)]
pub(crate) struct InMemoryLogReader {
    files: HashMap<PathBuf, Vec<u8>>,
    log: Option<EventLog>,
}

#[cfg(test)]
impl InMemoryLogReader {
    pub(crate) fn with_file(mut self, path: &str, content: &[u8]) -> Self {
        self.files.insert(PathBuf::from(path), content.to_vec());
        self
    }

    pub(crate) fn logging_to(mut self, log: EventLog) -> Self {
        self.log = Some(log);
        self
    }
}

#[cfg(test)]
impl LogReader for InMemoryLogReader {
    fn read(&self, path: &Path) -> Option<Vec<u8>> {
        if let Some(log) = &self.log {
            log.push(format!("log_reader.read({})", logged_path(path)));
        }
        self.files.get(path).cloned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn should_return_the_bytes_scripted_for_the_path() {
        let reader = InMemoryLogReader::default().with_file("/logs/pty.log", b"one\ntwo\n");

        let content = reader.read(Path::new("/logs/pty.log"));

        assert_eq!(content, Some(b"one\ntwo\n".to_vec()));
    }

    #[test]
    fn should_return_none_for_a_path_nobody_scripted() {
        let reader = InMemoryLogReader::default().with_file("/logs/pty.log", b"one");

        let content = reader.read(Path::new("/logs/cli.log"));

        assert_eq!(content, None);
    }

    #[test]
    fn should_return_an_empty_file_as_some_empty_bytes() {
        let reader = InMemoryLogReader::default().with_file("/logs/pty.log", b"");

        let content = reader.read(Path::new("/logs/pty.log"));

        assert_eq!(content, Some(Vec::new()));
    }

    #[test]
    fn should_log_every_read_including_the_ones_that_find_nothing() {
        let log = EventLog::default();
        let reader = InMemoryLogReader::default()
            .with_file("/logs/pty.log", b"x")
            .logging_to(log.clone());

        reader.read(Path::new("/logs/pty.log"));
        reader.read(Path::new("/logs/cli.log"));

        assert_eq!(
            log.entries(),
            [
                "log_reader.read(/logs/pty.log)",
                "log_reader.read(/logs/cli.log)"
            ]
        );
    }

    #[test]
    fn should_read_without_a_log_when_none_is_given() {
        let reader = InMemoryLogReader::default().with_file("/logs/pty.log", b"x");

        let content = reader.read(Path::new("/logs/pty.log"));

        assert_eq!(content, Some(b"x".to_vec()));
    }
}
