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
use crate::completion::application::completion_notice::CompletionNotice;
use crate::completion::domain::entities::completion_session::CompletionSession;
use crate::completion::infrastructure::completion_module::{build_completion_module, CompletionModule};
use crate::settings::domain::services::settings_catalog::{INLINE_ENABLED, INLINE_HISTORY_COUNT};
use crate::settings::domain::value_objects::settings::Settings;
use crate::completion::application::completion_request::CompletionRequest;
use crate::completion::domain::value_objects::completion_result::CompletionResult;
use crate::specs::application::ports::runtime_event::RuntimeEvent;
use crate::specs::application::ports::runtime_event_poll::RuntimeEventPoll;
use crate::specs::application::ports::runtime_event_sink::RuntimeEventSink;
use crate::specs::application::ports::runtime_event_source::RuntimeEventSource;
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
use crate::types::{
    EngineMsg, Msg,
};
use std::cell::RefCell;
use std::collections::VecDeque;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// Spawn the engine thread. Replies go to `tx` as `Msg::Completion` / `Msg::Ghost`.
/// Returns the engine's inbox. The thread exits on `EngineMsg::Shutdown` or when all senders are dropped.
pub(crate) fn spawn(tx: Sender<Msg>, settings: Settings) -> Sender<EngineMsg> {
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
    fn handle(&mut self, m: EngineMsg, pending: &mut Option<CompletionRequest>) -> bool {
        match m {
            EngineMsg::Complete(r) => *pending = Some(r),
            EngineMsg::CommandFinished(_) | EngineMsg::CommandEnded => {
                if let EngineMsg::CommandFinished(e) = m {
                    self.record_command.execute(e);
                }
                self.clear_ghost_text_cache();
                // generator results without `cache` live until the command ends
                self.completion.reset_generators.execute(&mut self.session);
            }
            EngineMsg::RecordInsert { cmd, name } => {
                self.completion.record_suggestion_insertion.execute(&mut self.session, cmd, name)
            }
            EngineMsg::Llm {
                req,
                buffer,
                cwd,
                shell,
            } => self.request_llm_ghost_text(req, buffer, cwd, shell),
            EngineMsg::Settings(s) => self.completion.apply_completion_settings.execute(&mut self.session, s),
            EngineMsg::Session { env, alias } => {
                self.completion.apply_session_context.execute(&mut self.session, env, alias)
            }
            EngineMsg::ToggleFuzzy => self.completion.toggle_fuzzy_search.execute(&mut self.session),
            EngineMsg::Runtime(_) => {} // handed to the js host by the inbox
            EngineMsg::Shutdown => return false,
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
        match inline::source(self.session.settings()) {
            Ok(Source::Specs) => specs(),
            Ok(Source::History) | Ok(Source::Profile(_)) => history(),
            _ => history().or_else(specs),
        }
    }

    /// LLM ghost text (SPEC §8.4).
    fn request_llm_ghost_text(&mut self, req: u64, buffer: String, cwd: PathBuf, shell: Shell) {
        let tx = self.replies.clone();
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
mod tests {
    use super::*;
    use crate::history::domain::entities::history_entry::HistoryEntryPrimitives;
    use crate::history::domain::repositories::history_repository::InMemoryHistoryRepository;
    use crate::history::infrastructure::adapters::jsonl_history_repository::JsonlHistoryRepository;
    use crate::completion::domain::value_objects::completion_phase::CompletionPhase;
    use crate::shared::domain::value_objects::data_directory::DataDirectory;
    use crate::shared::infrastructure::data_directory_from_env::data_directory_from_env;
    use crate::shared::infrastructure::system_clock::SystemClock;
    use crate::shared::infrastructure::testing::temp_dir::TempDir;
    use std::path::Path;
    use crate::settings::domain::value_objects::setting_value::SettingValue;
    use crate::settings::infrastructure::adapters::setting_value_json::settings_from_json;
    use crate::specs::application::ports::runtime_event_source::ChannelRuntimeEventSource;
    use crate::specs::application::ports::spec_runtime::SpecRuntime;
    use crate::specs::domain::repositories::spec_repository::SpecRepository;
    use crate::specs::domain::value_objects::process_output::ProcessOutput;
    use crate::specs::domain::value_objects::exec_request::ExecRequest;
    use crate::specs::domain::spec_error::SpecError;
    use crate::specs::domain::value_objects::generator_context::GeneratorContext;
    use serde_json::{json, Value};
    use std::collections::HashMap;
    use std::time::Instant;

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

    /// A spec repository over `js` that reads the env this process was started with.
    fn spec_repository_over(js: &Rc<QuickJsSpecRuntime>) -> Rc<dyn SpecRepository> {
        let bundle = EmbeddedSpecBundle::new(data_directory_from_env());
        Rc::new(QuickJsSpecRepository::new(js.clone(), bundle))
    }

    /// What the engine is built from in the tests: no data directory, so there is no recency file.
    fn without_a_data_directory() -> SharedDependencies {
        SharedDependencies {
            data_directory: DataDirectory::new(None, None),
            clock: Arc::new(SystemClock),
        }
    }

    /// An engine over a real runtime and a JSONL history in `dir`, whose replies go nowhere.
    fn engine_over(dir: &TempDir) -> EngineServer {
        let (tx, _) = std::sync::mpsc::channel();
        let js = Rc::new(QuickJsSpecRuntime::new(Rc::new(ChannelRuntimeEventSource::new()), &Settings::default()).unwrap());
        let history_module = HistoryModule::new(Rc::new(JsonlHistoryRepository::load(
            dir.path().join("h.jsonl"),
        )));
        let completion = build_completion_module(
            &without_a_data_directory(),
            spec_repository_over(&js),
            js.clone(),
            history_module.history.clone(),
            SpecIndex::default(),
        );
        let session = CompletionSession::new(Settings::default(), std::env::vars().collect(), completion.recency.load());
        EngineServer::new(tx, session, completion, history_module)
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
        let history_module = HistoryModule::new(Rc::new(InMemoryHistoryRepository::new(history)));
        let completion = build_completion_module(
            &without_a_data_directory(),
            spec_repository_over(&runtime),
            runtime.clone(),
            history_module.history.clone(),
            SpecIndex::default(),
        );
        let session = CompletionSession::new(settings, std::env::vars().collect(), completion.recency.load());
        let engine = EngineServer::new(replies, session, completion, history_module);
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
