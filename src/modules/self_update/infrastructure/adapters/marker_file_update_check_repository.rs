use crate::modules::self_update::domain::repositories::update_check_repository::UpdateCheckRepository;
use crate::modules::shared::domain::value_objects::data_directory::DataDirectory;
use crate::modules::shared::infrastructure::logger;
use anyhow::Context;
use std::fs::{self, File};
use std::time::{Duration, UNIX_EPOCH};

/// The last background update check, kept as the modification time of `update-check` in the data directory. Only
/// the time is kept, so the check never writes `state.json` and cannot race the engine's writes to it.
pub(crate) struct MarkerFileUpdateCheckRepository {
    data_directory: DataDirectory,
}

impl MarkerFileUpdateCheckRepository {
    pub(crate) fn new(data_directory: DataDirectory) -> Self {
        MarkerFileUpdateCheckRepository { data_directory }
    }
}

impl MarkerFileUpdateCheckRepository {
    /// The marker with `now_secs` as its modification time; the data root has to exist already.
    fn write_marker(&self, now_secs: u64) -> anyhow::Result<()> {
        let path = self.data_directory.update_check_file()?;
        let marker = File::create(&path).with_context(|| format!("could not write {}", path.display()))?;
        marker
            .set_modified(UNIX_EPOCH + Duration::from_secs(now_secs))
            .with_context(|| format!("could not set the time of {}", path.display()))
    }
}

impl UpdateCheckRepository for MarkerFileUpdateCheckRepository {
    /// `None` while there is no marker, or no data directory to hold one.
    fn last_check_secs(&self) -> Option<u64> {
        let path = self.data_directory.update_check_file().ok()?;
        let modified = fs::metadata(path).ok()?.modified().ok()?;
        modified.duration_since(UNIX_EPOCH).ok().map(|elapsed| elapsed.as_secs())
    }

    /// A marker that cannot be written is logged and the check goes unrecorded: the next tab checks again.
    fn record_check(&self, now_secs: u64) {
        if let Err(error) = self.write_marker(now_secs) {
            logger::warn(&format!("update check: {error:#}"));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::shared::domain::value_objects::data_directory::DataDirectory;
    use crate::modules::shared::infrastructure::testing::temp_dir::TempDir;

    fn repository(temp_dir: &TempDir) -> MarkerFileUpdateCheckRepository {
        MarkerFileUpdateCheckRepository::new(DataDirectory::new(Some(temp_dir.path().to_path_buf()), None))
    }

    #[test]
    fn should_have_no_last_check_before_any_was_recorded() {
        let temp_dir = TempDir::new("update-check-none");
        let repository = repository(&temp_dir);

        let last_check_secs = repository.last_check_secs();

        assert_eq!(last_check_secs, None);
    }

    #[test]
    fn should_read_back_the_time_of_the_check_it_recorded() {
        let temp_dir = TempDir::new("update-check-recorded");
        let repository = repository(&temp_dir);

        repository.record_check(1_700_000_000);

        assert_eq!(repository.last_check_secs(), Some(1_700_000_000));
    }
}
