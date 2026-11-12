use crate::modules::completion::application::completion_request::CompletionRequest;
use crate::modules::history::domain::entities::history_entry::HistoryEntry;
use crate::modules::settings::domain::value_objects::settings::Settings;
use crate::modules::shared::domain::value_objects::shell::Shell;
use std::collections::HashMap;
use std::path::PathBuf;
#[cfg(test)]
use std::sync::mpsc::Sender;

/// What the terminal session tells the completion engine. Fire and forget: every call returns at once, and a gone
/// engine (one that stopped or never started) is ignored, never reported.
pub(crate) trait CompletionEngine {
    fn complete(&self, request: CompletionRequest);
    fn command_finished(&self, entry: HistoryEntry);
    /// A command finished but must not be recorded (NoHistory).
    fn command_ended(&self);
    fn record_insertion(&self, command: String, name: String);
    fn request_llm_ghost_text(&self, request: u64, buffer: String, cwd: PathBuf, shell: Shell);
    fn apply_settings(&self, settings: Settings);
    fn apply_session_context(&self, env: HashMap<String, String>, alias: Option<String>);
    fn toggle_fuzzy_search(&self);
    fn shut_down(&self);
}

/// One call made to a [`RecordingCompletionEngine`], named after its method.
#[cfg(test)]
#[derive(Debug)]
pub(crate) enum CompletionEngineCall {
    Complete(CompletionRequest),
    CommandFinished(HistoryEntry),
    CommandEnded,
    RecordInsertion { command: String, name: String },
    RequestLlmGhostText { request: u64, buffer: String, cwd: PathBuf, shell: Shell },
    ApplySettings(Settings),
    ApplySessionContext { env: HashMap<String, String>, alias: Option<String> },
    ToggleFuzzySearch,
    ShutDown,
}

/// Test double: forwards every call, in the order it was made, to the receiver of `calls`, which hands each over once
/// (`try_iter`). A dropped receiver is ignored, like a gone engine.
#[cfg(test)]
pub(crate) struct RecordingCompletionEngine {
    calls: Sender<CompletionEngineCall>,
}

#[cfg(test)]
impl RecordingCompletionEngine {
    pub(crate) fn new(calls: Sender<CompletionEngineCall>) -> Self {
        RecordingCompletionEngine { calls }
    }
}

#[cfg(test)]
impl CompletionEngine for RecordingCompletionEngine {
    fn complete(&self, request: CompletionRequest) {
        let _ = self.calls.send(CompletionEngineCall::Complete(request));
    }

    fn command_finished(&self, entry: HistoryEntry) {
        let _ = self.calls.send(CompletionEngineCall::CommandFinished(entry));
    }

    fn command_ended(&self) {
        let _ = self.calls.send(CompletionEngineCall::CommandEnded);
    }

    fn record_insertion(&self, command: String, name: String) {
        let _ = self.calls.send(CompletionEngineCall::RecordInsertion { command, name });
    }

    fn request_llm_ghost_text(&self, request: u64, buffer: String, cwd: PathBuf, shell: Shell) {
        let _ = self.calls.send(CompletionEngineCall::RequestLlmGhostText { request, buffer, cwd, shell });
    }

    fn apply_settings(&self, settings: Settings) {
        let _ = self.calls.send(CompletionEngineCall::ApplySettings(settings));
    }

    fn apply_session_context(&self, env: HashMap<String, String>, alias: Option<String>) {
        let _ = self.calls.send(CompletionEngineCall::ApplySessionContext { env, alias });
    }

    fn toggle_fuzzy_search(&self) {
        let _ = self.calls.send(CompletionEngineCall::ToggleFuzzySearch);
    }

    fn shut_down(&self) {
        let _ = self.calls.send(CompletionEngineCall::ShutDown);
    }
}

/// Test double: panics when it is asked for completions and ignores everything else.
#[cfg(test)]
pub(crate) struct PanickingCompletionEngine;

#[cfg(test)]
impl CompletionEngine for PanickingCompletionEngine {
    fn complete(&self, _request: CompletionRequest) {
        panic!("the engine was asked");
    }
    fn command_finished(&self, _entry: HistoryEntry) {}
    fn command_ended(&self) {}
    fn record_insertion(&self, _command: String, _name: String) {}
    fn request_llm_ghost_text(&self, _request: u64, _buffer: String, _cwd: PathBuf, _shell: Shell) {}
    fn apply_settings(&self, _settings: Settings) {}
    fn apply_session_context(&self, _env: HashMap<String, String>, _alias: Option<String>) {}
    fn toggle_fuzzy_search(&self) {}
    fn shut_down(&self) {}
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::history::domain::entities::history_entry::HistoryEntryPrimitives;
    use crate::modules::settings::domain::value_objects::setting_value::SettingValue;
    use std::path::Path;
    use std::sync::mpsc;

    #[test]
    fn should_record_every_call_in_the_order_it_was_made() {
        let (sender, receiver) = mpsc::channel();
        let engine = RecordingCompletionEngine::new(sender);
        let request = CompletionRequest {
            id: 7,
            buffer: "git ".into(),
            cursor: 4,
            cwd: PathBuf::from("/repo"),
            shell: Shell::Zsh,
            history_mode: true,
        };
        let entry = HistoryEntry::new(HistoryEntryPrimitives {
            command: "git status".into(),
            ..HistoryEntryPrimitives::default()
        });
        let settings = Settings::default().with("autocomplete.fuzzySearch", SettingValue::Bool(true));
        let env = HashMap::from([("PWD".to_string(), "/repo".to_string())]);

        engine.complete(request);
        engine.command_finished(entry.clone());
        engine.command_ended();
        engine.record_insertion("git".into(), "status".into());
        engine.request_llm_ghost_text(8, "git st".into(), PathBuf::from("/repo"), Shell::Bash);
        engine.apply_settings(settings.clone());
        engine.apply_session_context(env.clone(), Some("g".into()));
        engine.toggle_fuzzy_search();
        engine.shut_down();

        let calls = receiver.try_iter().collect::<Vec<_>>();
        assert!(
            matches!(
                &calls[..],
                [
                    CompletionEngineCall::Complete(recorded_request),
                    CompletionEngineCall::CommandFinished(recorded_entry),
                    CompletionEngineCall::CommandEnded,
                    CompletionEngineCall::RecordInsertion { command, name },
                    CompletionEngineCall::RequestLlmGhostText { request: 8, buffer, cwd, shell: Shell::Bash },
                    CompletionEngineCall::ApplySettings(recorded_settings),
                    CompletionEngineCall::ApplySessionContext { env: recorded_env, alias },
                    CompletionEngineCall::ToggleFuzzySearch,
                    CompletionEngineCall::ShutDown,
                ] if recorded_request.id == 7
                    && recorded_request.buffer == "git "
                    && recorded_request.cursor == 4
                    && recorded_request.cwd == Path::new("/repo")
                    && recorded_request.shell == Shell::Zsh
                    && recorded_request.history_mode
                    && *recorded_entry == entry
                    && command == "git"
                    && name == "status"
                    && buffer == "git st"
                    && cwd == Path::new("/repo")
                    && *recorded_settings == settings
                    && *recorded_env == env
                    && alias.as_deref() == Some("g")
            ),
            "{calls:?}"
        );
    }
}
