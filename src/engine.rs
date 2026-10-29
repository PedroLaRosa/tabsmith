//! The completion engine thread (SPEC §4.3, §6): owns the JS host, spec/parse caches, generator state,
//! recency, history and the inline cache. Serves only the latest request.
//! OWNER: engine agent.
//!
//! Per request: mid-word guard → `command_line_tokenizer::current_command` → `CommandLineParser::parse` → generator triggers →
//! reply (phase `Parse`). Triggered generators run as small state machines over `QuickJsSpecRuntime` tasks;
//! whenever one finishes, the latest request is re-ranked and replied again (phase `Generators`).
use crate::inline::{self, GhostCache, Source};
use crate::shared::infrastructure::logger;
use crate::shared::infrastructure::shared_dependencies::SharedDependencies;
use crate::completion::domain::completion_error::CompletionError;
use crate::completion::domain::value_objects::parsed_command_line::ParsedCommandLine;
use crate::completion::application::command_line_parser::CommandLineParser;
use crate::completion::application::completion_notice::CompletionNotice;
use crate::completion::application::parse_context::ParseContext;
use crate::completion::application::ports::directory_lister::DirectoryLister;
use crate::completion::application::ports::executable_catalog::ExecutableCatalog;
use crate::completion::domain::entities::parse_prefix_cache::ParsePrefixCache;
use crate::completion::domain::entities::suggestion_recency::SuggestionRecency;
use crate::completion::domain::repositories::recency_repository::RecencyRepository;
use crate::completion::infrastructure::adapters::fs_directory_lister::FsDirectoryLister;
use crate::completion::infrastructure::adapters::path_executable_catalog::PathExecutableCatalog;
use crate::completion::infrastructure::adapters::state_file_recency_repository::StateFileRecencyRepository;
use crate::settings::domain::services::settings_catalog::{
    AUTOCOMPLETE_ALWAYS_SUGGEST_CURRENT_TOKEN, AUTOCOMPLETE_FIRST_TOKEN_COMPLETION,
    AUTOCOMPLETE_FUZZY_SEARCH, AUTOCOMPLETE_HIDE_AUTO_EXECUTE_SUGGESTION,
    AUTOCOMPLETE_IMMEDIATELY_EXECUTE_AFTER_SPACE, AUTOCOMPLETE_IMMEDIATELY_RUN_DANGEROUS_COMMANDS,
    AUTOCOMPLETE_INSERT_SPACE_AUTOMATICALLY, AUTOCOMPLETE_ONLY_SHOW_ON_TAB,
    AUTOCOMPLETE_PREFER_VERBOSE_SUGGESTIONS, AUTOCOMPLETE_SORT_METHOD,
    BETA_HISTORY_MODE, INLINE_ENABLED, INLINE_HISTORY_COUNT,
};
use crate::settings::domain::value_objects::settings::Settings;
use crate::completion::application::completion_request::CompletionRequest;
use crate::completion::domain::services::command_line_tokenizer;
use crate::completion::domain::services::first_token_suggestions;
use crate::completion::domain::services::generator_trigger_policy::{
    cache_key, dir_changed, effective_debounce, is_path_template, run_budget, script_timeout, triggered,
    SYNC_CALL,
};
use crate::completion::domain::services::history_suggestions;
use crate::completion::domain::services::path_template;
use crate::completion::domain::services::suggestion_assembler;
use crate::completion::domain::services::suggestion_finalizer;
use crate::completion::domain::services::suggestion_ranker;
use crate::completion::domain::value_objects::alias_table::parse_aliases;
use crate::completion::domain::value_objects::argument_hint::ArgumentHint;
use crate::completion::domain::value_objects::completion_phase::CompletionPhase;
use crate::completion::domain::value_objects::completion_result::{CompletionResult, CompletionResultParts};
use crate::completion::domain::value_objects::generator_key::{argument_key, GeneratorKey};
use crate::completion::domain::value_objects::suggestion::Suggestion;
use crate::specs::application::ports::runtime_event::RuntimeEvent;
use crate::specs::application::ports::runtime_event_poll::RuntimeEventPoll;
use crate::specs::application::ports::runtime_event_sink::RuntimeEventSink;
use crate::specs::application::ports::runtime_event_source::RuntimeEventSource;
use crate::specs::application::ports::spec_runtime::SpecRuntime;
use crate::specs::domain::spec_error::SpecError;
use crate::specs::domain::value_objects::arg::Arg;
use crate::specs::domain::value_objects::exec_request::ExecRequest;
use crate::specs::domain::value_objects::filter_strategy::FilterStrategy;
use crate::specs::domain::value_objects::generator::Generator;
use crate::specs::domain::value_objects::generator_context::GeneratorContext;
use crate::specs::domain::value_objects::generator_script::GeneratorScript;
use crate::specs::domain::value_objects::generator_trigger::GeneratorTrigger;
use crate::specs::domain::value_objects::spec_function_call::SpecFunctionCall;
use crate::specs::domain::value_objects::spec_function_result::SpecFunctionResult;
use crate::specs::domain::value_objects::spec_suggestion::SpecSuggestion;
use crate::specs::domain::value_objects::task_id::TaskId;
use crate::specs::domain::value_objects::text_or_function::TextOrFunction;
use crate::specs::infrastructure::adapters::exec_input::ExecInput;
use crate::specs::infrastructure::adapters::fig_spec_mapper::{
    self, generator_context_json, json_items, post_process_items, script_exec_input, spec_value_to_json,
};
use crate::specs::domain::value_objects::spec_index::SpecIndex;
use crate::specs::infrastructure::adapters::embedded_spec_bundle::EmbeddedSpecBundle;
use crate::specs::infrastructure::adapters::quickjs_spec_repository::QuickJsSpecRepository;
use crate::specs::infrastructure::adapters::quickjs_spec_runtime::{JsArg, QuickJsSpecRuntime, TaskResult};
use crate::specs::infrastructure::specs_module::build_specs_module;
use crate::completion::domain::entities::generator_cache::GeneratorCache;
use crate::completion::domain::value_objects::cache_lookup::CacheLookup;
use crate::completion::domain::value_objects::filter_options::FilterOptions;
use crate::completion::domain::value_objects::generator_slot_view::GeneratorSlotView;
use crate::completion::domain::value_objects::insertion_context::InsertionContext;
use crate::completion::domain::value_objects::suggestion_candidate::SuggestionCandidate;
use crate::completion::domain::value_objects::suggestion_query::SuggestionQuery;
use crate::shared::domain::value_objects::shell::Shell;
use crate::history::domain::entities::history_entry::HistoryEntry;
use crate::history::domain::repositories::history_repository::HistoryRepository;
use crate::history::application::record_command_use_case::RecordCommandUseCase;
use crate::history::infrastructure::history_module::{build_history_module, HistoryModule};
use crate::types::{
    now_ms, EngineMsg, Msg,
};
use serde_json::{json, Value};
use std::cell::RefCell;
use std::collections::{HashMap, VecDeque};
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// Pump slice while generators run: deferred messages and finished runs are handled at least this often.
const SLICE: Duration = Duration::from_millis(50);
const FIRST_TOKEN_TTL: Duration = Duration::from_secs(10);

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
            Engine::new(tx, settings, command_line_parser, &dependencies).run(&runtime, &inbox);
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
    }
}

/// Generator results of the current argument, one per generator (SPEC §6.6.1).
struct Slot {
    key: GeneratorKey,
    result: Vec<SpecSuggestion>,
    loading: bool,
    /// Trigger generation: runs carrying another value are stale.
    gen_id: u64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Step {
    Debounce,
    Script,
    Exec,
    PostProcess,
    Custom,
    Filter,
}

/// One generator run waiting on a `QuickJsSpecRuntime` task.
#[derive(Clone)]
struct Run {
    slot: usize,
    gen_id: u64,
    /// Stale-while-revalidate refresh: its failure leaves the slot alone.
    background: bool,
    arg: Rc<Arg>,
    idx: usize,
    ctx: GeneratorContext,
    tokens: Vec<String>,
    help: Vec<SpecSuggestion>,
    step: Step,
    task: TaskId,
    /// Cache key this run's task result is stored under (owner of an in-flight fetch).
    store: Option<String>,
    deadline: Instant,
}

enum Outcome {
    Pending,
    Done(Vec<SpecSuggestion>),
    Fail(String),
}

/// The latest served request (for phase-Generators replies).
struct Current {
    req: CompletionRequest,
    cursor: usize,
    pr: ParsedCommandLine,
    cmd_text: String,
    arg_key: u64,
    fuzzy: bool,
    queries: Vec<SuggestionQuery>,
    triggered: Vec<GeneratorKey>,
    first: Vec<SuggestionCandidate>,
    /// Buffer up to the start of the last token (history rows).
    prefix: String,
}

struct Engine {
    tx: Sender<Msg>,
    settings: Settings,
    command_line_parser: CommandLineParser,
    parse_prefix_cache: ParsePrefixCache,
    index: SpecIndex,
    history: Rc<dyn HistoryRepository>,
    record_command: RecordCommandUseCase,
    session_env: HashMap<String, String>,
    session_alias: Option<String>,
    wrapper_env: HashMap<String, String>,
    aliases: Option<(Shell, HashMap<String, String>)>,
    recency: SuggestionRecency,
    recency_repository: Rc<dyn RecencyRepository>,
    ghost_cache: Arc<Mutex<GhostCache>>,
    user_fuzzy: bool,
    last_arg_key: u64,
    slot_arg: Option<Rc<Arg>>,
    slots: Vec<Slot>,
    prev_search: String,
    next_gen: u64,
    runs: Vec<Run>,
    cache: GeneratorCache<Value>,
    cur: Option<Current>,
    first_token_cache: Option<(Instant, Vec<SuggestionCandidate>)>,
}

impl Engine {
    fn new(
        tx: Sender<Msg>,
        settings: Settings,
        command_line_parser: CommandLineParser,
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
            index,
            build_history_module(dependencies),
            dependencies.data_directory.state_file().ok(),
        )
    }

    fn with(
        tx: Sender<Msg>,
        settings: Settings,
        command_line_parser: CommandLineParser,
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
        Engine {
            tx,
            user_fuzzy: settings.bool(AUTOCOMPLETE_FUZZY_SEARCH),
            settings,
            command_line_parser,
            parse_prefix_cache: ParsePrefixCache::default(),
            index,
            history,
            record_command,
            session_env: HashMap::new(),
            session_alias: None,
            wrapper_env: std::env::vars().collect(),
            aliases: None,
            recency,
            recency_repository,
            ghost_cache: Arc::new(Mutex::new(GhostCache::default())),
            last_arg_key: 0,
            slot_arg: None,
            slots: Vec::new(),
            prev_search: String::new(),
            next_gen: 0,
            runs: Vec::new(),
            cache: GeneratorCache::default(),
            cur: None,
            first_token_cache: None,
        }
    }

    fn run(mut self, js: &QuickJsSpecRuntime, inbox: &EngineInbox) {
        let mut pending: Option<CompletionRequest> = None;
        'outer: loop {
            if pending.is_none() && self.runs.is_empty() {
                match inbox.next_msg(js) {
                    Some(m) => {
                        if !self.handle(m, &mut pending, js) {
                            break;
                        }
                    }
                    None => break,
                }
            }
            // only the latest request is served: drain everything queued first
            while let Some(m) = inbox.try_next_msg(js) {
                if !self.handle(m, &mut pending, js) {
                    break 'outer;
                }
            }
            if let Some(req) = pending.take() {
                if let Some(c) = self.compute(&req, js) {
                    let _ = self.tx.send(Msg::Completion(Box::new(c)));
                }
                continue;
            }
            if !self.runs.is_empty() {
                self.pump(js);
            }
        }
        self.recency_repository.flush();
    }

    /// Handle one message; `false` = shut down.
    fn handle(&mut self, m: EngineMsg, pending: &mut Option<CompletionRequest>, js: &QuickJsSpecRuntime) -> bool {
        match m {
            EngineMsg::Complete(r) => *pending = Some(r),
            EngineMsg::CommandFinished(_) | EngineMsg::CommandEnded => {
                if let EngineMsg::CommandFinished(e) = m {
                    self.record_command.execute(e);
                }
                if let Ok(mut c) = self.ghost_cache.lock() {
                    c.clear();
                }
                self.reset_slots(); // generator results without `cache` live until the command ends
                self.cur = None;
            }
            EngineMsg::RecordInsert { cmd, name } => self.record(cmd, name),
            EngineMsg::Llm {
                req,
                buffer,
                cwd,
                shell,
            } => self.llm(req, buffer, cwd, shell),
            EngineMsg::Settings(s) => {
                js.apply_settings(&s);
                self.settings = s;
                self.command_line_parser.clear_caches(&mut self.parse_prefix_cache);
                self.reset_slots();
                self.first_token_cache = None;
            }
            EngineMsg::Session { env, alias } => {
                self.session_env = env;
                self.session_alias = alias;
                self.aliases = None;
                self.first_token_cache = None; // aliases / PATH from this prompt
            }
            EngineMsg::ToggleFuzzy => self.user_fuzzy = !self.user_fuzzy,
            EngineMsg::Runtime(_) => {} // handed to the js host by the inbox
            EngineMsg::Shutdown => return false,
        }
        true
    }

    fn reset_slots(&mut self) {
        self.slots.clear();
        self.slot_arg = None;
        self.prev_search.clear();
    }

    fn env(&self) -> &HashMap<String, String> {
        if self.session_env.is_empty() {
            &self.wrapper_env
        } else {
            &self.session_env
        }
    }

    fn alias_map(&mut self, shell: Shell) -> HashMap<String, String> {
        if !matches!(&self.aliases, Some((s, _)) if *s == shell) {
            let map = match self.session_alias.as_deref() {
                Some(text) if !text.is_empty() => parse_aliases(text, shell),
                _ => HashMap::new(),
            };
            self.aliases = Some((shell, map));
        }
        self.aliases
            .as_ref()
            .map(|(_, m)| m.clone())
            .unwrap_or_default()
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
        let aliases = self.alias_map(req.shell);
        let Some(cmd) = command_line_tokenizer::current_command(before, req.shell, &aliases) else {
            return Some(self.error(req, cursor, true));
        };
        let env = if self.session_env.is_empty() {
            &self.wrapper_env
        } else {
            &self.session_env
        };
        let pctx = ParseContext {
            cwd: &req.cwd,
            settings: &self.settings,
            env,
            shell: req.shell,
            index: &self.index,
        };
        let pr = match self.command_line_parser.parse(
            &mut self.parse_prefix_cache,
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
        if arg_key != self.last_arg_key {
            self.user_fuzzy = self.settings.bool(AUTOCOMPLETE_FUZZY_SEARCH);
            self.last_arg_key = arg_key;
        }
        let strategy = arg
            .as_ref()
            .map_or(pr.completion().filter_strategy(), |a| a.filter_strategy());
        let fuzzy = match strategy {
            FilterStrategy::Prefix => false,
            FilterStrategy::Fuzzy => true,
            FilterStrategy::Default => self.user_fuzzy,
        };
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
        let triggered = self.update_slots(&pr, &ctx, &tokens, js)?;
        let first = if pr.first_token()
            && self.settings.bool(AUTOCOMPLETE_FIRST_TOKEN_COMPLETION)
        {
            self.first_token_items(&aliases, path, pathext)
        } else {
            Vec::new()
        };
        let prefix = before
            .strip_suffix(pr.raw_last_token())
            .unwrap_or(before)
            .to_string();
        self.cur = Some(Current {
            req: req.clone(),
            cursor,
            pr,
            cmd_text: cmd.original_text().to_string(),
            arg_key,
            fuzzy,
            queries,
            triggered,
            first,
            prefix,
        });
        Some(self.reply(CompletionPhase::Parse))
    }

    /// Error reply (menu error-reset). Generator state is reset on errors too.
    fn error(&mut self, req: &CompletionRequest, cursor: usize, ghost: bool) -> CompletionResult {
        self.reset_slots();
        self.cur = None;
        let mut c = CompletionResult::error(req.id, req.buffer.clone(), cursor);
        if ghost {
            c = c.with_ghost(self.ghost(req, cursor, None));
        }
        c
    }

    /// Query derivation per generator of the current arg (function `getQueryTerm`s are called once here).
    fn queries(&self, pr: &ParsedCommandLine, shell: Shell, js: &QuickJsSpecRuntime) -> Option<Vec<SuggestionQuery>> {
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

    /// SPEC §6.6.1: decide triggers (no mutation until every JS trigger answered), then start runs.
    fn update_slots(
        &mut self,
        pr: &ParsedCommandLine,
        ctx: &GeneratorContext,
        tokens: &[String],
        js: &QuickJsSpecRuntime,
    ) -> Option<Vec<GeneratorKey>> {
        let arg = pr.current_arg().clone();
        let same = match (&arg, &self.slot_arg) {
            (Some(a), Some(b)) => Rc::ptr_eq(a, b),
            _ => false,
        };
        let (new, old) = (pr.search_term(), self.prev_search.as_str());
        let gens: &[Generator] = arg.as_deref().map_or(&[], |a| a.generators());
        let mut fire = Vec::with_capacity(gens.len());
        for (i, g) in gens.iter().enumerate() {
            fire.push(if !same || i >= self.slots.len() {
                true
            } else if is_path_template(g) {
                dir_changed(new, old, ctx.shell() == Shell::Pwsh.name())
            } else {
                match g.trigger() {
                    None => arg.as_ref().is_some_and(|a| a.debounce().is_some()),
                    Some(GeneratorTrigger::Fn(f)) => match js.call(
                        SpecFunctionCall::Trigger {
                            function: *f,
                            new_token: new.to_string(),
                            old_token: old.to_string(),
                        },
                        None,
                        SYNC_CALL,
                    ) {
                        Ok(SpecFunctionResult::Trigger(fired)) => fired,
                        Err(SpecError::Aborted) => return None,
                        Ok(
                            SpecFunctionResult::QueryTerm(_)
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
                        ) => true, // a throwing trigger counts as a trigger
                    },
                    Some(t) => triggered(t, new, old).unwrap_or(true),
                }
            });
        }
        self.prev_search = pr.search_term().to_string();
        let Some(arg) = arg.as_ref() else {
            self.slots.clear();
            self.slot_arg = None;
            return Some(Vec::new());
        };
        if !same {
            // ponytail: results are not carried over to a different arg (they could be kept while loading)
            self.slots = (0..gens.len())
                .map(|i| Slot {
                    key: GeneratorKey::for_arg(arg, i),
                    result: Vec::new(),
                    loading: false,
                    gen_id: 0,
                })
                .collect();
            self.slot_arg = Some(arg.clone());
        }
        let help = if gens.iter().any(|g| g.template().iter().any(|t| t == "help")) {
            path_template::help_items(pr)
        } else {
            Vec::new()
        };
        let floor = script_timeout(&self.settings);
        let mut triggered = Vec::new();
        for (i, fire) in fire.into_iter().enumerate() {
            if !fire {
                continue;
            }
            self.next_gen += 1;
            let slot = &mut self.slots[i];
            slot.gen_id = self.next_gen;
            slot.loading = true;
            triggered.push(slot.key);
            let debounce = effective_debounce(arg.debounce());
            // ponytail: one deadline for the whole run (custom generators may exec several times)
            let budget = run_budget(debounce, floor, gens[i].script_timeout());
            let mut run = Run {
                slot: i,
                gen_id: self.next_gen,
                background: false,
                arg: arg.clone(),
                idx: i,
                ctx: ctx.clone(),
                tokens: tokens.to_vec(),
                help: help.clone(),
                step: Step::Debounce,
                task: 0,
                store: None,
                deadline: Instant::now() + Duration::from_millis(budget),
            };
            let outcome = match debounce {
                Some(ms) => {
                    run.task = js.start_sleep(ms);
                    Outcome::Pending
                }
                None => self.begin(&mut run, js),
            };
            self.settle(run, outcome);
        }
        Some(triggered)
    }

    // -----------------------------------------------------------------------------------------------------
    // Generator runs (SPEC §6.6.2–6.6.5)

    fn is_current(&self, run: &Run) -> bool {
        self.slots
            .get(run.slot)
            .is_some_and(|s| s.gen_id == run.gen_id)
    }

    /// Record an outcome; returns whether a current slot changed.
    fn settle(&mut self, run: Run, outcome: Outcome) -> bool {
        let items = match outcome {
            Outcome::Pending => {
                self.runs.push(run);
                return false;
            }
            Outcome::Done(items) => items,
            Outcome::Fail(e) => {
                logger::debug(&format!("engine: generator failed: {e}"));
                if run.background {
                    return false;
                }
                Vec::new() // any error → [] (never cached)
            }
        };
        match self.slots.get_mut(run.slot) {
            Some(s) if s.gen_id == run.gen_id => {
                s.result = items;
                s.loading = false;
                true
            }
            _ => false,
        }
    }

    /// First step after the (optional) debounce: template, script or custom.
    fn begin(&mut self, run: &mut Run, js: &QuickJsSpecRuntime) -> Outcome {
        let arg = run.arg.clone();
        let g = &arg.generators()[run.idx];
        if !g.template().is_empty() {
            let mut items = Vec::new();
            let files = g.template().iter().any(|t| t == "filepaths");
            if is_path_template(g) && !run.ctx.cwd().is_empty() {
                let dir = path_template::inserted_directory(
                    run.ctx.cwd(),
                    run.ctx.search_term(),
                    run.ctx.env(),
                    run.ctx.shell() == Shell::Pwsh.name(),
                );
                items = FsDirectoryLister.list(&dir, files, run.ctx.is_dangerous());
            }
            items.extend(run.help.iter().cloned()); // `history` template: [] (F6)
            return match g.filter_template_suggestions() {
                Some(f) => {
                    run.task = js.start_call(
                        f,
                        vec![JsArg::Json(fig_spec_mapper::template_json(&items))],
                        Some(&run.ctx),
                    );
                    run.step = Step::Filter;
                    Outcome::Pending
                }
                None => Outcome::Done(items),
            };
        }
        if run.ctx.cwd().is_empty() {
            return Outcome::Done(Vec::new()); // no context for script/custom generators
        }
        if let Some(script) = g.script() {
            return match script {
                GeneratorScript::Fn(f) => {
                    run.task =
                        js.start_call(*f, vec![JsArg::Json(json!(run.tokens))], Some(&run.ctx));
                    run.step = Step::Script;
                    Outcome::Pending
                }
                GeneratorScript::Argv(a) => self.script_input(run, g, json!(a), js),
                GeneratorScript::Shell(s) => self.script_input(run, g, json!(s), js),
                GeneratorScript::Object(o) => self.script_input(run, g, spec_value_to_json(o), js),
            };
        }
        let Some(custom) = g.custom() else {
            return Outcome::Done(Vec::new());
        };
        let key = g.cache().as_ref().map(|c| {
            cache_key(
                c,
                run.ctx.cwd(),
                c.cache_key().clone().unwrap_or_else(|| run.tokens.join(" ")),
            )
        });
        let args = vec![
            JsArg::Json(json!(run.tokens)),
            JsArg::Exec,
            JsArg::Json(generator_context_json(&run.ctx)),
        ];
        let ctx = run.ctx.clone();
        match self.fetch(run, g, key, Step::Custom, js, move |js| {
            js.start_call(custom, args, Some(&ctx))
        }) {
            Ok(v) => self.custom_result(run, g, v, js),
            Err(o) => o,
        }
    }

    /// Cached fetch (D10): a value now (`Ok`), or a started/shared task (`Err(Pending)`).
    fn fetch(
        &mut self,
        run: &mut Run,
        g: &Generator,
        key: Option<String>,
        step: Step,
        js: &QuickJsSpecRuntime,
        start: impl FnOnce(&QuickJsSpecRuntime) -> TaskId,
    ) -> Result<Value, Outcome> {
        let (Some(cfg), Some(key)) = (g.cache(), key) else {
            run.task = start(js);
            run.step = step;
            return Err(Outcome::Pending);
        };
        match self.cache.lookup(&key, cfg, now_ms()) {
            CacheLookup::Fresh(v) => Ok(v),
            CacheLookup::Stale { value, refresh } => {
                if refresh {
                    let mut bg = run.clone();
                    bg.background = true;
                    bg.task = start(js);
                    bg.step = step;
                    self.cache.start(&key, bg.task);
                    bg.store = Some(key);
                    self.runs.push(bg);
                }
                Ok(value)
            }
            CacheLookup::Shared(t) => {
                run.task = t;
                run.step = step;
                Err(Outcome::Pending)
            }
            CacheLookup::Miss => {
                run.task = start(js);
                run.step = step;
                self.cache.start(&key, run.task);
                run.store = Some(key);
                Err(Outcome::Pending)
            }
        }
    }

    /// SPEC §6.6.3: script value → exec input → (cached) exec.
    fn script_input(&mut self, run: &mut Run, g: &Generator, v: Value, js: &QuickJsSpecRuntime) -> Outcome {
        let Some(mut input) = script_exec_input(v, run.ctx.cwd()) else {
            return Outcome::Done(Vec::new());
        }; // falsy → []
        let key_src = input.to_string();
        let timeout = script_timeout(&self.settings)
            .max(g.script_timeout().unwrap_or(0))
            .max(input.get("timeout").and_then(Value::as_u64).unwrap_or(0));
        input["timeout"] = json!(timeout);
        let exec: ExecInput = match serde_json::from_value(input) {
            Ok(i) => i,
            Err(e) => return Outcome::Fail(format!("bad script input: {e}")),
        };
        let key = g
            .cache()
            .as_ref()
            .map(|c| cache_key(c, run.ctx.cwd(), c.cache_key().clone().unwrap_or(key_src)));
        let ctx = run.ctx.clone();
        match self.fetch(run, g, key, Step::Exec, js, move |js| {
            js.start_exec(ExecRequest::from(exec), &ctx)
        }) {
            Ok(v) => self.exec_output(run, g, &v, js),
            Err(o) => o,
        }
    }

    /// `splitOn` takes precedence over `postProcess`; neither → [].
    fn exec_output(&mut self, run: &mut Run, g: &Generator, v: &Value, js: &QuickJsSpecRuntime) -> Outcome {
        let stdout = v.get("stdout").and_then(Value::as_str).unwrap_or("");
        if let Some(sep) = g.split_on() {
            let t = stdout.trim();
            if t.is_empty() {
                return Outcome::Done(Vec::new());
            }
            return Outcome::Done(
                t.split(sep.as_str())
                    .map(|s| SpecSuggestion::from_script_output(s, run.ctx.is_dangerous()))
                    .collect(),
            );
        }
        let Some(pp) = g.post_process() else {
            return Outcome::Done(Vec::new());
        };
        run.task = js.start_call(
            pp,
            vec![JsArg::Json(json!(stdout)), JsArg::Json(json!(run.tokens))],
            Some(&run.ctx),
        );
        run.step = Step::PostProcess;
        Outcome::Pending
    }

    /// Custom result; `filterTemplateSuggestions` applies when the first item has a `context`.
    fn custom_result(&mut self, run: &mut Run, g: &Generator, v: Value, js: &QuickJsSpecRuntime) -> Outcome {
        let templated = v
            .as_array()
            .and_then(|a| a.first())
            .is_some_and(|x| x.get("context").is_some());
        match g.filter_template_suggestions() {
            Some(f) if templated => {
                run.task = js.start_call(f, vec![JsArg::Json(v)], Some(&run.ctx));
                run.step = Step::Filter;
                Outcome::Pending
            }
            _ => Outcome::Done(json_items(&v)), // undefined → []
        }
    }

    /// A task of `run` finished.
    fn advance(&mut self, run: &mut Run, res: TaskResult, js: &QuickJsSpecRuntime) -> Outcome {
        let v = match res {
            Ok(v) => v,
            Err(e) => {
                if let Some(k) = run.store.take() {
                    self.cache.fail(&k);
                }
                return Outcome::Fail(e);
            }
        };
        if let Some(k) = run.store.take() {
            self.cache.store(&k, v.clone(), now_ms());
        }
        if !self.is_current(run) {
            return Outcome::Done(Vec::new()); // superseded (e.g. after a debounce): abandon
        }
        let arg = run.arg.clone();
        let g = &arg.generators()[run.idx];
        match run.step {
            Step::Debounce => self.begin(run, js),
            Step::Script => self.script_input(run, g, v, js),
            Step::Exec => self.exec_output(run, g, &v, js),
            Step::Custom => self.custom_result(run, g, v, js),
            Step::PostProcess => Outcome::Done(post_process_items(&v, run.ctx.is_dangerous())),
            Step::Filter => Outcome::Done(json_items(&v)),
        }
    }

    /// Wait (one slice) on every running generator task; reply again when a current slot changed.
    fn pump(&mut self, js: &QuickJsSpecRuntime) {
        let mut tasks: Vec<TaskId> = self.runs.iter().map(|r| r.task).collect();
        tasks.sort_unstable();
        tasks.dedup();
        let deadline = self
            .runs
            .iter()
            .map(|r| r.deadline)
            .min()
            .map_or(Instant::now() + SLICE, |d| d.min(Instant::now() + SLICE));
        let results = match js.wait_json(&tasks, deadline) {
            Ok(r) => r,
            Err(SpecError::Aborted) => return, // a newer request is queued
            Err(
                e @ (SpecError::Timeout | SpecError::Invalid(_) | SpecError::BundleUnavailable(_)),
            ) => vec![Some(Err(e.to_string())); tasks.len()],
        };
        let mut changed = false;
        let mut runs = std::mem::take(&mut self.runs);
        for (task, res) in tasks.iter().zip(results) {
            let Some(res) = res else { continue };
            let (mine, rest): (Vec<Run>, Vec<Run>) =
                runs.into_iter().partition(|r| r.task == *task);
            runs = rest;
            // the owner (stores into the cache) goes first so sharers see a consistent cache
            let (owners, sharers): (Vec<Run>, Vec<Run>) =
                mine.into_iter().partition(|r| r.store.is_some());
            for mut run in owners.into_iter().chain(sharers) {
                let o = self.advance(&mut run, res.clone(), js);
                changed |= self.settle(run, o);
            }
            js.forget(&[*task]);
        }
        let now = Instant::now();
        let (expired, alive): (Vec<Run>, Vec<Run>) =
            runs.into_iter().partition(|r| now >= r.deadline);
        self.runs.extend(alive);
        for mut run in expired {
            if let Some(k) = run.store.take() {
                self.cache.fail(&k);
            }
            if !self.runs.iter().any(|r| r.task == run.task) {
                js.forget(&[run.task]);
            }
            changed |= self.settle(run, Outcome::Fail("timed out".into()));
        }
        if changed && self.cur.is_some() {
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
        if let Some((at, items)) = &self.first_token_cache {
            if at.elapsed() < FIRST_TOKEN_TTL {
                return items.clone();
            }
        }
        let executables = PathExecutableCatalog.executables(path.as_deref(), pathext.as_deref());
        let items = first_token_suggestions::first_token_items(
            self.index.completions(),
            aliases,
            &executables,
        );
        self.first_token_cache = Some((Instant::now(), items.clone()));
        items
    }

    /// Rank the current request's suggestions (SPEC §6.7.5) into a reply.
    fn reply(&self, phase: CompletionPhase) -> CompletionResult {
        let cur = self.cur.as_ref().expect("reply without a current request");
        let (pr, s) = (&cur.pr, &self.settings);
        let mut list = if pr.first_token() {
            cur.first.clone()
        } else {
            let views: Vec<GeneratorSlotView> = self
                .slots
                .iter()
                .zip(&cur.queries)
                .map(|(sl, q)| GeneratorSlotView::new(sl.key, q.clone(), sl.loading, &sl.result))
                .collect();
            suggestion_assembler::assemble(pr, &views)
        };
        let mode = s.string(BETA_HISTORY_MODE).unwrap_or_default();
        if cur.req.history_mode || mode == "show" || mode == "history_only" {
            let hist = history_suggestions::history_rows(&cur.prefix, &self.history.recent());
            list = history_suggestions::mix_history(list, hist, mode == "show" && !cur.req.history_mode);
        }
        let fo = FilterOptions::default()
            .with_fuzzy(cur.fuzzy)
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
        let map = Some(&self.recency).filter(|_| use_recency).and_then(|r| r.of_command(cmd));
        let ranked = suggestion_ranker::rank(list, pr.search_term(), map, &fo);
        let ic = InsertionContext::new(pr.search_term(), pr.raw_last_token(), cur.req.shell)
            .with_fuzzy(cur.fuzzy)
            .with_prefer_verbose(s.bool(AUTOCOMPLETE_PREFER_VERBOSE_SUGGESTIONS))
            .with_insert_space(s.bool(AUTOCOMPLETE_INSERT_SPACE_AUTOMATICALLY));
        let items: Vec<Suggestion> = ranked
            .into_iter()
            .map(|(x, p)| suggestion_finalizer::to_item(x, p, &ic))
            .collect();
        let ghost = self.ghost(&cur.req, cur.cursor, Some(&items));
        CompletionResult::new(CompletionResultParts {
            req: cur.req.id,
            phase,
            error: false,
            buffer: cur.req.buffer.clone(),
            cursor: cur.cursor,
            tokens: pr.tokens().clone(),
            command_text: cur.cmd_text.clone(),
            command_index: pr.command_index(),
            arg_key: cur.arg_key,
            current_arg: pr.current_arg().as_ref().map(|a| {
                ArgumentHint::new(
                    a.name().clone().unwrap_or_default(),
                    a.description().clone().unwrap_or_default(),
                )
            }),
            search_term: pr.search_term().to_string(),
            triggered_generators: if phase == CompletionPhase::Parse {
                cur.triggered.clone()
            } else {
                Vec::new()
            },
            items,
            loading: self.slots.iter().any(|s| s.loading),
            ghost,
            fuzzy: cur.fuzzy,
        })
    }

    /// Local inline ghost (SPEC §6.11). Also computed for parse-error replies (commands without a spec).
    fn ghost(&self, req: &CompletionRequest, cursor: usize, items: Option<&[Suggestion]>) -> Option<String> {
        if !self.settings.bool(INLINE_ENABLED)
            || req.buffer.is_empty()
            || cursor != req.buffer.len()
        {
            return None;
        }
        let history = || inline::history_ghost(&self.history.recent(), &req.buffer);
        let specs = || items.and_then(inline::spec_ghost);
        match inline::source(&self.settings) {
            Ok(Source::Specs) => specs(),
            Ok(Source::History) | Ok(Source::Profile(_)) => history(),
            _ => history().or_else(specs),
        }
    }

    // -----------------------------------------------------------------------------------------------------
    // Recency (SPEC §6.7.5) and LLM ghost text (SPEC §8.4)

    fn record(&mut self, cmd: String, name: String) {
        if self.recency.record(cmd, name, now_ms()) {
            self.recency_repository.save(&self.recency);
        }
    }

    fn llm(&mut self, req: u64, buffer: String, cwd: PathBuf, shell: Shell) {
        let tx = self.tx.clone();
        let Ok(Source::Profile(profile)) = inline::source(&self.settings) else {
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
        let history = llm_history(&self.settings, &self.history.recent());
        let env = self.env().clone();
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
    fn session_update_refreshes_first_token_items() {
        // first-token-cache-ignores-session-update: `alias zzal=…` at this prompt shows `zzal` at the next one
        let dir = TempDir::new("engine-session");
        let (tx, _rx) = std::sync::mpsc::channel();
        let js = Rc::new(QuickJsSpecRuntime::new(Rc::new(ChannelRuntimeEventSource::new()), &Settings::default()).unwrap());
        let mut e = Engine::with(
            tx,
            Settings::default(),
            parser_over(&js),
            SpecIndex::default(),
            HistoryModule::new(Rc::new(JsonlHistoryRepository::load(
                dir.path().join("h.jsonl"),
            ))),
            None,
        );
        assert!(e.first_token_items(&HashMap::new(), None, None).is_empty());
        assert!(e.handle(
            EngineMsg::Session { env: HashMap::new(), alias: None },
            &mut None,
            &js
        ));
        let aliases = HashMap::from([("zzal".to_string(), "git status".to_string())]);
        assert_eq!(
            e.first_token_items(&aliases, None, None)
                .iter()
                .map(|s| s.names()[0].as_str())
                .collect::<Vec<_>>(),
            ["zzal"]
        );
    }

    /// A parser over `js` and a spec repository that reads the env this process was started with.
    fn parser_over(js: &Rc<QuickJsSpecRuntime>) -> CommandLineParser {
        let bundle = EmbeddedSpecBundle::new(data_directory_from_env());
        CommandLineParser::new(Rc::new(QuickJsSpecRepository::new(js.clone(), bundle)), js.clone())
    }

    /// An engine whose wrapper env is `PATH=/wrapper/bin`, with the js runtime its parser and its `handle` share.
    fn engine_with_wrapper_env(dir: &TempDir) -> (Engine, Rc<QuickJsSpecRuntime>) {
        let (tx, _) = std::sync::mpsc::channel();
        let js = Rc::new(QuickJsSpecRuntime::new(Rc::new(ChannelRuntimeEventSource::new()), &Settings::default()).unwrap());
        let mut e = Engine::with(
            tx,
            Settings::default(),
            parser_over(&js),
            SpecIndex::default(),
            HistoryModule::new(Rc::new(JsonlHistoryRepository::load(
                dir.path().join("h.jsonl"),
            ))),
            None,
        );
        e.wrapper_env = wrapper_env();
        (e, js)
    }

    fn wrapper_env() -> HashMap<String, String> {
        HashMap::from([("PATH".to_string(), "/wrapper/bin".to_string())])
    }

    fn report_session_context(e: &mut Engine, js: &mut Rc<QuickJsSpecRuntime>, env: &[(&str, &str)], alias: Option<&str>) {
        let env = env
            .iter()
            .map(|(name, value)| (name.to_string(), value.to_string()))
            .collect();
        let alias = alias.map(str::to_owned);
        assert!(e.handle(EngineMsg::Session { env, alias }, &mut None, js));
    }

    #[test]
    fn should_use_the_env_of_a_session_context_in_place_of_the_wrapper_env() {
        let dir = TempDir::new("engine-session-env");
        let (mut e, mut js) = engine_with_wrapper_env(&dir);

        report_session_context(&mut e, &mut js, &[("PATH", "/session/bin")], None);

        assert_eq!(
            e.env(),
            &HashMap::from([("PATH".to_string(), "/session/bin".to_string())])
        );
    }

    /// An empty env means the shell reported none, not that it has none.
    #[test]
    fn should_fall_back_to_the_wrapper_env_after_a_session_context_with_an_empty_env() {
        let dir = TempDir::new("engine-session-empty-env");
        let (mut e, mut js) = engine_with_wrapper_env(&dir);
        report_session_context(&mut e, &mut js, &[("PATH", "/session/bin")], None);

        report_session_context(&mut e, &mut js, &[], None);

        assert_eq!(e.env(), &wrapper_env());
    }

    #[test]
    fn should_parse_the_aliases_of_a_session_context_for_the_shell() {
        let dir = TempDir::new("engine-session-aliases");
        let (mut e, mut js) = engine_with_wrapper_env(&dir);

        report_session_context(&mut e, &mut js, &[], Some("alias ll='ls -l'"));

        assert_eq!(
            e.alias_map(Shell::Bash),
            HashMap::from([("ll".to_string(), "ls -l".to_string())])
        );
    }

    #[test]
    fn should_find_no_aliases_after_a_session_context_with_an_empty_alias_text() {
        let dir = TempDir::new("engine-session-empty-alias");
        let (mut e, mut js) = engine_with_wrapper_env(&dir);
        report_session_context(&mut e, &mut js, &[], Some("alias ll='ls -l'"));
        e.alias_map(Shell::Bash);

        report_session_context(&mut e, &mut js, &[], Some(""));

        assert_eq!(e.alias_map(Shell::Bash), HashMap::new());
    }

    #[test]
    fn should_find_no_aliases_after_a_session_context_without_alias_text() {
        let dir = TempDir::new("engine-session-no-alias");
        let (mut e, mut js) = engine_with_wrapper_env(&dir);
        report_session_context(&mut e, &mut js, &[], Some("alias ll='ls -l'"));
        e.alias_map(Shell::Bash);

        report_session_context(&mut e, &mut js, &[], None);

        assert_eq!(e.alias_map(Shell::Bash), HashMap::new());
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
        let engine = Engine::with(replies, settings, parser_over(&runtime), SpecIndex::default(), history, None);
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
