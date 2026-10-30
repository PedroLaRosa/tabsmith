//! The terminal session's `CompletionEngine` port over the engine thread's inbox, wired here because only bootstrap
//! may join modules.
use crate::bootstrap::engine_message::EngineMessage;
use crate::completion::application::completion_request::CompletionRequest;
use crate::history::domain::entities::history_entry::HistoryEntry;
use crate::settings::domain::value_objects::settings::Settings;
use crate::shared::domain::value_objects::shell::Shell;
use crate::terminal_session::application::ports::completion_engine::CompletionEngine;
use crate::types::Msg;
use std::collections::HashMap;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::mpsc::Sender;

/// Each call is one message to the engine thread, its payload unchanged. A send error (the engine stopped or never
/// started) is dropped, never logged.
pub(crate) struct ChannelCompletionEngine {
    sender: Sender<EngineMessage>,
}

/// Starts the engine thread, its replies going to `replies`, and returns the port over its inbox. The inbox keeps a
/// sender of its own, so dropping this port never ends the engine: only `shut_down` does.
pub(crate) fn start_completion_engine(replies: Sender<Msg>, settings: Settings) -> Rc<dyn CompletionEngine> {
    Rc::new(ChannelCompletionEngine {
        sender: crate::bootstrap::engine_server::spawn(replies, settings),
    })
}

impl CompletionEngine for ChannelCompletionEngine {
    fn complete(&self, request: CompletionRequest) {
        let _ = self.sender.send(EngineMessage::Complete(request));
    }

    fn command_finished(&self, entry: HistoryEntry) {
        let _ = self.sender.send(EngineMessage::CommandFinished(entry));
    }

    fn command_ended(&self) {
        let _ = self.sender.send(EngineMessage::CommandEnded);
    }

    fn record_insertion(&self, command: String, name: String) {
        let _ = self.sender.send(EngineMessage::RecordInsert { cmd: command, name });
    }

    fn request_llm_ghost_text(&self, request: u64, buffer: String, cwd: PathBuf, shell: Shell) {
        let _ = self.sender.send(EngineMessage::Llm {
            req: request,
            buffer,
            cwd,
            shell,
        });
    }

    fn apply_settings(&self, settings: Settings) {
        let _ = self.sender.send(EngineMessage::Settings(settings));
    }

    fn apply_session_context(&self, env: HashMap<String, String>, alias: Option<String>) {
        let _ = self.sender.send(EngineMessage::Session { env, alias });
    }

    fn toggle_fuzzy_search(&self) {
        let _ = self.sender.send(EngineMessage::ToggleFuzzy);
    }

    fn shut_down(&self) {
        let _ = self.sender.send(EngineMessage::Shutdown);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::history::domain::entities::history_entry::HistoryEntryPrimitives;
    use crate::settings::domain::value_objects::setting_value::SettingValue;
    use std::panic::AssertUnwindSafe;
    use std::path::Path;
    use std::sync::mpsc;

    fn request() -> CompletionRequest {
        CompletionRequest {
            id: 7,
            buffer: "git ".into(),
            cursor: 4,
            cwd: PathBuf::from("/repo"),
            shell: Shell::Zsh,
            history_mode: true,
        }
    }

    fn entry() -> HistoryEntry {
        HistoryEntry::new(HistoryEntryPrimitives {
            command: "git status".into(),
            ..HistoryEntryPrimitives::default()
        })
    }

    fn settings() -> Settings {
        Settings::default().with("autocomplete.fuzzySearch", SettingValue::Bool(true))
    }

    fn env() -> HashMap<String, String> {
        HashMap::from([("PWD".to_string(), "/repo".to_string())])
    }

    /// Calls every method once, in the port's order.
    fn make_every_call(engine: &ChannelCompletionEngine) {
        engine.complete(request());
        engine.command_finished(entry());
        engine.command_ended();
        engine.record_insertion("git".into(), "status".into());
        engine.request_llm_ghost_text(8, "git st".into(), PathBuf::from("/repo"), Shell::Bash);
        engine.apply_settings(settings());
        engine.apply_session_context(env(), Some("g".into()));
        engine.toggle_fuzzy_search();
        engine.shut_down();
    }

    #[test]
    fn should_send_each_call_as_its_engine_message() {
        let (sender, receiver) = mpsc::channel();
        let engine = ChannelCompletionEngine { sender };

        make_every_call(&engine);

        let messages = receiver.try_iter().collect::<Vec<_>>();
        assert!(matches!(
            &messages[..],
            [
                EngineMessage::Complete(sent_request),
                EngineMessage::CommandFinished(sent_entry),
                EngineMessage::CommandEnded,
                EngineMessage::RecordInsert { cmd, name },
                EngineMessage::Llm { req: 8, buffer, cwd, shell: Shell::Bash },
                EngineMessage::Settings(sent_settings),
                EngineMessage::Session { env: sent_env, alias },
                EngineMessage::ToggleFuzzy,
                EngineMessage::Shutdown,
            ] if sent_request.id == 7
                && sent_request.buffer == "git "
                && sent_request.cursor == 4
                && sent_request.cwd == Path::new("/repo")
                && sent_request.shell == Shell::Zsh
                && sent_request.history_mode
                && *sent_entry == entry()
                && cmd == "git"
                && name == "status"
                && buffer == "git st"
                && cwd == Path::new("/repo")
                && *sent_settings == settings()
                && *sent_env == env()
                && alias.as_deref() == Some("g")
        ));
    }

    #[test]
    fn should_ignore_an_engine_that_has_stopped() {
        let (sender, receiver) = mpsc::channel();
        drop(receiver);
        let engine = ChannelCompletionEngine { sender };

        let calls = std::panic::catch_unwind(AssertUnwindSafe(|| make_every_call(&engine)));

        assert!(calls.is_ok());
    }
}
