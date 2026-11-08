use std::path::PathBuf;

/// The directory the wrapper itself runs in. Asked at every request: it can change, or vanish, while the session
/// runs.
pub(crate) trait WorkingDirectory {
    fn current(&self) -> Option<PathBuf>;
}

/// Test double: the directory it was given.
#[cfg(test)]
pub(crate) struct FixedWorkingDirectory {
    directory: Option<PathBuf>,
}

#[cfg(test)]
impl FixedWorkingDirectory {
    pub(crate) fn new(directory: Option<PathBuf>) -> Self {
        FixedWorkingDirectory { directory }
    }
}

#[cfg(test)]
impl WorkingDirectory for FixedWorkingDirectory {
    fn current(&self) -> Option<PathBuf> {
        self.directory.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn should_report_the_directory_it_was_given() {
        let directory = FixedWorkingDirectory::new(Some(PathBuf::from("/work")));

        let current = directory.current();

        assert_eq!(current, Some(PathBuf::from("/work")));
    }

    #[test]
    fn should_report_none_when_it_has_none() {
        let directory = FixedWorkingDirectory::new(None);

        let current = directory.current();

        assert_eq!(current, None);
    }
}
