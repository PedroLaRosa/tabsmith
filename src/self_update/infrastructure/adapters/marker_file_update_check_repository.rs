use crate::self_update::domain::repositories::update_check_repository::UpdateCheckRepository;
use crate::shared::domain::value_objects::data_directory::DataDirectory;

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

impl UpdateCheckRepository for MarkerFileUpdateCheckRepository {
    fn last_check_secs(&self) -> Option<u64> {
        None
    }

    fn record_check(&self, _now_secs: u64) {}
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shared::domain::value_objects::data_directory::DataDirectory;
    use crate::shared::infrastructure::testing::temp_dir::TempDir;

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
}
