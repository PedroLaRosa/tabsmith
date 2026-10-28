use crate::shared::domain::value_objects::data_directory::DataDirectory;
use crate::shared::infrastructure::json_document_store::JsonDocumentStore;
use crate::shell_integration::domain::repositories::restart_reminder_repository::RestartReminderRepository;

/// The reminder as `doctor.prompt-restart-terminal` in `state.json`, next to the other keys kept there.
pub(crate) struct StateRestartReminderRepository {
    data_directory: DataDirectory,
}

impl StateRestartReminderRepository {
    pub(crate) fn new(data_directory: DataDirectory) -> Self {
        StateRestartReminderRepository { data_directory }
    }
}

impl RestartReminderRepository for StateRestartReminderRepository {
    /// Nothing is written without a data directory or when `state.json` cannot be loaded (it is never clobbered); a
    /// failed save is ignored.
    fn save_pending(&self) {
        let state_file = self.data_directory.state_file();
        if let Some(mut s) = state_file.ok().and_then(|path| JsonDocumentStore::load(&path).ok()) {
            s.map
                .insert("doctor.prompt-restart-terminal".into(), true.into());
            let _ = s.save();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shared::infrastructure::testing::temp_dir::TempDir;

    #[test]
    fn should_save_the_reminder_into_a_new_state_file() {
        let temp_dir = TempDir::new("reminder-new");
        let repository = StateRestartReminderRepository::new(DataDirectory::new(Some(temp_dir.path().into()), None));

        repository.save_pending();

        assert_eq!(
            std::fs::read_to_string(temp_dir.path().join("state.json")).unwrap(),
            "{\n  \"doctor.prompt-restart-terminal\": true\n}\n"
        );
    }
}
