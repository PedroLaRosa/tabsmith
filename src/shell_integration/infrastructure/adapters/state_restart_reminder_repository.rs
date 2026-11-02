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

    /// Only a stored boolean `true` is pending; no data directory, no file or a corrupt file is not.
    fn is_pending(&self) -> bool {
        let state_file = self.data_directory.state_file();
        state_file
            .ok()
            .and_then(|path| JsonDocumentStore::load(&path).ok())
            .is_some_and(|s| s.map.get("doctor.prompt-restart-terminal").and_then(|v| v.as_bool()) == Some(true))
    }

    /// The key stays, set to `false`. Nothing is written without a data directory or when `state.json` cannot be
    /// loaded; a failed save is ignored.
    fn clear_pending(&self) {
        let state_file = self.data_directory.state_file();
        if let Some(mut s) = state_file.ok().and_then(|path| JsonDocumentStore::load(&path).ok()) {
            s.map
                .insert("doctor.prompt-restart-terminal".into(), false.into());
            let _ = s.save();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shared::infrastructure::testing::temp_dir::TempDir;
    use std::path::Path;

    fn rooted_at(root: &Path) -> StateRestartReminderRepository {
        StateRestartReminderRepository::new(DataDirectory::new(Some(root.into()), None))
    }

    #[test]
    fn should_save_the_reminder_into_a_new_state_file() {
        let temp_dir = TempDir::new("reminder-new");
        let repository = rooted_at(temp_dir.path());

        repository.save_pending();

        assert_eq!(
            std::fs::read_to_string(temp_dir.path().join("state.json")).unwrap(),
            "{\n  \"doctor.prompt-restart-terminal\": true\n}\n"
        );
    }

    #[test]
    fn should_keep_the_other_keys_of_the_state_file() {
        let temp_dir = TempDir::new("reminder-keeps");
        let state_file = temp_dir.path().join("state.json");
        std::fs::write(&state_file, "{\"autocomplete.recency\": {\"git\": 3}}").unwrap();

        rooted_at(temp_dir.path()).save_pending();

        assert_eq!(
            std::fs::read_to_string(&state_file).unwrap(),
            "{\n  \"autocomplete.recency\": {\n    \"git\": 3\n  },\n  \"doctor.prompt-restart-terminal\": true\n}\n"
        );
    }

    #[test]
    fn should_leave_a_corrupt_state_file_untouched() {
        let temp_dir = TempDir::new("reminder-corrupt");
        let state_file = temp_dir.path().join("state.json");
        std::fs::write(&state_file, "{oops").unwrap();

        rooted_at(temp_dir.path()).save_pending();

        assert_eq!(std::fs::read_to_string(&state_file).unwrap(), "{oops");
    }

    /// The settings override does not move the state file, so without a root there is nowhere to save it.
    #[test]
    fn should_save_nothing_without_a_data_directory() {
        let temp_dir = TempDir::new("reminder-no-root");
        let repository = StateRestartReminderRepository::new(DataDirectory::new(
            None,
            Some(temp_dir.path().join("settings.json")),
        ));

        repository.save_pending();

        assert_eq!(std::fs::read_dir(temp_dir.path()).unwrap().count(), 0);
    }

    #[test]
    fn should_recreate_a_missing_data_directory_to_save_the_reminder() {
        let temp_dir = TempDir::new("reminder-purged");
        let root = temp_dir.path().join(".tabsmith");

        rooted_at(&root).save_pending();

        assert_eq!(
            std::fs::read_to_string(root.join("state.json")).unwrap(),
            "{\n  \"doctor.prompt-restart-terminal\": true\n}\n"
        );
    }

    fn state_file_with(temp_dir: &TempDir, contents: &str) -> std::path::PathBuf {
        let state_file = temp_dir.path().join("state.json");
        std::fs::write(&state_file, contents).unwrap();
        state_file
    }

    fn without_data_directory(temp_dir: &TempDir) -> StateRestartReminderRepository {
        StateRestartReminderRepository::new(DataDirectory::new(
            None,
            Some(temp_dir.path().join("settings.json")),
        ))
    }

    #[test]
    fn should_report_the_reminder_pending_after_it_is_saved() {
        let temp_dir = TempDir::new("reminder-pending");
        let repository = rooted_at(temp_dir.path());
        repository.save_pending();

        let pending = repository.is_pending();

        assert!(pending);
    }

    #[test]
    fn should_report_no_reminder_pending_for_a_new_data_directory() {
        let temp_dir = TempDir::new("reminder-fresh");

        let pending = rooted_at(temp_dir.path()).is_pending();

        assert!(!pending);
    }

    #[test]
    fn should_report_no_reminder_pending_for_a_stored_false() {
        let temp_dir = TempDir::new("reminder-false");
        state_file_with(&temp_dir, "{\"doctor.prompt-restart-terminal\": false}");

        let pending = rooted_at(temp_dir.path()).is_pending();

        assert!(!pending);
    }

    #[test]
    fn should_report_no_reminder_pending_for_the_string_true() {
        let temp_dir = TempDir::new("reminder-string");
        state_file_with(&temp_dir, "{\"doctor.prompt-restart-terminal\": \"true\"}");

        let pending = rooted_at(temp_dir.path()).is_pending();

        assert!(!pending);
    }

    #[test]
    fn should_report_no_reminder_pending_for_the_number_one() {
        let temp_dir = TempDir::new("reminder-number");
        state_file_with(&temp_dir, "{\"doctor.prompt-restart-terminal\": 1}");

        let pending = rooted_at(temp_dir.path()).is_pending();

        assert!(!pending);
    }

    #[test]
    fn should_report_no_reminder_pending_for_a_corrupt_state_file() {
        let temp_dir = TempDir::new("reminder-pending-corrupt");
        state_file_with(&temp_dir, "{oops");

        let pending = rooted_at(temp_dir.path()).is_pending();

        assert!(!pending);
    }

    #[test]
    fn should_report_no_reminder_pending_without_a_data_directory() {
        let temp_dir = TempDir::new("reminder-pending-no-root");

        let pending = without_data_directory(&temp_dir).is_pending();

        assert!(!pending);
    }

    #[test]
    fn should_clear_a_saved_reminder_but_keep_its_key() {
        let temp_dir = TempDir::new("reminder-clear");
        let repository = rooted_at(temp_dir.path());
        repository.save_pending();

        repository.clear_pending();

        assert_eq!(
            std::fs::read_to_string(temp_dir.path().join("state.json")).unwrap(),
            "{\n  \"doctor.prompt-restart-terminal\": false\n}\n"
        );
    }

    #[test]
    fn should_keep_the_other_keys_of_the_state_file_when_clearing_the_reminder() {
        let temp_dir = TempDir::new("reminder-clear-keeps");
        let state_file = state_file_with(
            &temp_dir,
            "{\"zoo\": 1, \"doctor.prompt-restart-terminal\": true, \"autocomplete.recency\": {\"git\": 3}}",
        );

        rooted_at(temp_dir.path()).clear_pending();

        assert_eq!(
            std::fs::read_to_string(&state_file).unwrap(),
            "{\n  \"autocomplete.recency\": {\n    \"git\": 3\n  },\n  \"doctor.prompt-restart-terminal\": false,\n  \"zoo\": 1\n}\n"
        );
    }

    #[test]
    fn should_leave_a_corrupt_state_file_untouched_when_clearing_the_reminder() {
        let temp_dir = TempDir::new("reminder-clear-corrupt");
        let state_file = state_file_with(&temp_dir, "{oops");

        rooted_at(temp_dir.path()).clear_pending();

        assert_eq!(std::fs::read_to_string(&state_file).unwrap(), "{oops");
    }

    #[test]
    fn should_write_nothing_without_a_data_directory_when_clearing_the_reminder() {
        let temp_dir = TempDir::new("reminder-clear-no-root");

        without_data_directory(&temp_dir).clear_pending();

        assert_eq!(std::fs::read_dir(temp_dir.path()).unwrap().count(), 0);
    }
}
