use crate::completion::application::completion_request::CompletionRequest;
use crate::history::domain::entities::history_entry::HistoryEntry;
use crate::settings::domain::value_objects::settings::Settings;
use crate::shared::domain::value_objects::shell::Shell;
use crate::specs::application::ports::runtime_event::RuntimeEvent;
use std::collections::HashMap;
use std::path::PathBuf;

/// Messages to the engine thread.
pub(crate) enum EngineMessage {
    Complete(CompletionRequest),
    CommandFinished(HistoryEntry),
    /// A command finished but must not be recorded (NoHistory).
    CommandEnded,
    RecordInsert {
        cmd: String,
        name: String,
    },
    Llm {
        req: u64,
        buffer: String,
        cwd: PathBuf,
        shell: Shell,
    },
    Settings(Settings),
    Session {
        env: HashMap<String, String>,
        alias: Option<String>,
    },
    ToggleFuzzy,
    /// Internal (js host): an event for the spec runtime, e.g. a spawned process finished.
    Runtime(RuntimeEvent),
    Shutdown,
}
