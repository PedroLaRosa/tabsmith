//! The completion engine thread's server (SPEC §4.3, §6). `spawn` builds the inbox, the spec runtime and the modules
//! on that thread; the server owns the inbox instance and dispatches each `EngineMessage` to the completion use cases
//! and the ghost_text use cases (the engine-server event table, plan §4). Only the latest request is served; while
//! generators run, a request is replied again whenever one of its generators finishes (phase `Generators`).
use crate::modules::ghost_text::application::llm_ghost_text_answer::LlmGhostTextAnswer;
use crate::modules::ghost_text::application::llm_ghost_text_request::LlmGhostTextRequest;
use crate::modules::ghost_text::infrastructure::ghost_text_module::{build_ghost_text_module, GhostTextModule};
use crate::modules::shared::infrastructure::logger;
use crate::modules::shared::infrastructure::shared_dependencies::SharedDependencies;
use crate::modules::completion::application::completion_notice::CompletionNotice;
use crate::modules::completion::domain::entities::completion_session::CompletionSession;
use crate::modules::completion::infrastructure::completion_module::{build_completion_module, CompletionModule};
use crate::modules::settings::domain::value_objects::settings::Settings;
use crate::modules::completion::application::completion_request::CompletionRequest;
use crate::modules::completion::domain::value_objects::completion_result::CompletionResult;
use crate::modules::specs::domain::value_objects::spec_index::SpecIndex;
use crate::modules::specs::infrastructure::adapters::embedded_spec_bundle::EmbeddedSpecBundle;
use crate::modules::specs::infrastructure::adapters::quickjs_spec_repository::QuickJsSpecRepository;
use crate::modules::specs::infrastructure::adapters::quickjs_spec_runtime::QuickJsSpecRuntime;
use crate::modules::specs::infrastructure::specs_module::build_specs_module;
use crate::modules::history::application::record_command_use_case::RecordCommandUseCase;
use crate::modules::history::infrastructure::history_module::{build_history_module, HistoryModule};
use crate::bootstrap::engine_inbox::EngineInbox;
use crate::bootstrap::engine_message::EngineMessage;
use crate::modules::terminal_session::infrastructure::pty::session_message::SessionMessage;
use std::rc::Rc;
use std::sync::mpsc::{self, Sender};

/// Spawn the engine thread. Replies go to `tx` as `SessionMessage::Completion` / `SessionMessage::Ghost`.
/// Returns the engine's inbox. The thread exits on `EngineMessage::Shutdown`; the inbox keeps a sender, so its
/// channel never disconnects.
pub(crate) fn spawn(tx: Sender<SessionMessage>, settings: Settings) -> Sender<EngineMessage> {
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
            let ghost_text = build_ghost_text_module(Rc::clone(&history_module.history));
            let completion = build_completion_module(
                &dependencies,
                repository,
                runtime.clone(),
                history_module.history.clone(),
                index,
            );
            let recency = completion.recency.load();
            let session = CompletionSession::new(settings, std::env::vars().collect(), recency);
            EngineServer::new(tx, session, completion, history_module, ghost_text).run(&runtime, &inbox);
        });
    if let Err(e) = spawned {
        logger::error(&format!("engine: spawn: {e}"));
    }
    etx
}

struct EngineServer {
    replies: Sender<SessionMessage>,
    session: CompletionSession,
    completion: CompletionModule,
    record_command: RecordCommandUseCase,
    ghost_text: GhostTextModule,
}

impl EngineServer {
    fn new(
        replies: Sender<SessionMessage>,
        session: CompletionSession,
        completion: CompletionModule,
        history_module: HistoryModule,
        ghost_text: GhostTextModule,
    ) -> EngineServer {
        let HistoryModule { record_command, .. } = history_module;
        EngineServer {
            replies,
            session,
            completion,
            record_command,
            ghost_text,
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
                self.ghost_text.clear_ghost_text_cache.execute();
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
            } => self.answer_llm_ghost_text_request(req, LlmGhostTextRequest { buffer, cwd, shell }),
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
        let ghost = self.ghost_text.suggest_local_ghost_text.execute(self.session.settings(), &result);
        let _ = self.replies.send(SessionMessage::Completion(Box::new(result.with_ghost(ghost))));
    }

    /// LLM ghost text (SPEC §8.4): answered at once, or by the `llm` thread running the job.
    fn answer_llm_ghost_text_request(&self, req: u64, request: LlmGhostTextRequest) {
        let answer = self.ghost_text.request_llm_ghost_text.execute(
            self.session.settings(),
            self.session.env(),
            request,
        );
        let tx = self.replies.clone();
        match answer {
            LlmGhostTextAnswer::Immediate(text) => {
                let _ = tx.send(SessionMessage::Ghost { req, text });
            }
            LlmGhostTextAnswer::Deferred(job) => {
                let spawned = std::thread::Builder::new()
                    .name("llm".into())
                    .spawn(move || {
                        let text = job.run();
                        let _ = tx.send(SessionMessage::Ghost { req, text });
                    });
                if let Err(e) = spawned {
                    logger::warn(&format!("engine: llm worker: {e}"));
                }
            }
        }
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

#[cfg(test)]
mod tests;
