#[cfg(test)]
use std::cell::RefCell;
use std::path::Path;
#[cfg(test)]
use std::path::PathBuf;

/// Deletes tabsmith's data directory, for `uninstall --purge`.
pub(crate) trait DataDirectoryPurger {
    /// A directory that is not there is fine; any other failure is the io error itself.
    fn purge(&self, directory: &Path) -> std::io::Result<()>;
}

/// Records each directory it was asked to purge, and fails with an io error showing the message it was given, for
/// tests.
#[cfg(test)]
#[derive(Default)]
pub(crate) struct RecordingDataDirectoryPurger {
    failure: Option<String>,
    purged: RefCell<Vec<PathBuf>>,
}

#[cfg(test)]
impl RecordingDataDirectoryPurger {
    pub(crate) fn failing(message: &str) -> Self {
        RecordingDataDirectoryPurger {
            failure: Some(message.into()),
            ..Self::default()
        }
    }

    pub(crate) fn purged(&self) -> Vec<PathBuf> {
        self.purged.borrow().clone()
    }
}

#[cfg(test)]
impl DataDirectoryPurger for RecordingDataDirectoryPurger {
    fn purge(&self, directory: &Path) -> std::io::Result<()> {
        self.purged.borrow_mut().push(directory.into());
        match &self.failure {
            Some(message) => Err(std::io::Error::other(message.clone())),
            None => Ok(()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn should_record_each_directory_it_was_asked_to_purge() {
        let purger = RecordingDataDirectoryPurger::default();

        let purged = purger.purge(Path::new("/home/u/.tabsmith"));

        assert!(
            purged.is_ok() && purger.purged() == [PathBuf::from("/home/u/.tabsmith")],
            "{purged:?}"
        );
    }

    #[test]
    fn should_fail_with_an_io_error_showing_the_message_it_was_given() {
        let purger = RecordingDataDirectoryPurger::failing("Permission denied (os error 13)");

        let failure = purger.purge(Path::new("/home/u/.tabsmith")).unwrap_err();

        assert_eq!(
            (failure.to_string(), purger.purged()),
            (
                "Permission denied (os error 13)".to_string(),
                vec![PathBuf::from("/home/u/.tabsmith")]
            )
        );
    }
}
