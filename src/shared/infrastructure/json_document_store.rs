use crate::shared::infrastructure::atomic_file::AtomicFile;
use crate::shared::infrastructure::json_document_store_error::JsonDocumentStoreError;
use serde_json::{Map, Value};
use std::path::{Path, PathBuf};

/// A flat JSON object file (`settings.json`, `state.json`): literal dotted keys → arbitrary JSON.
pub(crate) struct JsonDocumentStore {
    pub(crate) path: PathBuf,
    pub(crate) map: Map<String, Value>,
}

impl JsonDocumentStore {
    /// Missing file → empty map. Parse error → Err.
    pub(crate) fn load(path: &Path) -> Result<JsonDocumentStore, JsonDocumentStoreError> {
        let map = match std::fs::read_to_string(path) {
            Ok(s) if s.trim().is_empty() => Map::new(),
            Ok(s) => match serde_json::from_str::<Value>(&s)
                .map_err(|source| JsonDocumentStoreError::Corrupted { path: path.to_path_buf(), source })?
            {
                Value::Object(m) => m,
                _ => return Err(JsonDocumentStoreError::NotAnObject { path: path.to_path_buf() }),
            },
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Map::new(),
            Err(source) => return Err(JsonDocumentStoreError::Read { path: path.to_path_buf(), source }),
        };
        Ok(JsonDocumentStore {
            path: path.to_path_buf(),
            map,
        })
    }

    /// Pretty JSON (keys sorted) plus a trailing newline, written with [`AtomicFile::write_document`] (a symlinked
    /// file stays a symlink; Unix mode 0600).
    pub(crate) fn save(&self) -> Result<(), JsonDocumentStoreError> {
        let mut body = serde_json::to_string_pretty(&Value::Object(self.map.clone()))
            .map_err(JsonDocumentStoreError::Serialize)?;
        body.push('\n');
        AtomicFile::new(&self.path)
            .write_document(body.as_bytes())
            .map_err(JsonDocumentStoreError::Write)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shared::infrastructure::testing::temp_dir::TempDir;

    #[test]
    fn should_load_a_missing_file_as_an_empty_document() {
        let temp_dir = TempDir::new("document-missing");
        let path = temp_dir.path().join("state.json");

        let document = JsonDocumentStore::load(&path).unwrap();

        assert!(document.map.is_empty());
    }
}
