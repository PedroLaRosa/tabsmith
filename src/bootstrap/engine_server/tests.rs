use super::*;
use crate::modules::history::domain::entities::history_entry::{HistoryEntry, HistoryEntryPrimitives};
use crate::modules::history::domain::repositories::history_repository::InMemoryHistoryRepository;
use crate::modules::history::infrastructure::adapters::jsonl_history_repository::JsonlHistoryRepository;
use crate::modules::completion::domain::value_objects::completion_phase::CompletionPhase;
use crate::modules::shared::domain::value_objects::data_directory::DataDirectory;
use crate::modules::shared::domain::value_objects::shell::Shell;
use crate::modules::shared::infrastructure::data_directory_from_env::data_directory_from_env;
use crate::modules::shared::infrastructure::system_clock::SystemClock;
use crate::modules::shared::infrastructure::testing::temp_dir::TempDir;
use std::path::Path;
use std::sync::Arc;
use crate::modules::settings::domain::services::settings_catalog::INLINE_ENABLED;
use crate::modules::settings::domain::value_objects::setting_value::SettingValue;
use crate::modules::settings::infrastructure::adapters::setting_value_json::settings_from_json;
use crate::modules::specs::application::ports::runtime_event_source::ChannelRuntimeEventSource;
use crate::modules::specs::domain::repositories::spec_repository::SpecRepository;
use serde_json::json;
use std::sync::mpsc::RecvTimeoutError;
use std::time::{Duration, Instant};

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
    let ghost_text = build_ghost_text_module(Rc::clone(&history_module.history));
    EngineServer::new(tx, session, completion, history_module, ghost_text)
}

/// The message a shell's session context arrives as: its env and its alias text.
fn session_context(env: &[(&str, &str)], alias: Option<&str>) -> EngineMessage {
    let env = env
        .iter()
        .map(|(name, value)| (name.to_string(), value.to_string()))
        .collect();
    let alias = alias.map(str::to_owned);
    EngineMessage::Session { env, alias }
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

fn complete(id: u64, cwd: &Path, buffer: &str) -> EngineMessage {
    complete_at(id, cwd, buffer, buffer.len())
}

fn complete_at(id: u64, cwd: &Path, buffer: &str, cursor: usize) -> EngineMessage {
    EngineMessage::Complete(CompletionRequest {
        id,
        buffer: buffer.into(),
        cursor,
        cwd: cwd.to_path_buf(),
        shell: Shell::Zsh,
        history_mode: false,
    })
}

fn command_ended() -> EngineMessage {
    EngineMessage::CommandEnded
}

fn apply_settings(settings: Settings) -> EngineMessage {
    EngineMessage::Settings(settings)
}

fn shutdown() -> EngineMessage {
    EngineMessage::Shutdown
}

/// What `serve`'s watcher does after a reply arrived.
enum Then {
    Wait,
    Deliver(Vec<EngineMessage>),
    Stop,
}

fn stop_when(done: impl Fn(&[Reply]) -> bool + Send + 'static) -> impl FnMut(&[Reply]) -> Then + Send + 'static {
    move |replies| if done(replies) { Then::Stop } else { Then::Wait }
}

/// Runs an engine over a real inbox, runtime and spec repository on this thread (QuickJS is !Send) and hands
/// the reply receiver and a sender to the engine's inbox to `watch`, which runs on a watcher thread and returns
/// what the test reads. It is sent `first` before the engine starts.
fn run_engine<T: Send + 'static>(
    settings: Settings,
    history: Vec<HistoryEntry>,
    first: Vec<EngineMessage>,
    watch: impl FnOnce(mpsc::Receiver<SessionMessage>, mpsc::Sender<EngineMessage>) -> T + Send + 'static,
) -> T {
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
    let ghost_text = build_ghost_text_module(Rc::clone(&history_module.history));
    let engine = EngineServer::new(replies, session, completion, history_module, ghost_text);
    for message in first {
        to_engine.send(message).unwrap();
    }
    let to_engine = to_engine.clone();
    let watcher = std::thread::spawn(move || watch(from_engine, to_engine));
    engine.run(&runtime, &inbox);
    watcher.join().unwrap()
}

/// Runs an engine over a real inbox, runtime and spec repository on this thread (QuickJS is !Send) and returns
/// every reply it sent. It is sent `first` before it starts; a watcher thread asks `react` after each reply.
/// After `Then::Stop` the watcher collects for `grace`, then shuts the engine down (after `SERVE_LIMIT` too).
fn serve(
    settings: Settings,
    history: Vec<HistoryEntry>,
    first: Vec<EngineMessage>,
    grace: Duration,
    mut react: impl FnMut(&[Reply]) -> Then + Send + 'static,
) -> Vec<Reply> {
    run_engine(settings, history, first, move |from_engine, to_engine| {
        let mut seen = Vec::new();
        let mut shutdown_at = Instant::now() + SERVE_LIMIT;
        let mut stopped = false;
        loop {
            match from_engine.recv_timeout(shutdown_at.saturating_duration_since(Instant::now())) {
                Ok(SessionMessage::Completion(c)) => {
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
                        SessionMessage::Completion(c) => Some(observe(&c)),
                        _ => None,
                    }));
                    return seen;
                }
                Err(RecvTimeoutError::Disconnected) => return seen,
            }
        }
    })
}

/// What a test reads of the LLM ghost texts an engine answered: `SessionMessage::Ghost { req, text }` as `(req, text)`.
type GhostReply = (u64, Option<String>);

/// Like `serve`, for the LLM path: returns the completion replies and the ghost replies the engine sent, in
/// order. The watcher shuts the engine down once `done` holds for the ghost replies (after `SERVE_LIMIT` too).
fn serve_ghosts(
    settings: Settings,
    first: Vec<EngineMessage>,
    done: impl Fn(&[GhostReply]) -> bool + Send + 'static,
) -> (Vec<Reply>, Vec<GhostReply>) {
    run_engine(settings, Vec::new(), first, move |from_engine, to_engine| {
        let mut completions = Vec::new();
        let mut ghosts = Vec::new();
        let mut stop_at = Instant::now() + SERVE_LIMIT;
        let mut stopped = false;
        loop {
            match from_engine.recv_timeout(stop_at.saturating_duration_since(Instant::now())) {
                Ok(SessionMessage::Completion(c)) => completions.push(observe(&c)),
                Ok(SessionMessage::Ghost { req, text }) => {
                    ghosts.push((req, text));
                    if !stopped && done(&ghosts) {
                        stopped = true;
                        let _ = to_engine.send(shutdown());
                        stop_at = stop_at.min(Instant::now() + Duration::from_millis(100));
                    }
                }
                Ok(_) => {}
                Err(RecvTimeoutError::Timeout) => {
                    let _ = to_engine.send(shutdown());
                    return (completions, ghosts);
                }
                Err(RecvTimeoutError::Disconnected) => return (completions, ghosts),
            }
        }
    })
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

#[test]
fn should_answer_an_llm_request_at_once_with_no_ghost_text_for_a_source_that_is_not_a_profile() {
    let dir = TempDir::new("engine-llm-no-profile");
    let request = EngineMessage::Llm { req: 7, buffer: "git sta".into(), cwd: dir.path().to_path_buf(), shell: Shell::Zsh };

    let (completions, ghosts) = serve_ghosts(Settings::default(), vec![request], |ghosts| ghosts.len() == 1);

    assert_eq!((completions, ghosts), (Vec::new(), vec![(7, None)]));
}

#[test]
fn should_answer_an_llm_request_with_no_ghost_text_when_the_provider_cannot_be_reached() {
    let dir = TempDir::new("engine-llm-unreachable");
    let settings = settings_from_json(
        json!({
            "inline.source": "mock",
            "inline.profiles.mock": {"provider": "openai", "model": "m", "base_url": "http://127.0.0.1:9/v1"}
        })
        .as_object()
        .unwrap()
        .clone(),
    );
    let request = EngineMessage::Llm { req: 7, buffer: "zzzcmd foo".into(), cwd: dir.path().to_path_buf(), shell: Shell::Zsh };

    let (_, ghosts) = serve_ghosts(settings, vec![request], |ghosts| ghosts.len() == 1);

    assert_eq!(ghosts, [(7, None)]);
}
