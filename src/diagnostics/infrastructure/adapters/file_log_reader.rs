use crate::diagnostics::application::ports::log_reader::LogReader;
use std::path::Path;

/// Reads log files from the disk.
pub(crate) struct FileLogReader;

impl LogReader for FileLogReader {
    fn read(&self, path: &Path) -> Option<Vec<u8>> {
        std::fs::read(path).ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shared::infrastructure::testing::temp_dir::TempDir;

    #[test]
    fn should_read_the_bytes_of_an_existing_file_from_the_disk() {
        let temp_dir = TempDir::new("file-log-reader-existing");
        let path = temp_dir.path().join("pty.log");
        std::fs::write(&path, b"one\ntwo\n").unwrap();

        let content = FileLogReader.read(&path);

        assert_eq!(content, Some(b"one\ntwo\n".to_vec()));
    }

    #[test]
    fn should_read_an_empty_file_as_some_empty_bytes() {
        let temp_dir = TempDir::new("file-log-reader-empty");
        let path = temp_dir.path().join("pty.log");
        std::fs::write(&path, b"").unwrap();

        let content = FileLogReader.read(&path);

        assert_eq!(content, Some(Vec::new()));
    }

    #[test]
    fn should_return_none_for_a_file_that_does_not_exist() {
        let temp_dir = TempDir::new("file-log-reader-missing");
        let path = temp_dir.path().join("pty.log");

        let content = FileLogReader.read(&path);

        assert_eq!(content, None);
    }

    #[test]
    fn should_return_none_for_a_directory_named_like_the_log() {
        let temp_dir = TempDir::new("file-log-reader-directory");
        let path = temp_dir.path().join("pty.log");
        std::fs::create_dir(&path).unwrap();

        let content = FileLogReader.read(&path);

        assert_eq!(content, None);
    }
}
