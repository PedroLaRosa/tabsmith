use crate::diagnostics::application::log_paths::LogPaths;
use crate::diagnostics::application::ports::log_reader::LogReader;
use crate::diagnostics::domain::diagnostics_error::DiagnosticsError;
use crate::shared::domain::value_objects::data_directory::DataDirectory;
use std::rc::Rc;

const TAIL_LINES: usize = 50;

/// Shows where tabsmith keeps its logs and the end of the pty log. The paths come first and cost no I/O, so the
/// command can print them before it reads a pty log that may be huge or not a regular file.
pub(crate) struct ShowRecentLogsUseCase {
    data_directory: DataDirectory,
    reader: Rc<dyn LogReader>,
}

impl ShowRecentLogsUseCase {
    pub(crate) fn new(data_directory: DataDirectory, reader: Rc<dyn LogReader>) -> Self {
        ShowRecentLogsUseCase {
            data_directory,
            reader,
        }
    }

    pub(crate) fn paths(&self) -> Result<LogPaths, DiagnosticsError> {
        let logs_dir = self
            .data_directory
            .logs_dir()
            .map_err(DiagnosticsError::DataDirectory)?;
        Ok(LogPaths::new(logs_dir.join("cli.log"), logs_dir.join("pty.log")))
    }

    /// The last lines of the pty log; `None` when it cannot be read, and no lines for an empty file.
    pub(crate) fn pty_tail(&self, paths: &LogPaths) -> Option<Vec<String>> {
        let bytes = self.reader.read(paths.pty_log())?;
        let text = String::from_utf8_lossy(&bytes);
        let lines: Vec<&str> = text.lines().collect();
        let first_kept = lines.len().saturating_sub(TAIL_LINES);
        Some(lines[first_kept..].iter().map(|line| line.to_string()).collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diagnostics::application::test_fixtures::EventLog;
    use crate::diagnostics::application::ports::log_reader::InMemoryLogReader;
    use std::path::PathBuf;

    const PTY_LOG: &str = "/data/logs/pty.log";

    fn data_directory() -> DataDirectory {
        DataDirectory::new(Some(PathBuf::from("/data")), None)
    }

    fn numbered_lines(count: usize) -> Vec<u8> {
        (1..=count).map(|n| format!("line {n}\n")).collect::<String>().into_bytes()
    }

    fn tail_of(content: &[u8]) -> Option<Vec<String>> {
        let reader = InMemoryLogReader::default().with_file(PTY_LOG, content);
        let use_case = ShowRecentLogsUseCase::new(data_directory(), Rc::new(reader));
        let paths = use_case.paths().expect("the data directory is known");
        use_case.pty_tail(&paths)
    }

    fn first_and_last(tail: Option<Vec<String>>) -> (usize, String, String) {
        let lines = tail.expect("the log can be read");
        (lines.len(), lines[0].clone(), lines[lines.len() - 1].clone())
    }

    #[test]
    fn should_return_the_cli_log_and_the_pty_log_under_the_logs_directory() {
        let use_case = ShowRecentLogsUseCase::new(data_directory(), Rc::new(InMemoryLogReader::default()));

        let paths = use_case.paths();

        assert_eq!(
            paths.ok(),
            Some(LogPaths::new(
                PathBuf::from("/data/logs/cli.log"),
                PathBuf::from("/data/logs/pty.log")
            ))
        );
    }

    #[test]
    fn should_return_both_paths_even_when_no_log_exists() {
        let log = EventLog::default();
        let reader = InMemoryLogReader::default().logging_to(log.clone());
        let use_case = ShowRecentLogsUseCase::new(data_directory(), Rc::new(reader));

        let paths = use_case.paths();

        assert_eq!(paths.map(|p| p.pty_log().to_path_buf()).ok(), Some(PathBuf::from(PTY_LOG)));
    }

    #[test]
    fn should_not_read_any_file_while_working_out_the_paths() {
        let log = EventLog::default();
        let reader = InMemoryLogReader::default().with_file(PTY_LOG, b"x").logging_to(log.clone());
        let use_case = ShowRecentLogsUseCase::new(data_directory(), Rc::new(reader));

        let _ = use_case.paths();

        assert_eq!(log.entries(), Vec::<String>::new());
    }

    #[test]
    fn should_fail_with_the_home_directory_text_when_the_data_directory_has_no_root() {
        let reader = InMemoryLogReader::default();
        let use_case = ShowRecentLogsUseCase::new(DataDirectory::new(None, None), Rc::new(reader));

        let error = use_case.paths().err().map(|e| e.to_string());

        assert_eq!(
            error,
            Some("could not determine the home directory (set HOME or TABSMITH_HOME)".to_string())
        );
    }

    #[test]
    fn should_fail_with_a_data_directory_error_when_the_data_directory_has_no_root() {
        let use_case = ShowRecentLogsUseCase::new(
            DataDirectory::new(None, None),
            Rc::new(InMemoryLogReader::default()),
        );

        let error = use_case.paths().err();

        assert!(matches!(error, Some(DiagnosticsError::DataDirectory(_))));
    }

    #[test]
    fn should_never_ask_the_reader_when_the_data_directory_has_no_root() {
        let log = EventLog::default();
        let reader = InMemoryLogReader::default().logging_to(log.clone());
        let use_case = ShowRecentLogsUseCase::new(DataDirectory::new(None, None), Rc::new(reader));

        let _ = use_case.paths();

        assert_eq!(log.entries(), Vec::<String>::new());
    }

    #[test]
    fn should_read_the_pty_log_only_when_asked_for_its_tail() {
        let log = EventLog::default();
        let reader = InMemoryLogReader::default().with_file(PTY_LOG, b"a\n").logging_to(log.clone());
        let use_case = ShowRecentLogsUseCase::new(data_directory(), Rc::new(reader));
        let paths = use_case.paths().expect("the data directory is known");

        let _ = use_case.pty_tail(&paths);

        assert_eq!(log.entries(), ["log_reader.read(/data/logs/pty.log)"]);
    }

    #[test]
    fn should_return_every_line_of_a_log_shorter_than_fifty_lines() {
        let tail = tail_of(&numbered_lines(49));

        assert_eq!(first_and_last(tail), (49, "line 1".to_string(), "line 49".to_string()));
    }

    #[test]
    fn should_return_every_line_of_a_log_of_exactly_fifty_lines() {
        let tail = tail_of(&numbered_lines(50));

        assert_eq!(first_and_last(tail), (50, "line 1".to_string(), "line 50".to_string()));
    }

    #[test]
    fn should_drop_the_oldest_line_of_a_log_of_fifty_one_lines() {
        let tail = tail_of(&numbered_lines(51));

        assert_eq!(first_and_last(tail), (50, "line 2".to_string(), "line 51".to_string()));
    }

    #[test]
    fn should_keep_only_the_last_fifty_lines_of_a_long_log() {
        let tail = tail_of(&numbered_lines(120));

        assert_eq!(first_and_last(tail), (50, "line 71".to_string(), "line 120".to_string()));
    }

    #[test]
    fn should_return_no_lines_but_some_tail_for_an_empty_log() {
        let tail = tail_of(b"");

        assert_eq!(tail, Some(Vec::new()));
    }

    #[test]
    fn should_return_no_tail_when_the_log_cannot_be_read() {
        let use_case = ShowRecentLogsUseCase::new(data_directory(), Rc::new(InMemoryLogReader::default()));
        let paths = use_case.paths().expect("the data directory is known");

        let tail = use_case.pty_tail(&paths);

        assert_eq!(tail, None);
    }

    #[test]
    fn should_drop_the_carriage_return_of_windows_line_endings() {
        let tail = tail_of(b"one\r\ntwo\r\n");

        assert_eq!(tail, Some(vec!["one".to_string(), "two".to_string()]));
    }

    #[test]
    fn should_replace_invalid_utf8_instead_of_failing() {
        let tail = tail_of(b"ok \xff\nnext\n");

        assert_eq!(tail, Some(vec!["ok \u{FFFD}".to_string(), "next".to_string()]));
    }

    #[test]
    fn should_keep_the_last_line_that_has_no_final_newline() {
        let tail = tail_of(b"one\ntwo");

        assert_eq!(tail, Some(vec!["one".to_string(), "two".to_string()]));
    }
}
