use crate::modules::terminal_session::application::ports::working_directory::WorkingDirectory;
use std::path::PathBuf;

/// The directory this process runs in, asked of the operating system at every call.
pub(crate) struct ProcessWorkingDirectory;

impl WorkingDirectory for ProcessWorkingDirectory {
    fn current(&self) -> Option<PathBuf> {
        std::env::current_dir().ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn should_report_the_process_current_directory() {
        let directory = ProcessWorkingDirectory;

        let current = directory.current();

        assert_eq!(current, std::env::current_dir().ok());
    }
}
