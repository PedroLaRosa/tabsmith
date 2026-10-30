//! The completion engine thread's server (SPEC §4.3, §6). `spawn` builds the inbox, the spec runtime and the modules
//! on that thread; the server owns the inbox instance and dispatches each `EngineMessage` to the completion use cases
//! and the legacy ghost text block (the engine-server event table, plan §4). Only the latest request is served; while
//! generators run, a request is replied again whenever one of its generators finishes (phase `Generators`).
use crate::ghost_text::domain::services::ghost_text_source_resolver;
use crate::ghost_text::domain::value_objects::ghost_text_source::GhostTextSource;
use crate::inline::{self, GhostCache};
use crate::shared::infrastructure::logger;
use crate::shared::infrastructure::shared_dependencies::SharedDependencies;
use crate::completion::application::completion_notice::CompletionNotice;
use crate::completion::domain::entities::completion_session::CompletionSession;
use crate::completion::infrastructure::completion_module::{build_completion_module, CompletionModule};
use crate::settings::domain::services::settings_catalog::{INLINE_ENABLED, INLINE_HISTORY_COUNT};
use crate::settings::domain::value_objects::settings::Settings;
use crate::completion::application::completion_request::CompletionRequest;
use crate::completion::domain::value_objects::completion_result::CompletionResult;
use crate::specs::domain::value_objects::spec_index::SpecIndex;
use crate::specs::infrastructure::adapters::embedded_spec_bundle::EmbeddedSpecBundle;
use crate::specs::infrastructure::adapters::quickjs_spec_repository::QuickJsSpecRepository;
use crate::specs::infrastructure::adapters::quickjs_spec_runtime::QuickJsSpecRuntime;
use crate::specs::infrastructure::specs_module::build_specs_module;
use crate::shared::domain::value_objects::shell::Shell;
use crate::history::domain::entities::history_entry::HistoryEntry;
use crate::history::domain::repositories::history_repository::HistoryRepository;
use crate::history::application::record_command_use_case::RecordCommandUseCase;
use crate::history::infrastructure::history_module::{build_history_module, HistoryModule};
use crate::bootstrap::engine_inbox::EngineInbox;
use crate::bootstrap::engine_message::EngineMessage;
use crate::types::Msg;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::mpsc::{self, Sender};
use std::sync::{Arc, Mutex};

/// Spawn the engine thread. Replies go to `tx` as `Msg::Completion` / `Msg::Ghost`.
/// Returns the engine's inbox. The thread exits on `EngineMessage::Shutdown`; the inbox keeps a sender, so its
/// channel never disconnects.
pub(crate) fn spawn(tx: Sender<Msg>, settings: Settings) -> Sender<EngineMessage> {
    let (etx, erx) = mpsc::channel();
    let inner = etx.clone();
    let spawned = std::thread::Builder::new()
        .name("engine".into())
        .spawn(move || {
            // QuickJS is !Send: the inbox, the runtime and everything holding them are created on this thread.
            let inbox = Rc::new(EngineInbox::new(erx, inner));
            let runtime = match QuickJsSpecRuntime::new(inbox.clone(), &settings) {
                Ok(r) => Rc::new(r),
                Err(e) => {
                    logger::error(&format!("engine: js host: {e:#}"));
                    return;
                }
            };
            let dependencies = SharedDependencies::from_env();
            let repository = Rc::new(QuickJsSpecRepository::new(
                runtime.clone(),
                EmbeddedSpecBundle::new(dependencies.data_directory.clone()),
            ));
            let bundle = EmbeddedSpecBundle::new(dependencies.data_directory.clone());
            // ponytail: load_index ensures the bundle a second time (touch + cleanup twice), as the engine always did.
            let index = build_specs_module(&dependencies)
                .install_spec_bundle
                .execute()
                .and_then(|()| bundle.load_index())
                .unwrap_or_else(|e| {
                    logger::error(&format!("engine: specs: {e:#}"));
                    SpecIndex::default()
                });
            let history_module = build_history_module(&dependencies);
            let completion = build_completion_module(
                &dependencies,
                repository,
                runtime.clone(),
                history_module.history.clone(),
                index,
            );
            let recency = completion.recency.load();
            let session = CompletionSession::new(settings, std::env::vars().collect(), recency);
            EngineServer::new(tx, session, completion, history_module).run(&runtime, &inbox);
        });
    if let Err(e) = spawned {
        logger::error(&format!("engine: spawn: {e}"));
    }
    etx
}

struct EngineServer {
    replies: Sender<Msg>,
    session: CompletionSession,
    completion: CompletionModule,
    record_command: RecordCommandUseCase,
    history: Rc<dyn HistoryRepository>,
    ghost_cache: Arc<Mutex<GhostCache>>,
}

impl EngineServer {
    fn new(replies: Sender<Msg>, session: CompletionSession, completion: CompletionModule, history_module: HistoryModule) -> EngineServer {
        let HistoryModule {
            history,
            record_command,
        } = history_module;
        EngineServer {
            replies,
            session,
            completion,
            record_command,
            history,
            ghost_cache: Arc::new(Mutex::new(GhostCache::default())),
        }
    }

    fn run(mut self, js: &QuickJsSpecRuntime, inbox: &EngineInbox) {
        let mut pending: Option<CompletionRequest> = None;
        'outer: loop {
            if pending.is_none() && !self.session.has_running_generators() {
                match inbox.next_msg(js) {
                    Some(m) => {
                        if !self.handle(m, &mut pending) {
                            break;
                        }
                    }
                    None => break,
                }
            }
            // only the latest request is served: drain everything queued first
            while let Some(m) = inbox.try_next_msg(js) {
                if !self.handle(m, &mut pending) {
                    break 'outer;
                }
            }
            if let Some(req) = pending.take() {
                if let Some(result) =
                    self.completion.complete_command_line.execute(&mut self.session, &req, &mut |n| Self::log_notice(n))
                {
                    self.send_reply(result);
                }
                continue;
            }
            if self.session.has_running_generators() {
                if let Some(result) =
                    self.completion.progress_generators.execute(&mut self.session, &mut |n| Self::log_notice(n))
                {
                    self.send_reply(result);
                }
            }
        }
        self.completion.recency.flush();
    }

    /// Handle one message; `false` = shut down.
    fn handle(&mut self, m: EngineMessage, pending: &mut Option<CompletionRequest>) -> bool {
        match m {
            EngineMessage::Complete(r) => *pending = Some(r),
            EngineMessage::CommandFinished(_) | EngineMessage::CommandEnded => {
                if let EngineMessage::CommandFinished(e) = m {
                    self.record_command.execute(e);
                }
                self.clear_ghost_text_cache();
                // generator results without `cache` live until the command ends
                self.completion.reset_generators.execute(&mut self.session);
            }
            EngineMessage::RecordInsert { cmd, name } => {
                self.completion.record_suggestion_insertion.execute(&mut self.session, cmd, name)
            }
            EngineMessage::Llm {
                req,
                buffer,
                cwd,
                shell,
            } => self.request_llm_ghost_text(req, buffer, cwd, shell),
            EngineMessage::Settings(s) => self.completion.apply_completion_settings.execute(&mut self.session, s),
            EngineMessage::Session { env, alias } => {
                self.completion.apply_session_context.execute(&mut self.session, env, alias)
            }
            EngineMessage::ToggleFuzzy => self.completion.toggle_fuzzy_search.execute(&mut self.session),
            EngineMessage::Runtime(_) => {} // handed to the js host by the inbox
            EngineMessage::Shutdown => return false,
        }
        true
    }

    /// The one place a completion result is sent: with the local ghost text for its buffer (Q15).
    fn send_reply(&self, result: CompletionResult) {
        let ghost = self.local_ghost_text(&result);
        let _ = self.replies.send(Msg::Completion(Box::new(result.with_ghost(ghost))));
    }

    /// Logs what the completion services report, the moment they report it (D5).
    fn log_notice(notice: CompletionNotice) {
        match notice {
            CompletionNotice::GenerateSpecFailed(line) => logger::warn(&line),
            CompletionNotice::GeneratorFailed(e) => logger::debug(&format!("engine: generator failed: {e}")),
            CompletionNotice::NoSuggestions(e) => logger::debug(&format!("engine: no suggestions: {e}")),
        }
    }
}

// legacy ghost text: Phase 7.6 replaces this block
impl EngineServer {
    /// Local inline ghost (SPEC §6.11) of a reply; error replies (commands without a spec) get one too.
    fn local_ghost_text(&self, result: &CompletionResult) -> Option<String> {
        if !self.session.settings().bool(INLINE_ENABLED)
            || result.buffer().is_empty()
            || result.cursor() != result.buffer().len()
        {
            return None;
        }
        let history = || inline::history_ghost(&self.history.recent(), result.buffer());
        let specs = || inline::spec_ghost(result.suggestions());
        match ghost_text_source_resolver::source(self.session.settings()) {
            Ok(GhostTextSource::Specs) => specs(),
            Ok(GhostTextSource::History) | Ok(GhostTextSource::Profile(_)) => history(),
            _ => history().or_else(specs),
        }
    }

    /// LLM ghost text (SPEC §8.4).
    fn request_llm_ghost_text(&mut self, req: u64, buffer: String, cwd: PathBuf, shell: Shell) {
        let tx = self.replies.clone();
        let Ok(GhostTextSource::Profile(profile)) = ghost_text_source_resolver::source(self.session.settings()) else {
            let _ = tx.send(Msg::Ghost { req, text: None });
            return;
        };
        if let Some(text) = self.ghost_cache.lock().ok().and_then(|c| c.lookup(&buffer)) {
            let _ = tx.send(Msg::Ghost {
                req,
                text: Some(text),
            });
            return;
        }
        let history = llm_history(self.session.settings(), &self.history.recent());
        let env = self.session.env().clone();
        let cache = self.ghost_cache.clone();
        let spawned = std::thread::Builder::new()
            .name("llm".into())
            .spawn(move || {
                let text = inline::llm_complete(&profile, &buffer, &cwd, shell, &history, &env);
                if let (Some(t), Ok(mut c)) = (&text, cache.lock()) {
                    c.insert(&[format!("{buffer}{t}")]);
                }
                let _ = tx.send(Msg::Ghost { req, text });
            });
        if let Err(e) = spawned {
            logger::warn(&format!("engine: llm worker: {e}"));
        }
    }

    /// A finished or ended command makes the LLM ghost texts of its line stale.
    fn clear_ghost_text_cache(&self) {
        if let Ok(mut c) = self.ghost_cache.lock() {
            c.clear();
        }
    }
}

/// The history an LLM request may send: the last `inline.historyCount` (49) usable entries (SPEC §8.5).
fn llm_history(settings: &Settings, entries: &[HistoryEntry]) -> Vec<HistoryEntry> {
    inline::llm_history(entries, settings.i64(INLINE_HISTORY_COUNT).max(0) as usize)
}

#[cfg(test)]
mod tests;
