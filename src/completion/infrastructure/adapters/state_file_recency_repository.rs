use crate::completion::domain::entities::suggestion_recency::SuggestionRecency;
use crate::completion::domain::repositories::recency_repository::RecencyRepository;
use crate::shared::infrastructure::error_chain::display_chain;
use crate::shared::infrastructure::json_document_store::JsonDocumentStore;
use crate::shared::infrastructure::json_document_store_error::JsonDocumentStoreError;
use crate::shared::infrastructure::logger;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering::SeqCst};
use std::sync::{Arc, Mutex};
use std::time::Duration;

const RECENCY_KEY: &str = "autocomplete.recency";

/// `recency[cmd][name] = unix ms of the last insertion` (SPEC §3.2 `state.json`).
type RecencyRecord = HashMap<String, HashMap<String, u64>>;

/// The `RecencyRepository` over the `autocomplete.recency` key of the state file. A save writes at most once a
/// second, the latest recency saved by then, merged newest-wins with the file (other sessions write it too).
pub(crate) struct StateFileRecencyRepository {
    /// `None`: no state file (no data directory); the recency then lives in memory only.
    path: Option<PathBuf>,
    /// A write is scheduled and has not taken its snapshot of `pending` yet.
    saving: Arc<AtomicBool>,
    /// The latest recency saved: what the next write writes.
    pending: Arc<Mutex<SuggestionRecency>>,
}

impl StateFileRecencyRepository {
    /// Does no I/O: `load` reads the file.
    pub(crate) fn new(path: Option<PathBuf>) -> Self {
        StateFileRecencyRepository {
            path,
            saving: Arc::new(AtomicBool::new(false)),
            pending: Arc::new(Mutex::new(SuggestionRecency::default())),
        }
    }

    fn save_recency_now(&self) {
        let (Some(path), Ok(pending)) = (&self.path, self.pending.lock()) else {
            return;
        };
        if let Err(e) = save_recency(path, &pending.to_primitives()) {
            logger::warn(&format!("engine: saving recency: {}", display_chain(&e)));
        }
    }
}

impl RecencyRepository for StateFileRecencyRepository {
    fn load(&self) -> SuggestionRecency {
        SuggestionRecency::new(self.path.as_deref().map(load_recency).unwrap_or_default())
    }

    fn save(&self, recency: &SuggestionRecency) {
        let Some(path) = self.path.clone() else {
            return;
        };
        if let Ok(mut pending) = self.pending.lock() {
            *pending = recency.clone();
        }
        if self.saving.swap(true, SeqCst) {
            return; // a save is already scheduled; it writes the latest map
        }
        let (pending, saving) = (self.pending.clone(), self.saving.clone());
        // ponytail: throttled to one write per second (first insert + 1 s), not a trailing debounce
        let spawned = std::thread::Builder::new()
            .name("recency".into())
            .spawn(move || {
                std::thread::sleep(Duration::from_secs(1));
                saving.store(false, SeqCst);
                let snapshot = pending.lock().map(|r| r.clone()).unwrap_or_default();
                if let Err(e) = save_recency(&path, &snapshot.to_primitives()) {
                    logger::warn(&format!("engine: saving recency: {}", display_chain(&e)));
                }
            });
        if spawned.is_err() {
            self.saving.store(false, SeqCst);
            self.save_recency_now();
        }
    }

    fn flush(&self) {
        if self.saving.load(SeqCst) {
            self.save_recency_now();
        }
    }
}

/// Load `autocomplete.recency` from a state file (missing/corrupt → empty).
fn load_recency(path: &Path) -> RecencyRecord {
    JsonDocumentStore::load(path)
        .ok()
        .and_then(|s| s.map.get(RECENCY_KEY).cloned())
        .and_then(|v| serde_json::from_value(v).ok())
        .unwrap_or_default()
}

/// Read-modify-write `autocomplete.recency` into a state file (other keys kept). Merged with what is on disk
/// (newest timestamp wins), so concurrent sessions keep each other's entries. Skipped when the data root is gone
/// (`uninstall --purge` while this session runs): JsonDocumentStore::save would recreate it.
fn save_recency(path: &Path, rec: &RecencyRecord) -> Result<(), JsonDocumentStoreError> {
    if !path.parent().is_some_and(Path::is_dir) {
        return Ok(());
    }
    let mut store = JsonDocumentStore::load(path)?;
    let mut merged: RecencyRecord = store
        .map
        .get(RECENCY_KEY)
        .cloned()
        .and_then(|v| serde_json::from_value(v).ok())
        .unwrap_or_default();
    for (cmd, names) in rec {
        let m = merged.entry(cmd.clone()).or_default();
        for (name, &ts) in names {
            let e = m.entry(name.clone()).or_insert(0);
            *e = (*e).max(ts);
        }
    }
    store
        .map
        .insert(RECENCY_KEY.into(), serde_json::to_value(merged).map_err(JsonDocumentStoreError::Serialize)?);
    store.save()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shared::infrastructure::json_document_store::JsonDocumentStore;
    use crate::shared::infrastructure::testing::temp_dir::TempDir;
    use serde_json::json;

    #[test]
    fn recency_round_trip_keeps_other_keys() {
        let temp_dir = TempDir::new("engine");
        let dir = temp_dir.path().to_path_buf();
        let path = dir.join("state.json");
        std::fs::write(&path, r#"{"doctor.prompt-restart-terminal": true}"#).unwrap();
        assert!(load_recency(&path).is_empty());
        let rec: RecencyRecord = [(
            "git".to_string(),
            [("checkout".to_string(), 1_700_000_000_000u64)].into(),
        )]
        .into();
        save_recency(&path, &rec).unwrap();
        assert_eq!(load_recency(&path), rec);
        let store = JsonDocumentStore::load(&path).unwrap();
        assert_eq!(
            store.map.get("doctor.prompt-restart-terminal"),
            Some(&json!(true))
        );
        assert!(load_recency(&dir.join("missing.json")).is_empty());
        std::fs::remove_dir_all(&dir).unwrap();
        // R4-3: a purged data root is not recreated by a running session.
        save_recency(&path, &rec).unwrap();
        assert!(!dir.exists());
    }

    #[test]
    fn recency_save_merges_other_sessions() {
        // recency-cross-session-overwrite: two sessions loaded the same file; neither drops the other's entry
        let dir = TempDir::new("recency");
        let path = dir.path().join("state.json");
        std::fs::write(&path, "{}").unwrap();
        let one = |cmd: &str, name: &str, ts: u64| -> RecencyRecord {
            [(cmd.to_string(), [(name.to_string(), ts)].into())].into()
        };
        save_recency(&path, &one("git", "checkout", 5)).unwrap();
        save_recency(&path, &one("git", "status", 2)).unwrap();
        save_recency(&path, &one("git", "checkout", 3)).unwrap(); // older than disk: newest wins
        let got = load_recency(&path);
        assert_eq!(
            got["git"],
            [("checkout".to_string(), 5u64), ("status".to_string(), 2)].into()
        );
    }

    #[test]
    fn should_write_nothing_on_flush_without_a_pending_save() {
        let temp_dir = TempDir::new("recency-idle");
        let path = temp_dir.path().join("state.json");
        let repository = StateFileRecencyRepository::new(Some(path.clone()));

        repository.flush();

        assert!(!path.exists());
    }

    #[test]
    fn should_write_the_latest_recency_on_flush_after_a_save() {
        let temp_dir = TempDir::new("recency-flush");
        let path = temp_dir.path().join("state.json");
        let repository = StateFileRecencyRepository::new(Some(path.clone()));
        let mut recency = SuggestionRecency::default();
        recency.record("git".into(), "checkout".into(), 1_700_000_000_000);
        repository.save(&recency);
        recency.record("git".into(), "status".into(), 1_700_000_000_001);
        repository.save(&recency);

        repository.flush();

        assert_eq!(StateFileRecencyRepository::new(Some(path)).load(), recency);
    }

    #[test]
    fn should_keep_a_session_without_a_state_file_in_memory() {
        let repository = StateFileRecencyRepository::new(None);
        let mut recency = SuggestionRecency::default();
        recency.record("git".into(), "checkout".into(), 1_700_000_000_000);

        repository.save(&recency);
        repository.flush();

        assert_eq!(repository.load(), SuggestionRecency::default());
    }
}
