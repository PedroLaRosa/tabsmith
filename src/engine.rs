//! The completion engine thread (SPEC §4.3, §6): owns the JS host, spec/parse caches, generator state,
//! recency, history and the inline cache. Serves only the latest request.
//! OWNER: engine agent.
//!
//! Per request: mid-word guard → `command_line_tokenizer::current_command` → `CommandLineParser::parse` → generator triggers →
//! reply (phase `Parse`). Triggered generators run as small state machines over `SpecRuntime` tasks;
//! whenever one finishes, the latest request is re-ranked and replied again (phase `Generators`).
use crate::inline::{self, GhostCache, Source};
use crate::shared::infrastructure::logger;
use crate::shared::infrastructure::shared_dependencies::SharedDependencies;
use crate::shared::infrastructure::system_clock::SystemClock;
use crate::shared::application::ports::clock::Clock;
use crate::completion::domain::completion_error::CompletionError;
use crate::completion::domain::value_objects::parsed_command_line::ParsedCommandLine;
use crate::completion::application::apply_completion_settings_use_case::ApplyCompletionSettingsUseCase;
use crate::completion::application::apply_session_context_use_case::ApplySessionContextUseCase;
use crate::completion::application::command_line_parser::CommandLineParser;
use crate::completion::application::completion_notice::CompletionNotice;
use crate::completion::application::generator_runner::GeneratorRunner;
use crate::completion::application::parse_context::ParseContext;
use crate::completion::application::record_suggestion_insertion_use_case::RecordSuggestionInsertionUseCase;
use crate::completion::application::reset_generators_use_case::ResetGeneratorsUseCase;
use crate::completion::application::toggle_fuzzy_search_use_case::ToggleFuzzySearchUseCase;
use crate::completion::application::ports::executable_catalog::ExecutableCatalog;
use crate::completion::domain::entities::completion_session::CompletionSession;
use crate::completion::domain::repositories::recency_repository::RecencyRepository;
use crate::completion::infrastructure::adapters::fs_directory_lister::FsDirectoryLister;
use crate::completion::infrastructure::adapters::path_executable_catalog::PathExecutableCatalog;
use crate::completion::infrastructure::adapters::state_file_recency_repository::StateFileRecencyRepository;
use crate::settings::domain::services::settings_catalog::{
    AUTOCOMPLETE_ALWAYS_SUGGEST_CURRENT_TOKEN, AUTOCOMPLETE_FIRST_TOKEN_COMPLETION,
    AUTOCOMPLETE_HIDE_AUTO_EXECUTE_SUGGESTION,
    AUTOCOMPLETE_IMMEDIATELY_EXECUTE_AFTER_SPACE, AUTOCOMPLETE_IMMEDIATELY_RUN_DANGEROUS_COMMANDS,
    AUTOCOMPLETE_INSERT_SPACE_AUTOMATICALLY, AUTOCOMPLETE_ONLY_SHOW_ON_TAB,
    AUTOCOMPLETE_PREFER_VERBOSE_SUGGESTIONS, AUTOCOMPLETE_SORT_METHOD,
    BETA_HISTORY_MODE, INLINE_ENABLED, INLINE_HISTORY_COUNT,
};
use crate::settings::domain::value_objects::settings::Settings;
use crate::completion::application::completion_request::CompletionRequest;
use crate::completion::domain::services::command_line_tokenizer;
use crate::completion::domain::services::first_token_suggestions;
use crate::completion::domain::services::generator_trigger_policy::{is_path_template, SYNC_CALL};
use crate::completion::domain::services::history_suggestions;
use crate::completion::domain::services::suggestion_assembler;
use crate::completion::domain::services::suggestion_finalizer;
use crate::completion::domain::services::suggestion_ranker;
use crate::completion::domain::value_objects::argument_hint::ArgumentHint;
use crate::completion::domain::value_objects::completion_phase::CompletionPhase;
use crate::completion::domain::value_objects::completion_result::{CompletionResult, CompletionResultParts};
use crate::completion::domain::value_objects::generator_key::argument_key;
use crate::completion::domain::value_objects::suggestion::Suggestion;
use crate::specs::application::ports::runtime_event::RuntimeEvent;
use crate::specs::application::ports::runtime_event_poll::RuntimeEventPoll;
use crate::specs::application::ports::runtime_event_sink::RuntimeEventSink;
use crate::specs::application::ports::runtime_event_source::RuntimeEventSource;
use crate::specs::application::ports::spec_runtime::SpecRuntime;
use crate::specs::domain::spec_error::SpecError;
use crate::specs::domain::value_objects::filter_strategy::FilterStrategy;
use crate::specs::domain::value_objects::generator_context::GeneratorContext;
use crate::specs::domain::value_objects::spec_function_call::SpecFunctionCall;
use crate::specs::domain::value_objects::spec_function_result::SpecFunctionResult;
use crate::specs::domain::value_objects::text_or_function::TextOrFunction;
use crate::specs::domain::value_objects::spec_index::SpecIndex;
use crate::specs::infrastructure::adapters::embedded_spec_bundle::EmbeddedSpecBundle;
use crate::specs::infrastructure::adapters::quickjs_spec_repository::QuickJsSpecRepository;
use crate::specs::infrastructure::adapters::quickjs_spec_runtime::QuickJsSpecRuntime;
use crate::specs::infrastructure::specs_module::build_specs_module;
use crate::completion::domain::value_objects::filter_options::FilterOptions;
use crate::completion::domain::value_objects::insertion_context::InsertionContext;
use crate::completion::domain::value_objects::served_request::{ServedRequest, ServedRequestParts};
use crate::completion::domain::value_objects::suggestion_candidate::SuggestionCandidate;
use crate::completion::domain::value_objects::suggestion_query::SuggestionQuery;
use crate::shared::domain::value_objects::shell::Shell;
use crate::history::domain::entities::history_entry::HistoryEntry;
use crate::history::domain::repositories::history_repository::HistoryRepository;
use crate::history::application::record_command_use_case::RecordCommandUseCase;
use crate::history::infrastructure::history_module::{build_history_module, HistoryModule};
use crate::types::{
    EngineMsg, Msg,
};
use std::cell::RefCell;
use std::collections::{HashMap, VecDeque};
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// Spawn the engine thread. Replies go to `tx` as `Msg::Completion` / `Msg::Ghost`.
/// Returns the engine's inbox. The thread exits on `EngineMsg::Shutdown` or when all senders are dropped.
pub fn spawn(tx: Sender<Msg>, settings: Settings) -> Sender<EngineMsg> {
    let (etx, erx) = mpsc::channel();
    let inner = etx.clone();
    let spawned = std::thread::Builder::new()
        .name("engine".into())
        .spawn(move || {
            // QuickJS is !Send: the inbox and the runtime are created on this thread.
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
            let command_line_parser = CommandLineParser::new(repository, runtime.clone());
            Engine::new(tx, settings, command_line_parser, runtime.clone(), &dependencies).run(&runtime, &inbox);
        });
    if let Err(e) = spawned {
        logger::error(&format!("engine: spawn: {e}"));
    }
    etx
}

/// The engine's inbox: one channel for the engine's messages and the spec runtime's events. While the runtime
/// waits it polls the inbox, which keeps every other message, in arrival order, for the engine loop.
struct EngineInbox {
    receiver: Receiver<EngineMsg>,
    sender: Sender<EngineMsg>,
    deferred: RefCell<VecDeque<EngineMsg>>,
}

impl EngineInbox {
    fn new(receiver: Receiver<EngineMsg>, sender: Sender<EngineMsg>) -> Self {
        EngineInbox {
            receiver,
            sender,
            deferred: RefCell::new(VecDeque::new()),
        }
    }

    /// Next message for the engine loop: deferred messages first (incl. a request that aborted a wait), then a
    /// blocking receive. `Runtime` events are handed to the runtime at once. `None` when all senders are gone.
    fn next_msg(&self, runtime: &QuickJsSpecRuntime) -> Option<EngineMsg> {
        loop {
            if let Some(m) = self.deferred.borrow_mut().pop_front() {
                return Some(m);
            }
            let m = self.receiver.recv().ok()?;
            if let Some(m) = Self::deliver_runtime_event(runtime, m) {
                return Some(m);
            }
        }
    }

    /// Like `next_msg` but non-blocking.
    fn try_next_msg(&self, runtime: &QuickJsSpecRuntime) -> Option<EngineMsg> {
        loop {
            if let Some(m) = self.deferred.borrow_mut().pop_front() {
                return Some(m);
            }
            let m = self.receiver.try_recv().ok()?;
            if let Some(m) = Self::deliver_runtime_event(runtime, m) {
                return Some(m);
            }
        }
    }

    /// Hand a `Runtime` event to the runtime; return any other message.
    fn deliver_runtime_event(runtime: &QuickJsSpecRuntime, m: EngineMsg) -> Option<EngineMsg> {
        match m {
            EngineMsg::Runtime(event) => runtime.handle_event(event),
            m => return Some(m),
        }
        None
    }
}

impl RuntimeEventSource for EngineInbox {
    fn sink(&self) -> RuntimeEventSink {
        let sender = self.sender.clone();
        Arc::new(move |event: RuntimeEvent| {
            let _ = sender.send(EngineMsg::Runtime(event));
        })
    }

    /// Looks only at the kept messages, never at the channel: a `Complete` still unread there aborts nothing.
    fn has_newer_request(&self) -> bool {
        self.deferred
            .borrow()
            .iter()
            .any(|m| matches!(m, EngineMsg::Complete(_)))
    }

    fn poll(&self, timeout: Duration) -> RuntimeEventPoll {
        match self.receiver.recv_timeout(timeout) {
            Ok(EngineMsg::Runtime(event)) => RuntimeEventPoll::Event(event),
            Ok(m) => {
                self.deferred.borrow_mut().push_back(m); // the newer-request check at the top of a wait aborts
                RuntimeEventPoll::MessageKept
            }
            Err(RecvTimeoutError::Timeout) => RuntimeEventPoll::TimedOut,
            Err(RecvTimeoutError::Disconnected) => RuntimeEventPoll::Disconnected,
        }
    }
}

fn env_var(env: &HashMap<String, String>, key: &str) -> Option<String> {
    env.get(key)
        .or_else(|| {
            if cfg!(windows) {
                env.iter()
                    .find(|(k, _)| k.eq_ignore_ascii_case(key))
                    .map(|(_, v)| v)
            } else {
                None
            }
        })
        .cloned()
}

/// The history an LLM request may send: the last `inline.historyCount` (49) usable entries (SPEC §8.5).
fn llm_history(settings: &Settings, entries: &[HistoryEntry]) -> Vec<HistoryEntry> {
    inline::llm_history(entries, settings.i64(INLINE_HISTORY_COUNT).max(0) as usize)
}

/// Logs what the completion services report, the moment they report it (D5).
fn log_completion_notice(notice: CompletionNotice) {
    match notice {
        CompletionNotice::GenerateSpecFailed(line) => logger::warn(&line),
        CompletionNotice::GeneratorFailed(e) => logger::debug(&format!("engine: generator failed: {e}")),
        CompletionNotice::NoSuggestions(e) => logger::debug(&format!("engine: no suggestions: {e}")),
    }
}

struct Engine {
    tx: Sender<Msg>,
    command_line_parser: Rc<CommandLineParser>,
    index: SpecIndex,
    history: Rc<dyn HistoryRepository>,
    record_command: RecordCommandUseCase,
    recency_repository: Rc<dyn RecencyRepository>,
    ghost_cache: Arc<Mutex<GhostCache>>,
    generators: Rc<GeneratorRunner>,
    apply_completion_settings: ApplyCompletionSettingsUseCase,
    record_suggestion_insertion: RecordSuggestionInsertionUseCase,
    session: CompletionSession,
}

impl Engine {
    fn new(
        tx: Sender<Msg>,
        settings: Settings,
        command_line_parser: CommandLineParser,
        runtime: Rc<dyn SpecRuntime>,
        dependencies: &SharedDependencies,
    ) -> Engine {
        let bundle = EmbeddedSpecBundle::new(dependencies.data_directory.clone());
        // ponytail: load_index ensures the bundle a second time (touch + cleanup twice), as the engine always did.
        let index = build_specs_module(dependencies)
            .install_spec_bundle
            .execute()
            .and_then(|()| bundle.load_index())
            .unwrap_or_else(|e| {
                logger::error(&format!("engine: specs: {e:#}"));
                SpecIndex::default()
            });
        Engine::with(
            tx,
            settings,
            command_line_parser,
            runtime,
            index,
            build_history_module(dependencies),
            dependencies.data_directory.state_file().ok(),
        )
    }

    fn with(
        tx: Sender<Msg>,
        settings: Settings,
        command_line_parser: CommandLineParser,
        runtime: Rc<dyn SpecRuntime>,
        index: SpecIndex,
        history_module: HistoryModule,
        state_path: Option<PathBuf>,
    ) -> Engine {
        let HistoryModule {
            history,
            record_command,
        } = history_module;
        let recency_repository: Rc<dyn RecencyRepository> = Rc::new(StateFileRecencyRepository::new(state_path));
        let recency = recency_repository.load();
        let clock: Arc<dyn Clock + Send + Sync> = Arc::new(SystemClock);
        let command_line_parser = Rc::new(command_line_parser);
        let apply_completion_settings = ApplyCompletionSettingsUseCase::new(runtime.clone(), command_line_parser.clone());
        let record_suggestion_insertion = RecordSuggestionInsertionUseCase::new(recency_repository.clone(), clock.clone());
        let generators = Rc::new(GeneratorRunner::new(runtime, Rc::new(FsDirectoryLister), clock));
        Engine {
            tx,
            command_line_parser,
            index,
            history,
            record_command,
            recency_repository,
            ghost_cache: Arc::new(Mutex::new(GhostCache::default())),
            generators,
            apply_completion_settings,
            record_suggestion_insertion,
            session: CompletionSession::new(settings, std::env::vars().collect(), recency),
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
                if let Some(c) = self.compute(&req, js) {
                    let _ = self.tx.send(Msg::Completion(Box::new(c)));
                }
                continue;
            }
            if self.session.has_running_generators() {
                self.pump();
            }
        }
        self.recency_repository.flush();
    }

    /// Handle one message; `false` = shut down.
    fn handle(&mut self, m: EngineMsg, pending: &mut Option<CompletionRequest>) -> bool {
        match m {
            EngineMsg::Complete(r) => *pending = Some(r),
            EngineMsg::CommandFinished(_) | EngineMsg::CommandEnded => {
                if let EngineMsg::CommandFinished(e) = m {
                    self.record_command.execute(e);
                }
                if let Ok(mut c) = self.ghost_cache.lock() {
                    c.clear();
                }
                // generator results without `cache` live until the command ends
                ResetGeneratorsUseCase.execute(&mut self.session);
            }
            EngineMsg::RecordInsert { cmd, name } => self.record_suggestion_insertion.execute(&mut self.session, cmd, name),
            EngineMsg::Llm {
                req,
                buffer,
                cwd,
                shell,
            } => self.llm(req, buffer, cwd, shell),
            EngineMsg::Settings(s) => self.apply_completion_settings.execute(&mut self.session, s),
            EngineMsg::Session { env, alias } => ApplySessionContextUseCase.execute(&mut self.session, env, alias),
            EngineMsg::ToggleFuzzy => ToggleFuzzySearchUseCase.execute(&mut self.session),
            EngineMsg::Runtime(_) => {} // handed to the js host by the inbox
            EngineMsg::Shutdown => return false,
        }
        true
    }

    // -----------------------------------------------------------------------------------------------------
    // Requests

    /// Phase-Parse reply for `req`; `None` when a newer request aborted it (it is kept in the engine inbox).
    fn compute(&mut self, req: &CompletionRequest, js: &QuickJsSpecRuntime) -> Option<CompletionResult> {
        let cursor = req.cursor.min(req.buffer.len());
        // SPEC §6.1.5 mid-word guard
        if !req.buffer.is_char_boundary(cursor)
            || req.buffer[cursor..]
                .chars()
                .next()
                .is_some_and(|c| !c.is_whitespace())
        {
            return Some(self.error(req, cursor, false));
        }
        let before = &req.buffer[..cursor];
        let aliases = self.session.alias_map(req.shell);
        let Some(cmd) = command_line_tokenizer::current_command(before, req.shell, &aliases) else {
            return Some(self.error(req, cursor, true));
        };
        let (cache, settings, env) = self.session.parse_inputs();
        let pctx = ParseContext {
            cwd: &req.cwd,
            settings,
            env,
            shell: req.shell,
            index: &self.index,
        };
        let pr = match self.command_line_parser.parse(
            cache,
            &cmd,
            &pctx,
            &mut |notice| log_completion_notice(notice),
        ) {
            Ok(pr) => pr,
            Err(CompletionError::Aborted) => return None,
            Err(CompletionError::NoSuggestions(e)) => {
                logger::debug(&format!("engine: no suggestions: {e}"));
                return Some(self.error(req, cursor, true));
            }
        };
        let arg = pr.current_arg().clone();
        let arg_key = argument_key(arg.as_ref(), pr.completion());
        self.session.enter_argument(arg_key);
        let strategy = arg
            .as_ref()
            .map_or(pr.completion().filter_strategy(), |a| a.filter_strategy());
        let fuzzy = match strategy {
            FilterStrategy::Prefix => false,
            FilterStrategy::Fuzzy => true,
            FilterStrategy::Default => self.session.user_fuzzy(),
        };
        let env = self.session.env();
        let ctx = GeneratorContext::new(
            req.cwd.to_string_lossy().into_owned(),
            env.clone(),
            req.shell.name().into(),
            pr.search_term().to_string(),
            arg.as_ref().is_some_and(|a| a.is_dangerous()),
        );
        let (path, pathext) = (env_var(env, "PATH"), env_var(env, "PATHEXT"));
        let queries = self.queries(&pr, req.shell, js)?;
        let tokens = pr
            .tokens()
            .get(pr.command_index()..)
            .unwrap_or_default()
            .to_vec();
        let triggered = self.generators.trigger(&mut self.session, &pr, &ctx, &tokens, &mut |n| log_completion_notice(n))?;
        let first = if pr.first_token()
            && self.session.settings().bool(AUTOCOMPLETE_FIRST_TOKEN_COMPLETION)
        {
            self.first_token_items(&aliases, path, pathext)
        } else {
            Vec::new()
        };
        let prefix = before
            .strip_suffix(pr.raw_last_token())
            .unwrap_or(before)
            .to_string();
        self.session.serve(ServedRequest::new(ServedRequestParts {
            request_id: req.id,
            buffer: req.buffer.clone(),
            shell: req.shell,
            history_mode: req.history_mode,
            cursor,
            parsed: pr,
            command_text: cmd.original_text().to_string(),
            argument_key: arg_key,
            fuzzy,
            queries,
            triggered_generators: triggered,
            first_token_candidates: first,
            history_prefix: prefix,
        }));
        Some(self.reply(CompletionPhase::Parse))
    }

    /// Error reply (menu error-reset). Generator state is reset on errors too.
    fn error(&mut self, req: &CompletionRequest, cursor: usize, ghost: bool) -> CompletionResult {
        self.session.reset_generator_slots();
        self.session.forget_served_request();
        let mut c = CompletionResult::error(req.id, req.buffer.clone(), cursor);
        if ghost {
            c = c.with_ghost(self.ghost(&req.buffer, cursor, None));
        }
        c
    }

    /// Query derivation per generator of the current arg (function `getQueryTerm`s are called once here).
    fn queries(&self, pr: &ParsedCommandLine, shell: Shell, js: &dyn SpecRuntime) -> Option<Vec<SuggestionQuery>> {
        let Some(arg) = pr.current_arg() else {
            return Some(Vec::new());
        };
        let mut out = Vec::with_capacity(arg.generators().len());
        for g in arg.generators() {
            out.push(if is_path_template(g) {
                SuggestionQuery::Path {
                    backslash: shell == Shell::Pwsh,
                }
            } else {
                match g.get_query_term() {
                    None => SuggestionQuery::Term,
                    Some(TextOrFunction::Str(s)) => SuggestionQuery::Str(s.clone()),
                    Some(TextOrFunction::Fn(f)) => match js.call(
                        SpecFunctionCall::QueryTerm {
                            function: *f,
                            search_term: pr.search_term().to_string(),
                        },
                        None,
                        SYNC_CALL,
                    ) {
                        Ok(SpecFunctionResult::QueryTerm(Some(s))) => SuggestionQuery::Value(s),
                        Err(SpecError::Aborted) => return None,
                        Ok(
                            SpecFunctionResult::QueryTerm(None)
                            | SpecFunctionResult::Trigger(_)
                            | SpecFunctionResult::Alias(_)
                            | SpecFunctionResult::LoadSpec(_)
                            | SpecFunctionResult::GenerateSpecCacheKey(_)
                            | SpecFunctionResult::GenerateSpec(_)
                            | SpecFunctionResult::Script(_)
                            | SpecFunctionResult::Custom(_)
                            | SpecFunctionResult::PostProcess(_)
                            | SpecFunctionResult::FilterTemplateSuggestions(_),
                        )
                        | Err(
                            SpecError::Timeout
                            | SpecError::Invalid(_)
                            | SpecError::BundleUnavailable(_),
                        ) => SuggestionQuery::Value(pr.search_term().to_string()), // error → search term
                    },
                }
            });
        }
        Some(out)
    }

    // -----------------------------------------------------------------------------------------------------
    // Generator runs (SPEC §6.6.2–6.6.5)

    /// Wait (one slice) on every running generator task; reply again when a current slot changed.
    fn pump(&mut self) {
        if self.generators.progress(&mut self.session, &mut |n| log_completion_notice(n)) == Some(true)
            && self.session.served().is_some()
        {
            let _ = self
                .tx
                .send(Msg::Completion(Box::new(self.reply(CompletionPhase::Generators))));
        }
    }

    // -----------------------------------------------------------------------------------------------------
    // Replies

    fn first_token_items(
        &mut self,
        aliases: &HashMap<String, String>,
        path: Option<String>,
        pathext: Option<String>,
    ) -> Vec<SuggestionCandidate> {
        self.session.first_token_suggestions(&Instant::now, || {
            let executables = PathExecutableCatalog.executables(path.as_deref(), pathext.as_deref());
            first_token_suggestions::first_token_items(
                self.index.completions(),
                aliases,
                &executables,
            )
        })
    }

    /// Rank the current request's suggestions (SPEC §6.7.5) into a reply.
    fn reply(&self, phase: CompletionPhase) -> CompletionResult {
        let served = self.session.served().expect("reply without a current request");
        let (pr, s) = (served.parsed(), self.session.settings());
        let mut list = if pr.first_token() {
            served.first_token_candidates().clone()
        } else {
            let views = self.session.slot_views(served.queries());
            suggestion_assembler::assemble(pr, &views)
        };
        let mode = s.string(BETA_HISTORY_MODE).unwrap_or_default();
        if served.history_mode() || mode == "show" || mode == "history_only" {
            let hist = history_suggestions::history_rows(served.history_prefix(), &self.history.recent());
            list = history_suggestions::mix_history(list, hist, mode == "show" && !served.history_mode());
        }
        let fo = FilterOptions::default()
            .with_fuzzy(served.fuzzy())
            .with_auto_exec_hidden(
                s.bool(AUTOCOMPLETE_HIDE_AUTO_EXECUTE_SUGGESTION) || s.bool(AUTOCOMPLETE_ONLY_SHOW_ON_TAB),
            )
            .with_exec_after_space(s.bool(AUTOCOMPLETE_IMMEDIATELY_EXECUTE_AFTER_SPACE))
            .with_run_dangerous(s.bool(AUTOCOMPLETE_IMMEDIATELY_RUN_DANGEROUS_COMMANDS))
            .with_suggest_current_token(
                pr.current_arg()
                    .as_ref()
                    .and_then(|a| a.suggest_current_token())
                    .unwrap_or_else(|| s.bool(AUTOCOMPLETE_ALWAYS_SUGGEST_CURRENT_TOKEN)),
            );
        let cmd = pr.tokens().get(pr.command_index()).map_or("", String::as_str); // D21
        let use_recency = s.string(AUTOCOMPLETE_SORT_METHOD) != Some("alphabetical");
        let map = Some(self.session.recency()).filter(|_| use_recency).and_then(|r| r.of_command(cmd));
        let ranked = suggestion_ranker::rank(list, pr.search_term(), map, &fo);
        let ic = InsertionContext::new(pr.search_term(), pr.raw_last_token(), served.shell())
            .with_fuzzy(served.fuzzy())
            .with_prefer_verbose(s.bool(AUTOCOMPLETE_PREFER_VERBOSE_SUGGESTIONS))
            .with_insert_space(s.bool(AUTOCOMPLETE_INSERT_SPACE_AUTOMATICALLY));
        let items: Vec<Suggestion> = ranked
            .into_iter()
            .map(|(x, p)| suggestion_finalizer::to_item(x, p, &ic))
            .collect();
        let ghost = self.ghost(served.buffer(), served.cursor(), Some(&items));
        CompletionResult::new(CompletionResultParts {
            req: served.request_id(),
            phase,
            error: false,
            buffer: served.buffer().to_string(),
            cursor: served.cursor(),
            tokens: pr.tokens().clone(),
            command_text: served.command_text().to_string(),
            command_index: pr.command_index(),
            arg_key: served.argument_key(),
            current_arg: pr.current_arg().as_ref().map(|a| {
                ArgumentHint::new(
                    a.name().clone().unwrap_or_default(),
                    a.description().clone().unwrap_or_default(),
                )
            }),
            search_term: pr.search_term().to_string(),
            triggered_generators: if phase == CompletionPhase::Parse {
                served.triggered_generators().clone()
            } else {
                Vec::new()
            },
            items,
            loading: self.session.any_slot_loading(),
            ghost,
            fuzzy: served.fuzzy(),
        })
    }

    /// Local inline ghost (SPEC §6.11). Also computed for parse-error replies (commands without a spec).
    fn ghost(&self, buffer: &str, cursor: usize, items: Option<&[Suggestion]>) -> Option<String> {
        if !self.session.settings().bool(INLINE_ENABLED)
            || buffer.is_empty()
            || cursor != buffer.len()
        {
            return None;
        }
        let history = || inline::history_ghost(&self.history.recent(), buffer);
        let specs = || items.and_then(inline::spec_ghost);
        match inline::source(self.session.settings()) {
            Ok(Source::Specs) => specs(),
            Ok(Source::History) | Ok(Source::Profile(_)) => history(),
            _ => history().or_else(specs),
        }
    }

    // -----------------------------------------------------------------------------------------------------
    // LLM ghost text (SPEC §8.4)

    fn llm(&mut self, req: u64, buffer: String, cwd: PathBuf, shell: Shell) {
        let tx = self.tx.clone();
        let Ok(Source::Profile(profile)) = inline::source(self.session.settings()) else {
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::history::domain::entities::history_entry::HistoryEntryPrimitives;
    use crate::history::domain::repositories::history_repository::InMemoryHistoryRepository;
    use crate::history::infrastructure::adapters::jsonl_history_repository::JsonlHistoryRepository;
    use crate::shared::infrastructure::data_directory_from_env::data_directory_from_env;
    use crate::shared::infrastructure::testing::temp_dir::TempDir;
    use std::path::Path;
    use crate::settings::domain::value_objects::setting_value::SettingValue;
    use crate::settings::infrastructure::adapters::setting_value_json::settings_from_json;
    use crate::specs::application::ports::runtime_event_source::ChannelRuntimeEventSource;
    use crate::specs::domain::value_objects::process_output::ProcessOutput;
    use crate::specs::domain::value_objects::exec_request::ExecRequest;
    use serde_json::{json, Value};

    #[test]
    fn llm_history_honours_history_count() {
        // llm-history-count-ignored: only the last `inline.historyCount` usable entries leave the machine
        let e = |c: &str| HistoryEntry::new(HistoryEntryPrimitives {
            command: c.into(),
            exit_code: Some(0),
            ..Default::default()
        });
        let entries: Vec<HistoryEntry> = (0..100).map(|i| e(&format!("echo {i}"))).collect();
        let with = |v: Value| {
            settings_from_json(
                json!({ "inline.historyCount": v })
                    .as_object()
                    .unwrap()
                    .clone(),
            )
        };
        let got: Vec<String> = llm_history(&with(json!(2)), &entries)
            .into_iter()
            .map(|h| h.command().to_string())
            .collect();
        assert_eq!(got, ["echo 98", "echo 99"]);
        assert!(llm_history(&with(json!(0)), &entries).is_empty());
        assert_eq!(llm_history(&Settings::default(), &entries).len(), 49);
    }

    #[test]
    fn session_update_refreshes_first_token_items_keeps_the_engine_running() {
        // first-token-cache-ignores-session-update: `alias zzal=…` at this prompt shows `zzal` at the next one
        let dir = TempDir::new("engine-session");
        let mut e = engine_over(&dir);
        assert!(e.handle(session_context(&[], None), &mut None));
        assert!(e.handle(session_context(&[("PATH", "/session/bin")], None), &mut None));
        assert!(e.handle(session_context(&[], Some("alias ll='ls -l'")), &mut None));
        assert!(e.handle(session_context(&[], Some("")), &mut None));
    }

    /// A parser over `js` and a spec repository that reads the env this process was started with.
    fn parser_over(js: &Rc<QuickJsSpecRuntime>) -> CommandLineParser {
        let bundle = EmbeddedSpecBundle::new(data_directory_from_env());
        CommandLineParser::new(Rc::new(QuickJsSpecRepository::new(js.clone(), bundle)), js.clone())
    }

    /// An engine over a real runtime and a JSONL history in `dir`, whose replies go nowhere.
    fn engine_over(dir: &TempDir) -> Engine {
        let (tx, _) = std::sync::mpsc::channel();
        let js = Rc::new(QuickJsSpecRuntime::new(Rc::new(ChannelRuntimeEventSource::new()), &Settings::default()).unwrap());
        Engine::with(
            tx,
            Settings::default(),
            parser_over(&js),
            js.clone(),
            SpecIndex::default(),
            HistoryModule::new(Rc::new(JsonlHistoryRepository::load(
                dir.path().join("h.jsonl"),
            ))),
            None,
        )
    }

    /// The message a shell's session context arrives as: its env and its alias text.
    fn session_context(env: &[(&str, &str)], alias: Option<&str>) -> EngineMsg {
        let env = env
            .iter()
            .map(|(name, value)| (name.to_string(), value.to_string()))
            .collect();
        let alias = alias.map(str::to_owned);
        EngineMsg::Session { env, alias }
    }

    /// A runtime over a real engine inbox, and a sender into that inbox.
    fn runtime_with_inbox() -> (QuickJsSpecRuntime, Rc<EngineInbox>, Sender<EngineMsg>) {
        let (tx, rx) = mpsc::channel();
        let inbox = Rc::new(EngineInbox::new(rx, tx.clone()));
        let js = QuickJsSpecRuntime::new(inbox.clone(), &Settings::default()).unwrap();
        (js, inbox, tx)
    }

    #[test]
    fn wait_aborts_and_defers_on_complete() {
        let (js, inbox, tx) = runtime_with_inbox();
        let req = |id| CompletionRequest {
            id,
            buffer: "git ".into(),
            cursor: 4,
            cwd: "/".into(),
            shell: Shell::Zsh,
            history_mode: false,
        };
        let slow = js.start_sleep(10_000);
        tx.send(EngineMsg::ToggleFuzzy).unwrap();
        tx.send(EngineMsg::Complete(req(7))).unwrap();
        tx.send(EngineMsg::Shutdown).unwrap();
        let t0 = Instant::now();
        assert_eq!(
            js.wait_json(&[slow], Instant::now() + Duration::from_secs(5)),
            Err(SpecError::Aborted)
        );
        assert!(t0.elapsed() < Duration::from_secs(1));
        // Still queued → a new wait aborts at once.
        assert_eq!(
            js.wait_json(&[slow], Instant::now() + Duration::from_secs(5)),
            Err(SpecError::Aborted)
        );
        assert!(matches!(inbox.next_msg(&js), Some(EngineMsg::ToggleFuzzy)));
        assert!(matches!(inbox.next_msg(&js), Some(EngineMsg::Complete(r)) if r.id == 7));
        // Non-Complete messages are deferred without aborting; the slow task is still pending.
        let quick = js.start_sleep(10);
        let r = js
            .wait_json(&[quick, slow], Instant::now() + Duration::from_millis(300))
            .unwrap();
        assert_eq!(r, vec![Some(Ok(Value::Null)), None]);
        assert!(matches!(inbox.try_next_msg(&js), Some(EngineMsg::Shutdown)));
        assert!(inbox.try_next_msg(&js).is_none());
        js.forget(&[slow]);
        let unknown = |t| Some(Err(format!("unknown task {t}")));
        assert_eq!(
            js.wait_json(&[slow, quick], Instant::now()).unwrap(),
            vec![unknown(slow), unknown(quick)]
        );
        // Exec tasks deliver `{stdout, stderr, status}`; its Runtime event is consumed internally.
        let e = js.start_exec(
            ExecRequest::new("fig".into(), vec![], None, HashMap::new(), None),
            &GeneratorContext::default(),
        );
        let r = js
            .wait_json(&[e], Instant::now() + Duration::from_secs(5))
            .unwrap();
        assert_eq!(
            r,
            vec![Some(Ok(json!({"stdout": "", "stderr": "", "status": 1})))]
        );
    }

    #[test]
    fn should_deliver_a_runtime_event_read_by_the_engine_loop_to_the_runtime() {
        let (rt, inbox, tx) = runtime_with_inbox();
        let id = rt.start_sleep(10_000);
        let output = Ok(ProcessOutput::new("x".into(), "".into(), 0));
        tx.send(EngineMsg::Runtime(RuntimeEvent { id, output })).unwrap();

        let read = inbox.try_next_msg(&rt);
        let delivered = rt.wait_json(&[id], Instant::now());

        assert!(read.is_none());
        assert_eq!(
            delivered,
            Ok(vec![Some(Ok(json!({"status": 0, "stderr": "", "stdout": "x"})))])
        );
    }

    // The engine loop seen from its inbox: requests and events go in, replies come out (6.0a characterization).

    /// How long `serve` lets an engine run before it shuts it down, so a regression fails instead of hanging.
    const SERVE_LIMIT: Duration = Duration::from_secs(10);

    /// A local `tool` spec whose one argument is `name:"t",{arg}`. The default export is an object: a function
    /// export would run the CLI-version probe, which starts `tool --version` as a real process.
    fn tool_spec(arg: &str) -> String {
        format!("var s={{name:\"tool\",args:{{name:\"t\",{arg}}}}};export{{s as default}};")
    }

    /// A temp dir holding the local spec `tool_spec(arg)` and an empty `a.txt`: `./tool …` typed in it loads the
    /// spec (a single token holding `/` would load the global `dotslash` spec instead).
    fn spec_dir(tag: &str, arg: &str) -> TempDir {
        let dir = TempDir::new(tag);
        let build = dir.path().join(".fig").join("autocomplete").join("build");
        std::fs::create_dir_all(&build).unwrap();
        std::fs::write(build.join("tool.js"), tool_spec(arg)).unwrap();
        std::fs::write(dir.path().join("a.txt"), "").unwrap();
        dir
    }

    /// What the native `ls -1ApL` prints for `dir`, in the file system's order (each directory ends in `/`).
    fn listing(dir: &Path) -> Vec<String> {
        std::fs::read_dir(dir)
            .unwrap()
            .map(|entry| {
                let path = entry.unwrap().path();
                let name = path.file_name().unwrap().to_string_lossy().into_owned();
                if path.is_dir() {
                    format!("{name}/")
                } else {
                    name
                }
            })
            .collect()
    }

    fn ran(command: &str) -> HistoryEntry {
        HistoryEntry::new(HistoryEntryPrimitives {
            command: command.into(),
            exit_code: Some(0),
            ..Default::default()
        })
    }

    /// What a test reads of one completion reply.
    #[derive(Clone, Debug, PartialEq)]
    struct Reply {
        request: u64,
        generators: bool,
        error: bool,
        cursor: usize,
        loading: bool,
        rows: Vec<String>,
        ghost: Option<String>,
    }

    fn observe(c: &CompletionResult) -> Reply {
        Reply {
            request: c.request_id(),
            generators: c.phase() == CompletionPhase::Generators,
            error: c.is_error(),
            cursor: c.cursor(),
            loading: c.is_loading(),
            rows: c.suggestions().iter().map(|i| i.first_name().to_string()).collect(),
            ghost: c.ghost().map(String::from),
        }
    }

    fn complete(id: u64, cwd: &Path, buffer: &str) -> EngineMsg {
        complete_at(id, cwd, buffer, buffer.len())
    }

    fn complete_at(id: u64, cwd: &Path, buffer: &str, cursor: usize) -> EngineMsg {
        EngineMsg::Complete(CompletionRequest {
            id,
            buffer: buffer.into(),
            cursor,
            cwd: cwd.to_path_buf(),
            shell: Shell::Zsh,
            history_mode: false,
        })
    }

    fn command_ended() -> EngineMsg {
        EngineMsg::CommandEnded
    }

    fn apply_settings(settings: Settings) -> EngineMsg {
        EngineMsg::Settings(settings)
    }

    fn shutdown() -> EngineMsg {
        EngineMsg::Shutdown
    }

    /// What `serve`'s watcher does after a reply arrived.
    enum Then {
        Wait,
        Deliver(Vec<EngineMsg>),
        Stop,
    }

    fn stop_when(done: impl Fn(&[Reply]) -> bool + Send + 'static) -> impl FnMut(&[Reply]) -> Then + Send + 'static {
        move |replies| if done(replies) { Then::Stop } else { Then::Wait }
    }

    /// Runs an engine over a real inbox, runtime and spec repository on this thread (QuickJS is !Send) and returns
    /// every reply it sent. It is sent `first` before it starts; a watcher thread asks `react` after each reply.
    /// After `Then::Stop` the watcher collects for `grace`, then shuts the engine down (after `SERVE_LIMIT` too).
    fn serve(
        settings: Settings,
        history: Vec<HistoryEntry>,
        first: Vec<EngineMsg>,
        grace: Duration,
        mut react: impl FnMut(&[Reply]) -> Then + Send + 'static,
    ) -> Vec<Reply> {
        let (to_engine, from_senders) = mpsc::channel();
        let inbox = Rc::new(EngineInbox::new(from_senders, to_engine.clone()));
        let runtime = Rc::new(QuickJsSpecRuntime::new(inbox.clone(), &settings).unwrap());
        let (replies, from_engine) = mpsc::channel();
        let history = HistoryModule::new(Rc::new(InMemoryHistoryRepository::new(history)));
        let engine = Engine::with(replies, settings, parser_over(&runtime), runtime.clone(), SpecIndex::default(), history, None);
        for message in first {
            to_engine.send(message).unwrap();
        }
        let to_engine = to_engine.clone();
        let watcher = std::thread::spawn(move || {
            let mut seen = Vec::new();
            let mut shutdown_at = Instant::now() + SERVE_LIMIT;
            let mut stopped = false;
            loop {
                match from_engine.recv_timeout(shutdown_at.saturating_duration_since(Instant::now())) {
                    Ok(Msg::Completion(c)) => {
                        seen.push(observe(&c));
                        if stopped {
                            continue;
                        }
                        match react(&seen) {
                            Then::Wait => {}
                            Then::Deliver(messages) => {
                                for message in messages {
                                    let _ = to_engine.send(message);
                                }
                            }
                            Then::Stop => {
                                stopped = true;
                                shutdown_at = shutdown_at.min(Instant::now() + grace);
                            }
                        }
                    }
                    Ok(_) => {} // LLM ghost text
                    Err(RecvTimeoutError::Timeout) => {
                        let _ = to_engine.send(shutdown());
                        seen.extend(from_engine.iter().filter_map(|m| match m {
                            Msg::Completion(c) => Some(observe(&c)),
                            _ => None,
                        }));
                        return seen;
                    }
                    Err(RecvTimeoutError::Disconnected) => return seen,
                }
            }
        });
        engine.run(&runtime, &inbox);
        watcher.join().unwrap()
    }

    #[test]
    fn should_reply_again_with_the_split_lines_of_a_script_generator() {
        let dir = spec_dir("engine-split-lines", r#"generators:{script:["ls","-1ApL"],splitOn:"\n"}"#);

        let replies = serve(
            Settings::default(),
            Vec::new(),
            vec![complete(1, dir.path(), "./tool ")],
            Duration::ZERO,
            stop_when(|replies| replies.iter().any(|r| r.generators)),
        );

        assert_eq!(
            replies,
            [
                Reply { request: 1, generators: false, error: false, cursor: 7, loading: true, rows: Vec::new(), ghost: None },
                Reply { request: 1, generators: true, error: false, cursor: 7, loading: false, rows: listing(dir.path()), ghost: None },
            ]
        );
    }

    #[test]
    fn should_reply_again_with_the_rows_a_post_process_function_returns() {
        let dir = spec_dir(
            "engine-post-process",
            r#"generators:{script:["fig"],postProcess:function(out,tokens){return ["pp-one",{name:"pp-two"}];}}"#,
        );

        let replies = serve(
            Settings::default(),
            Vec::new(),
            vec![complete(1, dir.path(), "./tool ")],
            Duration::ZERO,
            stop_when(|replies| replies.iter().any(|r| r.generators)),
        );

        assert_eq!(
            replies,
            [
                Reply { request: 1, generators: false, error: false, cursor: 7, loading: true, rows: Vec::new(), ghost: None },
                Reply { request: 1, generators: true, error: false, cursor: 7, loading: false, rows: vec!["pp-one".into(), "pp-two".into()], ghost: None },
            ]
        );
    }

    #[test]
    fn should_reply_again_with_the_filtered_rows_of_a_templated_custom_generator() {
        let dir = spec_dir(
            "engine-templated-custom",
            r#"generators:{custom:function(tokens,exec,context){return [{name:"keep",context:{}},{name:"drop",context:{}}];},filterTemplateSuggestions:function(rows){return rows.filter(function(r){return r.name!=="drop";});}}"#,
        );

        let replies = serve(
            Settings::default(),
            Vec::new(),
            vec![complete(1, dir.path(), "./tool ")],
            Duration::ZERO,
            stop_when(|replies| replies.iter().any(|r| r.generators)),
        );

        assert_eq!(
            replies,
            [
                Reply { request: 1, generators: false, error: false, cursor: 7, loading: true, rows: Vec::new(), ghost: None },
                Reply { request: 1, generators: true, error: false, cursor: 7, loading: false, rows: vec!["keep".into()], ghost: None },
            ]
        );
    }

    /// The command ending forgets the generator results but not the generator cache, so the next request fires the
    /// generator again and finds its result fresh in the cache.
    #[test]
    fn should_serve_a_fresh_cached_custom_result_in_the_parse_reply() {
        let dir = spec_dir(
            "engine-cached-custom",
            r#"generators:{custom:function(){return [{name:"cached"}];},cache:{ttl:600000}}"#,
        );
        let cwd = dir.path().to_path_buf();
        let mut command_ended_sent = false;
        let end_the_command_then_ask_again = move |replies: &[Reply]| {
            if replies.iter().any(|r| r.request == 2) {
                return Then::Stop;
            }
            if !command_ended_sent && replies.iter().any(|r| r.request == 1 && r.generators) {
                command_ended_sent = true;
                return Then::Deliver(vec![command_ended(), complete(2, &cwd, "./tool ")]);
            }
            Then::Wait
        };

        let replies = serve(
            Settings::default(),
            Vec::new(),
            vec![complete(1, dir.path(), "./tool ")],
            Duration::from_millis(300),
            end_the_command_then_ask_again,
        );

        assert_eq!(
            replies,
            [
                Reply { request: 1, generators: false, error: false, cursor: 7, loading: true, rows: Vec::new(), ghost: None },
                Reply { request: 1, generators: true, error: false, cursor: 7, loading: false, rows: vec!["cached".into()], ghost: None },
                Reply { request: 2, generators: false, error: false, cursor: 7, loading: false, rows: vec!["cached".into()], ghost: None },
            ]
        );
    }

    #[test]
    fn should_settle_a_generator_with_bad_script_input_with_no_rows() {
        let dir = spec_dir("engine-bad-script-input", r#"generators:{script:{command:1}}"#);

        let replies = serve(
            Settings::default(),
            Vec::new(),
            vec![complete(1, dir.path(), "./tool ")],
            Duration::from_millis(300),
            stop_when(|replies| !replies.is_empty()),
        );

        assert_eq!(
            replies,
            [Reply { request: 1, generators: false, error: false, cursor: 7, loading: false, rows: Vec::new(), ghost: None }]
        );
    }

    /// The second request re-fires the debounced generator, so the first run wakes up superseded and is abandoned.
    #[test]
    fn should_keep_the_rows_of_a_superseded_debounced_run_out_of_the_slot() {
        let dir = spec_dir(
            "engine-superseded-debounce",
            r#"debounce:1000,generators:{script:["ls","-1ApL"],splitOn:"\n"}"#,
        );
        let cwd = dir.path().to_path_buf();
        let mut asked_again = false;
        let ask_again_then_stop_at_generators = move |replies: &[Reply]| {
            if replies.iter().any(|r| r.generators) {
                return Then::Stop;
            }
            if !asked_again && replies.iter().any(|r| r.request == 1 && !r.generators) {
                asked_again = true;
                return Then::Deliver(vec![complete(2, &cwd, "./tool ")]);
            }
            Then::Wait
        };

        let replies = serve(
            Settings::default(),
            Vec::new(),
            vec![complete(1, dir.path(), "./tool ")],
            Duration::from_millis(1500),
            ask_again_then_stop_at_generators,
        );

        assert_eq!(
            replies,
            [
                Reply { request: 1, generators: false, error: false, cursor: 7, loading: true, rows: Vec::new(), ghost: None },
                Reply { request: 2, generators: false, error: false, cursor: 7, loading: true, rows: Vec::new(), ghost: None },
                Reply { request: 2, generators: true, error: false, cursor: 7, loading: false, rows: listing(dir.path()), ghost: None },
            ]
        );
    }

    #[test]
    fn should_serve_only_the_latest_request_after_draining_the_inbox() {
        let dir = spec_dir("engine-latest-request", "");

        let replies = serve(
            Settings::default(),
            Vec::new(),
            vec![complete(1, dir.path(), "./tool "), complete(2, dir.path(), "./missing ")],
            Duration::ZERO,
            stop_when(|replies| replies.iter().any(|r| r.request == 2)),
        );

        assert_eq!(
            replies,
            [Reply { request: 2, generators: false, error: true, cursor: 10, loading: false, rows: Vec::new(), ghost: None }]
        );
    }

    /// The engine starts with ghost text off; only the settings queued behind the request turn it on.
    #[test]
    fn should_apply_settings_queued_after_a_request_before_serving_it() {
        let dir = spec_dir("engine-settings-before-request", "");

        let replies = serve(
            Settings::default().with(INLINE_ENABLED, SettingValue::Bool(false)),
            vec![ran("./missing --flag")],
            vec![complete(1, dir.path(), "./missing "), apply_settings(Settings::default())],
            Duration::ZERO,
            stop_when(|replies| !replies.is_empty()),
        );

        assert_eq!(
            replies,
            [Reply { request: 1, generators: false, error: true, cursor: 10, loading: false, rows: Vec::new(), ghost: Some("--flag".into()) }]
        );
    }

    #[test]
    fn should_drop_a_request_that_arrives_with_shutdown() {
        let dir = spec_dir("engine-request-with-shutdown", "");

        let replies = serve(
            Settings::default(),
            Vec::new(),
            vec![complete(1, dir.path(), "./missing "), shutdown()],
            Duration::ZERO,
            stop_when(|_| false),
        );

        assert_eq!(replies, Vec::<Reply>::new());
    }

    #[test]
    fn should_add_the_local_ghost_text_to_a_generators_reply() {
        let dir = spec_dir("engine-generators-ghost", r#"generators:{script:["ls","-1ApL"],splitOn:"\n"}"#);

        let replies = serve(
            Settings::default(),
            Vec::new(),
            vec![complete(1, dir.path(), "./tool a")],
            Duration::ZERO,
            stop_when(|replies| replies.iter().any(|r| r.generators)),
        );

        assert_eq!(
            replies,
            [
                Reply { request: 1, generators: false, error: false, cursor: 8, loading: true, rows: Vec::new(), ghost: None },
                Reply { request: 1, generators: true, error: false, cursor: 8, loading: false, rows: vec!["a.txt".into()], ghost: Some(".txt".into()) },
            ]
        );
    }

    #[test]
    fn should_add_a_history_ghost_text_to_the_error_reply_of_a_command_without_a_spec() {
        let dir = spec_dir("engine-error-history-ghost", "");

        let replies = serve(
            Settings::default(),
            vec![ran("./missing --flag")],
            vec![complete(1, dir.path(), "./missing ")],
            Duration::ZERO,
            stop_when(|replies| !replies.is_empty()),
        );

        assert_eq!(
            replies,
            [Reply { request: 1, generators: false, error: true, cursor: 10, loading: false, rows: Vec::new(), ghost: Some("--flag".into()) }]
        );
    }

    #[test]
    fn should_reply_an_error_without_ghost_text_for_a_cursor_inside_a_word() {
        let dir = spec_dir("engine-mid-word", "");

        let replies = serve(
            Settings::default(),
            vec![ran("./missing --flag")],
            vec![complete_at(1, dir.path(), "./missing ", 3)],
            Duration::ZERO,
            stop_when(|replies| !replies.is_empty()),
        );

        assert_eq!(
            replies,
            [Reply { request: 1, generators: false, error: true, cursor: 3, loading: false, rows: Vec::new(), ghost: None }]
        );
    }
}
