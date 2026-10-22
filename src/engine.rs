//! The completion engine thread (SPEC §4.3, §6): owns the JS host, spec/parse caches, generator state,
//! recency, history and the inline cache. Serves only the latest request.
//! OWNER: engine agent.
//!
//! Per request: mid-word guard → `shell_parser::current_command` → `Parser::parse` → generator triggers →
//! reply (phase `Parse`). Triggered generators run as small state machines over `Js` tasks; whenever one
//! finishes, the latest request is re-ranked and replied again (phase `Generators`).
use crate::inline::{self, GhostCache, History, Source};
use crate::js::{ExecInput, GenCtx, Js, JsArg, JsError, TaskId, TaskResult};
use crate::logger;
use crate::parser::{ParseCtx, ParseError, ParseResult, Parser};
use crate::settings::{Settings, Store};
use crate::shell_parser;
use crate::spec::{Arg, Generator, Script, StrOrFn, Strategy, Suggestion, Trigger};
use crate::specs::SpecIndex;
use crate::suggest::{self, FilterOpts, GenCache, GenView, InsCtx, Lookup, Query, Sugg};
use crate::types::{now_ms, ArgInfo, Completion, EngineMsg, GenKey, HistoryEntry, Item, ItemType, Msg, Phase, Request, SessionCtx, Shell};
use serde_json::{json, Value};
use std::collections::hash_map::DefaultHasher;
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, Ordering::SeqCst};
use std::sync::mpsc::{self, Sender};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// Pump slice while generators run: deferred messages and finished runs are handled at least this often.
const SLICE: Duration = Duration::from_millis(50);
/// Timeout for synchronous spec callbacks (function triggers, `getQueryTerm`).
const SYNC_CALL: Duration = Duration::from_secs(1);
const FIRST_TOKEN_TTL: Duration = Duration::from_secs(10);
const RECENCY_KEY: &str = "autocomplete.recency";

/// `recency[cmd][name] = unix ms of the last insertion` (SPEC §3.2 `state.json`).
pub type Recency = HashMap<String, HashMap<String, u64>>;

/// Spawn the engine thread. Replies go to `tx` as `Msg::Completion` / `Msg::Ghost`.
/// Returns the engine's inbox. The thread exits on `EngineMsg::Shutdown` or when all senders are dropped.
pub fn spawn(tx: Sender<Msg>, settings: Settings) -> Sender<EngineMsg> {
    let (etx, erx) = mpsc::channel();
    let inner = etx.clone();
    let spawned = std::thread::Builder::new().name("engine".into()).spawn(move || {
        // QuickJS is !Send: the runtime is created on this thread.
        match Js::new(erx, inner, settings.clone()) {
            Ok(mut js) => Engine::new(tx, settings).run(&mut js),
            Err(e) => logger::error(&format!("engine: js host: {e:#}")),
        }
    });
    if let Err(e) = spawned {
        logger::error(&format!("engine: spawn: {e}"));
    }
    etx
}

/// Load `autocomplete.recency` from a state file (missing/corrupt → empty).
pub fn load_recency(path: &Path) -> Recency {
    Store::load(path)
        .ok()
        .and_then(|s| s.map.get(RECENCY_KEY).cloned())
        .and_then(|v| serde_json::from_value(v).ok())
        .unwrap_or_default()
}

/// Read-modify-write `autocomplete.recency` into a state file (other keys kept). Merged with what is on disk
/// (newest timestamp wins), so concurrent sessions keep each other's entries.
pub fn save_recency(path: &Path, rec: &Recency) -> anyhow::Result<()> {
    let mut store = Store::load(path)?;
    let mut merged: Recency = store.map.get(RECENCY_KEY).cloned().and_then(|v| serde_json::from_value(v).ok()).unwrap_or_default();
    for (cmd, names) in rec {
        let m = merged.entry(cmd.clone()).or_default();
        for (name, &ts) in names {
            let e = m.entry(name.clone()).or_insert(0);
            *e = (*e).max(ts);
        }
    }
    store.map.insert(RECENCY_KEY.into(), serde_json::to_value(merged)?);
    store.save()
}

fn hash2(a: usize, b: usize) -> u64 {
    let mut h = DefaultHasher::new();
    (a, b).hash(&mut h);
    h.finish()
}

/// Generator identity: arg identity + generator index.
fn gen_key(arg: &Rc<Arg>, idx: usize) -> GenKey {
    hash2(Rc::as_ptr(arg) as usize, idx)
}

/// `filepaths` / `folders` templates behave like the bundled `filepaths()` helper (trigger, query term).
fn is_path_template(g: &Generator) -> bool {
    g.template.iter().any(|t| t == "filepaths" || t == "folders")
}

/// JS truthiness of a callback result.
fn truthy(v: &Value) -> bool {
    match v {
        Value::Null => false,
        Value::Bool(b) => *b,
        Value::Number(n) => n.as_f64().is_some_and(|f| f != 0.0 && !f.is_nan()),
        Value::String(s) => !s.is_empty(),
        _ => true,
    }
}

fn env_var(env: &HashMap<String, String>, key: &str) -> Option<String> {
    env.get(key)
        .or_else(|| if cfg!(windows) { env.iter().find(|(k, _)| k.eq_ignore_ascii_case(key)).map(|(_, v)| v) } else { None })
        .cloned()
}

/// `autocomplete.scriptTimeout` (5000 ms; Windows 20000).
fn script_timeout(settings: &Settings) -> u64 {
    settings.f64("autocomplete.scriptTimeout", if cfg!(windows) { 20000.0 } else { 5000.0 }).max(0.0) as u64
}

/// The history an LLM request may send: the last `inline.historyCount` (49) usable entries (SPEC §8.5).
fn llm_history(settings: &Settings, entries: &[HistoryEntry]) -> Vec<HistoryEntry> {
    inline::llm_history(entries, settings.i64("inline.historyCount", 49).max(0) as usize)
}

/// Script value → exec input JSON (SPEC §6.6.3); `None` for falsy values. A legacy string runs as `sh -c`, the
/// same form as `executeShellCommand(string)`, so the js host picks sh / Git Bash / `cmd /C` on Windows.
fn script_exec_input(v: Value, cwd: &str) -> Option<Value> {
    let cwd = json!(cwd);
    Some(match v {
        Value::Array(a) if !a.is_empty() => json!({"command": a[0], "args": a[1..], "cwd": cwd}),
        Value::String(s) if !s.is_empty() => json!({"command": "sh", "args": ["-c", s], "cwd": cwd}), // none in 2.692.3
        Value::Object(o) => {
            let mut m = serde_json::Map::new();
            m.insert("cwd".into(), cwd);
            m.extend(o);
            Value::Object(m)
        }
        _ => return None,
    })
}

/// Script-generator string → `{type:"arg", name, insertValue, isDangerous}`.
fn script_string(s: &str, is_dangerous: bool) -> Suggestion {
    Suggestion {
        names: vec![s.to_string()],
        insert_value: Some(s.to_string()),
        kind: Some(ItemType::Arg),
        is_dangerous,
        ..Default::default()
    }
}

/// Suggestion objects (custom / filterTemplateSuggestions results).
fn json_items(v: &Value) -> Vec<Suggestion> {
    v.as_array().map(|a| a.iter().filter_map(crate::spec::suggestion_from_json).collect()).unwrap_or_default()
}

/// `postProcess` result: drop falsy items and objects without a name; strings become arg rows.
fn post_process_items(v: &Value, is_dangerous: bool) -> Vec<Suggestion> {
    let Some(a) = v.as_array() else { return Vec::new() };
    a.iter()
        .filter_map(|x| match x {
            Value::String(s) if !s.is_empty() => Some(script_string(s, is_dangerous)),
            Value::Object(o) if o.get("name").is_some_and(truthy) => crate::spec::suggestion_from_json(x),
            _ => None,
        })
        .collect()
}

/// `generatorContext` for custom generators (SPEC §6.5).
fn gen_context_json(ctx: &GenCtx) -> Value {
    json!({"currentWorkingDirectory": ctx.cwd, "currentProcess": ctx.shell, "sshPrefix": "",
           "searchTerm": ctx.search_term, "environmentVariables": ctx.env, "isDangerous": ctx.is_dangerous})
}

/// Generator results of the current argument, one per generator (SPEC §6.6.1).
struct Slot {
    key: GenKey,
    result: Vec<Suggestion>,
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

/// One generator run waiting on a `Js` task.
#[derive(Clone)]
struct Run {
    slot: usize,
    gen_id: u64,
    /// Stale-while-revalidate refresh: its failure leaves the slot alone.
    background: bool,
    arg: Rc<Arg>,
    idx: usize,
    ctx: GenCtx,
    tokens: Vec<String>,
    help: Vec<Suggestion>,
    step: Step,
    task: TaskId,
    /// Cache key this run's task result is stored under (owner of an in-flight fetch).
    store: Option<String>,
    deadline: Instant,
}

enum Outcome {
    Pending,
    Done(Vec<Suggestion>),
    Fail(String),
}

/// The latest served request (for phase-Generators replies).
struct Current {
    req: Request,
    cursor: usize,
    pr: ParseResult,
    cmd_text: String,
    arg_key: u64,
    fuzzy: bool,
    queries: Vec<Query>,
    triggered: Vec<GenKey>,
    first: Vec<Sugg>,
    /// Buffer up to the start of the last token (history rows).
    prefix: String,
}

struct Engine {
    tx: Sender<Msg>,
    settings: Settings,
    parser: Parser,
    index: SpecIndex,
    history: History,
    session: SessionCtx,
    wrapper_env: HashMap<String, String>,
    aliases: Option<(Shell, HashMap<String, String>)>,
    recency: Arc<Mutex<Recency>>,
    saving: Arc<AtomicBool>,
    state_path: Option<PathBuf>,
    ghost_cache: Arc<Mutex<GhostCache>>,
    user_fuzzy: bool,
    last_arg_key: u64,
    slot_arg: Option<Rc<Arg>>,
    slots: Vec<Slot>,
    prev_search: String,
    next_gen: u64,
    runs: Vec<Run>,
    cache: GenCache,
    cur: Option<Current>,
    first_token_cache: Option<(Instant, Vec<Sugg>)>,
}

impl Engine {
    fn new(tx: Sender<Msg>, settings: Settings) -> Engine {
        let index = crate::specs::ensure_extracted().and_then(|_| SpecIndex::load()).unwrap_or_else(|e| {
            logger::error(&format!("engine: specs: {e:#}"));
            SpecIndex::default()
        });
        Engine::with(tx, settings, index, History::load(), crate::settings::state_path().ok())
    }

    fn with(tx: Sender<Msg>, settings: Settings, index: SpecIndex, history: History, state_path: Option<PathBuf>) -> Engine {
        let recency = state_path.as_deref().map(load_recency).unwrap_or_default();
        Engine {
            tx,
            user_fuzzy: settings.bool("autocomplete.fuzzySearch", false),
            settings,
            parser: Parser::new(),
            index,
            history,
            session: SessionCtx::default(),
            wrapper_env: std::env::vars().collect(),
            aliases: None,
            recency: Arc::new(Mutex::new(recency)),
            saving: Arc::new(AtomicBool::new(false)),
            state_path,
            ghost_cache: Arc::new(Mutex::new(GhostCache::default())),
            last_arg_key: 0,
            slot_arg: None,
            slots: Vec::new(),
            prev_search: String::new(),
            next_gen: 0,
            runs: Vec::new(),
            cache: GenCache::default(),
            cur: None,
            first_token_cache: None,
        }
    }

    fn run(mut self, js: &mut Js) {
        let mut pending: Option<Request> = None;
        'outer: loop {
            if pending.is_none() && self.runs.is_empty() {
                match js.next_msg() {
                    Some(m) => {
                        if !self.handle(m, &mut pending, js) {
                            break;
                        }
                    }
                    None => break,
                }
            }
            // only the latest request is served: drain everything queued first
            while let Some(m) = js.try_next_msg() {
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
        if self.saving.load(SeqCst) {
            self.save_recency_now();
        }
    }

    /// Handle one message; `false` = shut down.
    fn handle(&mut self, m: EngineMsg, pending: &mut Option<Request>, js: &mut Js) -> bool {
        match m {
            EngineMsg::Complete(r) => *pending = Some(r),
            EngineMsg::CommandFinished(e) => {
                self.history.append(e);
                if let Ok(mut c) = self.ghost_cache.lock() {
                    c.clear();
                }
                self.reset_slots(); // generator results without `cache` live until the command ends
                self.cur = None;
            }
            EngineMsg::RecordInsert { cmd, name } => self.record(cmd, name),
            EngineMsg::Llm { req, buffer, cwd, shell } => self.llm(req, buffer, cwd, shell),
            EngineMsg::Settings(s) => {
                js.set_settings(s.clone());
                self.settings = s;
                self.parser.clear_caches();
                self.reset_slots();
                self.first_token_cache = None;
            }
            EngineMsg::Session(s) => {
                self.session = s;
                self.aliases = None;
                self.first_token_cache = None; // aliases / PATH from this prompt
            }
            EngineMsg::ToggleFuzzy => self.user_fuzzy = !self.user_fuzzy,
            EngineMsg::ExecDone { .. } | EngineMsg::Timer { .. } => {} // consumed by the js host
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
        if self.session.env.is_empty() {
            &self.wrapper_env
        } else {
            &self.session.env
        }
    }

    fn alias_map(&mut self, shell: Shell) -> HashMap<String, String> {
        if !matches!(&self.aliases, Some((s, _)) if *s == shell) {
            let map = match self.session.alias.as_deref() {
                Some(text) if !text.is_empty() => shell_parser::parse_aliases(text, shell),
                _ => HashMap::new(),
            };
            self.aliases = Some((shell, map));
        }
        self.aliases.as_ref().map(|(_, m)| m.clone()).unwrap_or_default()
    }

    // -----------------------------------------------------------------------------------------------------
    // Requests

    /// Phase-Parse reply for `req`; `None` when a newer request aborted it (it is queued in `js`).
    fn compute(&mut self, req: &Request, js: &mut Js) -> Option<Completion> {
        let cursor = req.cursor.min(req.buffer.len());
        // SPEC §6.1.5 mid-word guard
        if !req.buffer.is_char_boundary(cursor) || req.buffer[cursor..].chars().next().is_some_and(|c| !c.is_whitespace()) {
            return Some(self.error(req, cursor, false));
        }
        let before = &req.buffer[..cursor];
        let aliases = self.alias_map(req.shell);
        let Some(cmd) = shell_parser::current_command(before, req.shell, &aliases) else {
            return Some(self.error(req, cursor, true));
        };
        let env = if self.session.env.is_empty() { &self.wrapper_env } else { &self.session.env };
        let pctx = ParseCtx { cwd: &req.cwd, settings: &self.settings, env, shell: req.shell, index: &self.index };
        let pr = match self.parser.parse(&cmd, &pctx, js) {
            Ok(pr) => pr,
            Err(ParseError::Aborted) => return None,
            Err(ParseError::NoSuggestions(e)) => {
                logger::debug(&format!("engine: no suggestions: {e}"));
                return Some(self.error(req, cursor, true));
            }
        };
        let arg = pr.current_arg.clone();
        let arg_key = hash2(arg.as_ref().map_or(0, |a| Rc::as_ptr(a) as usize), Rc::as_ptr(&pr.completion) as usize);
        if arg_key != self.last_arg_key {
            self.user_fuzzy = self.settings.bool("autocomplete.fuzzySearch", false);
            self.last_arg_key = arg_key;
        }
        let strategy = arg.as_ref().map_or(pr.completion.filter_strategy, |a| a.filter_strategy);
        let fuzzy = match strategy {
            Strategy::Prefix => false,
            Strategy::Fuzzy => true,
            Strategy::Default => self.user_fuzzy,
        };
        let ctx = GenCtx {
            cwd: req.cwd.to_string_lossy().into_owned(),
            env: env.clone(),
            shell: req.shell.name().into(),
            search_term: pr.search_term.clone(),
            is_dangerous: arg.as_ref().is_some_and(|a| a.is_dangerous),
        };
        let (path, pathext) = (env_var(env, "PATH"), env_var(env, "PATHEXT"));
        let queries = self.queries(&pr, req.shell, js)?;
        let tokens = pr.tokens.get(pr.command_index..).unwrap_or_default().to_vec();
        let triggered = self.update_slots(&pr, &ctx, &tokens, js)?;
        let first = if pr.first_token && self.settings.bool("autocomplete.firstTokenCompletion", false) {
            self.first_token_items(&aliases, path, pathext)
        } else {
            Vec::new()
        };
        let prefix = before.strip_suffix(pr.raw_last_token.as_str()).unwrap_or(before).to_string();
        self.cur = Some(Current { req: req.clone(), cursor, pr, cmd_text: cmd.original_text, arg_key, fuzzy, queries, triggered, first, prefix });
        Some(self.reply(Phase::Parse))
    }

    /// Error reply (menu error-reset). Kiro resets generator state on errors too.
    fn error(&mut self, req: &Request, cursor: usize, ghost: bool) -> Completion {
        self.reset_slots();
        self.cur = None;
        let mut c = Completion::error(req.id, req.buffer.clone(), cursor);
        if ghost {
            c.ghost = self.ghost(req, cursor, None);
        }
        c
    }

    /// Query derivation per generator of the current arg (function `getQueryTerm`s are called once here).
    fn queries(&self, pr: &ParseResult, shell: Shell, js: &mut Js) -> Option<Vec<Query>> {
        let Some(arg) = &pr.current_arg else { return Some(Vec::new()) };
        let mut out = Vec::with_capacity(arg.generators.len());
        for g in &arg.generators {
            out.push(if is_path_template(g) {
                Query::Path { backslash: shell == Shell::Pwsh }
            } else {
                match &g.get_query_term {
                    None => Query::Term,
                    Some(StrOrFn::Str(s)) => Query::Str(s.clone()),
                    Some(StrOrFn::Fn(f)) => match js.call(*f, vec![JsArg::Json(json!(pr.search_term))], None, SYNC_CALL) {
                        Ok(Value::String(s)) => Query::Value(s),
                        Err(JsError::Aborted) => return None,
                        _ => Query::Value(pr.search_term.clone()), // error → search term
                    },
                }
            });
        }
        Some(out)
    }

    /// SPEC §6.6.1: decide triggers (no mutation until every JS trigger answered), then start runs.
    fn update_slots(&mut self, pr: &ParseResult, ctx: &GenCtx, tokens: &[String], js: &mut Js) -> Option<Vec<GenKey>> {
        let arg = pr.current_arg.clone();
        let same = match (&arg, &self.slot_arg) {
            (Some(a), Some(b)) => Rc::ptr_eq(a, b),
            _ => false,
        };
        let (new, old) = (pr.search_term.as_str(), self.prev_search.as_str());
        let gens: &[Generator] = arg.as_deref().map_or(&[], |a| &a.generators);
        let mut fire = Vec::with_capacity(gens.len());
        for (i, g) in gens.iter().enumerate() {
            fire.push(if !same || i >= self.slots.len() {
                true
            } else if is_path_template(g) {
                suggest::dir_changed(new, old, ctx.shell == Shell::Pwsh.name())
            } else {
                match &g.trigger {
                    None => arg.as_ref().is_some_and(|a| a.debounce.is_some()),
                    Some(Trigger::Fn(f)) => match js.call(*f, vec![JsArg::Json(json!(new)), JsArg::Json(json!(old))], None, SYNC_CALL) {
                        Ok(v) => truthy(&v),
                        Err(JsError::Aborted) => return None,
                        Err(_) => true, // a throwing trigger counts as a trigger
                    },
                    Some(t) => suggest::triggered(t, new, old).unwrap_or(true),
                }
            });
        }
        self.prev_search = pr.search_term.clone();
        let Some(arg) = arg.as_ref() else {
            self.slots.clear();
            self.slot_arg = None;
            return Some(Vec::new());
        };
        if !same {
            // ponytail: results are not carried over to a different arg (qac keeps them while loading)
            self.slots = (0..gens.len()).map(|i| Slot { key: gen_key(arg, i), result: Vec::new(), loading: false, gen_id: 0 }).collect();
            self.slot_arg = Some(arg.clone());
        }
        let help = if gens.iter().any(|g| g.template.iter().any(|t| t == "help")) { suggest::help_items(pr) } else { Vec::new() };
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
            let debounce = arg.debounce.map(|ms| if ms > 0 { ms } else { 200 });
            // ponytail: one deadline for the whole run (custom generators may exec several times)
            let budget = debounce.unwrap_or(0) + 2 * floor.max(gens[i].script_timeout.unwrap_or(0)) + 1000;
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
        self.slots.get(run.slot).is_some_and(|s| s.gen_id == run.gen_id)
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
    fn begin(&mut self, run: &mut Run, js: &mut Js) -> Outcome {
        let arg = run.arg.clone();
        let g = &arg.generators[run.idx];
        if !g.template.is_empty() {
            let mut items = Vec::new();
            let files = g.template.iter().any(|t| t == "filepaths");
            if is_path_template(g) && !run.ctx.cwd.is_empty() {
                let dir = suggest::inserted_directory(&run.ctx.cwd, &run.ctx.search_term, &run.ctx.env, run.ctx.shell == Shell::Pwsh.name());
                items = suggest::list_dir(&dir, files, run.ctx.is_dangerous);
            }
            items.extend(run.help.iter().cloned()); // `history` template: [] (F6)
            return match g.filter_template_suggestions {
                Some(f) => {
                    run.task = js.start_call(f, vec![JsArg::Json(suggest::template_json(&items))], Some(&run.ctx));
                    run.step = Step::Filter;
                    Outcome::Pending
                }
                None => Outcome::Done(items),
            };
        }
        if run.ctx.cwd.is_empty() {
            return Outcome::Done(Vec::new()); // no context for script/custom generators
        }
        if let Some(script) = &g.script {
            return match script {
                Script::Fn(f) => {
                    run.task = js.start_call(*f, vec![JsArg::Json(json!(run.tokens))], Some(&run.ctx));
                    run.step = Step::Script;
                    Outcome::Pending
                }
                Script::Argv(a) => self.script_input(run, g, json!(a), js),
                Script::Shell(s) => self.script_input(run, g, json!(s), js),
                Script::Object(o) => self.script_input(run, g, o.clone(), js),
            };
        }
        let Some(custom) = g.custom else { return Outcome::Done(Vec::new()) };
        let key = g.cache.as_ref().map(|c| suggest::cache_key(c, &run.ctx.cwd, c.cache_key.clone().unwrap_or_else(|| run.tokens.join(" "))));
        let args = vec![JsArg::Json(json!(run.tokens)), JsArg::Exec, JsArg::Json(gen_context_json(&run.ctx))];
        let ctx = run.ctx.clone();
        match self.fetch(run, g, key, Step::Custom, js, move |js| js.start_call(custom, args, Some(&ctx))) {
            Ok(v) => self.custom_result(run, g, v, js),
            Err(o) => o,
        }
    }

    /// Cached fetch (D10): a value now (`Ok`), or a started/shared task (`Err(Pending)`).
    fn fetch(&mut self, run: &mut Run, g: &Generator, key: Option<String>, step: Step, js: &mut Js, start: impl FnOnce(&mut Js) -> TaskId) -> Result<Value, Outcome> {
        let (Some(cfg), Some(key)) = (&g.cache, key) else {
            run.task = start(js);
            run.step = step;
            return Err(Outcome::Pending);
        };
        match self.cache.lookup(&key, cfg, now_ms()) {
            Lookup::Fresh(v) => Ok(v),
            Lookup::Stale { value, refresh } => {
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
            Lookup::Shared(t) => {
                run.task = t;
                run.step = step;
                Err(Outcome::Pending)
            }
            Lookup::Miss => {
                run.task = start(js);
                run.step = step;
                self.cache.start(&key, run.task);
                run.store = Some(key);
                Err(Outcome::Pending)
            }
        }
    }

    /// SPEC §6.6.3: script value → exec input → (cached) exec.
    fn script_input(&mut self, run: &mut Run, g: &Generator, v: Value, js: &mut Js) -> Outcome {
        let Some(mut input) = script_exec_input(v, &run.ctx.cwd) else { return Outcome::Done(Vec::new()) }; // falsy → []
        let key_src = input.to_string();
        let timeout = script_timeout(&self.settings)
            .max(g.script_timeout.unwrap_or(0))
            .max(input.get("timeout").and_then(Value::as_u64).unwrap_or(0));
        input["timeout"] = json!(timeout);
        let exec: ExecInput = match serde_json::from_value(input) {
            Ok(i) => i,
            Err(e) => return Outcome::Fail(format!("bad script input: {e}")),
        };
        let key = g.cache.as_ref().map(|c| suggest::cache_key(c, &run.ctx.cwd, c.cache_key.clone().unwrap_or(key_src)));
        let ctx = run.ctx.clone();
        match self.fetch(run, g, key, Step::Exec, js, move |js| js.start_exec(exec, &ctx)) {
            Ok(v) => self.exec_output(run, g, &v, js),
            Err(o) => o,
        }
    }

    /// `splitOn` takes precedence over `postProcess`; neither → [].
    fn exec_output(&mut self, run: &mut Run, g: &Generator, v: &Value, js: &mut Js) -> Outcome {
        let stdout = v.get("stdout").and_then(Value::as_str).unwrap_or("");
        if let Some(sep) = &g.split_on {
            let t = stdout.trim();
            if t.is_empty() {
                return Outcome::Done(Vec::new());
            }
            return Outcome::Done(t.split(sep.as_str()).map(|s| script_string(s, run.ctx.is_dangerous)).collect());
        }
        let Some(pp) = g.post_process else { return Outcome::Done(Vec::new()) };
        run.task = js.start_call(pp, vec![JsArg::Json(json!(stdout)), JsArg::Json(json!(run.tokens))], Some(&run.ctx));
        run.step = Step::PostProcess;
        Outcome::Pending
    }

    /// Custom result; `filterTemplateSuggestions` applies when the first item has a `context`.
    fn custom_result(&mut self, run: &mut Run, g: &Generator, v: Value, js: &mut Js) -> Outcome {
        let templated = v.as_array().and_then(|a| a.first()).is_some_and(|x| x.get("context").is_some());
        match g.filter_template_suggestions {
            Some(f) if templated => {
                run.task = js.start_call(f, vec![JsArg::Json(v)], Some(&run.ctx));
                run.step = Step::Filter;
                Outcome::Pending
            }
            _ => Outcome::Done(json_items(&v)), // undefined → []
        }
    }

    /// A task of `run` finished.
    fn advance(&mut self, run: &mut Run, res: TaskResult, js: &mut Js) -> Outcome {
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
        let g = &arg.generators[run.idx];
        match run.step {
            Step::Debounce => self.begin(run, js),
            Step::Script => self.script_input(run, g, v, js),
            Step::Exec => self.exec_output(run, g, &v, js),
            Step::Custom => self.custom_result(run, g, v, js),
            Step::PostProcess => Outcome::Done(post_process_items(&v, run.ctx.is_dangerous)),
            Step::Filter => Outcome::Done(json_items(&v)),
        }
    }

    /// Wait (one slice) on every running generator task; reply again when a current slot changed.
    fn pump(&mut self, js: &mut Js) {
        let mut tasks: Vec<TaskId> = self.runs.iter().map(|r| r.task).collect();
        tasks.sort_unstable();
        tasks.dedup();
        let deadline = self.runs.iter().map(|r| r.deadline).min().map_or(Instant::now() + SLICE, |d| d.min(Instant::now() + SLICE));
        let results = match js.wait(&tasks, deadline) {
            Ok(r) => r,
            Err(JsError::Aborted) => return, // a newer request is queued
            Err(e) => vec![Some(Err(e.to_string())); tasks.len()],
        };
        let mut changed = false;
        let mut runs = std::mem::take(&mut self.runs);
        for (task, res) in tasks.iter().zip(results) {
            let Some(res) = res else { continue };
            let (mine, rest): (Vec<Run>, Vec<Run>) = runs.into_iter().partition(|r| r.task == *task);
            runs = rest;
            // the owner (stores into the cache) goes first so sharers see a consistent cache
            let (owners, sharers): (Vec<Run>, Vec<Run>) = mine.into_iter().partition(|r| r.store.is_some());
            for mut run in owners.into_iter().chain(sharers) {
                let o = self.advance(&mut run, res.clone(), js);
                changed |= self.settle(run, o);
            }
            js.forget(&[*task]);
        }
        let now = Instant::now();
        let (expired, alive): (Vec<Run>, Vec<Run>) = runs.into_iter().partition(|r| now >= r.deadline);
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
            let _ = self.tx.send(Msg::Completion(Box::new(self.reply(Phase::Generators))));
        }
    }

    // -----------------------------------------------------------------------------------------------------
    // Replies

    fn first_token_items(&mut self, aliases: &HashMap<String, String>, path: Option<String>, pathext: Option<String>) -> Vec<Sugg> {
        if let Some((at, items)) = &self.first_token_cache {
            if at.elapsed() < FIRST_TOKEN_TTL {
                return items.clone();
            }
        }
        let items = suggest::first_token_items(&self.index.completions, aliases, path.as_deref(), pathext.as_deref());
        self.first_token_cache = Some((Instant::now(), items.clone()));
        items
    }

    /// Rank the current request's suggestions (SPEC §6.7.5) into a reply.
    fn reply(&self, phase: Phase) -> Completion {
        let cur = self.cur.as_ref().expect("reply without a current request");
        let (pr, s) = (&cur.pr, &self.settings);
        let mut list = if pr.first_token {
            cur.first.clone()
        } else {
            let views: Vec<GenView> = self
                .slots
                .iter()
                .zip(&cur.queries)
                .map(|(sl, q)| GenView { key: sl.key, query: q.clone(), loading: sl.loading, items: &sl.result })
                .collect();
            suggest::assemble(pr, &views)
        };
        let mode = s.string("beta.history.mode").unwrap_or("off");
        if cur.req.history_mode || mode == "show" || mode == "history_only" {
            let hist = suggest::history_rows(&cur.prefix, self.history.entries());
            list = suggest::mix_history(list, hist, mode == "show" && !cur.req.history_mode);
        }
        let fo = FilterOpts {
            fuzzy: cur.fuzzy,
            auto_exec_hidden: s.bool("autocomplete.hideAutoExecuteSuggestion", false) || s.bool("autocomplete.onlyShowOnTab", false),
            exec_after_space: s.bool("autocomplete.immediatelyExecuteAfterSpace", false),
            run_dangerous: s.bool("autocomplete.immediatelyRunDangerousCommands", false),
            suggest_current_token: pr
                .current_arg
                .as_ref()
                .and_then(|a| a.suggest_current_token)
                .unwrap_or_else(|| s.bool("autocomplete.alwaysSuggestCurrentToken", false)),
        };
        let cmd = pr.tokens.get(pr.command_index).map_or("", String::as_str); // D21
        let use_recency = s.string("autocomplete.sortMethod") != Some("alphabetical");
        let ranked = {
            let rec = self.recency.lock().ok();
            let map = rec.as_ref().filter(|_| use_recency).and_then(|r| r.get(cmd));
            suggest::rank(list, &pr.search_term, map, &fo)
        };
        let ic = InsCtx {
            search_term: &pr.search_term,
            raw: &pr.raw_last_token,
            shell: cur.req.shell,
            fuzzy: cur.fuzzy,
            prefer_verbose: s.bool("autocomplete.preferVerboseSuggestions", false),
            insert_space: s.bool("autocomplete.insertSpaceAutomatically", true),
        };
        let items: Vec<Item> = ranked.into_iter().map(|(x, p)| suggest::to_item(x, p, &ic)).collect();
        let ghost = self.ghost(&cur.req, cur.cursor, Some(&items));
        Completion {
            req: cur.req.id,
            phase,
            error: false,
            buffer: cur.req.buffer.clone(),
            cursor: cur.cursor,
            tokens: pr.tokens.clone(),
            command_text: cur.cmd_text.clone(),
            command_index: pr.command_index,
            arg_key: cur.arg_key,
            current_arg: pr.current_arg.as_ref().map(|a| ArgInfo {
                name: a.name.clone().unwrap_or_default(),
                description: a.description.clone().unwrap_or_default(),
            }),
            search_term: pr.search_term.clone(),
            triggered_generators: if phase == Phase::Parse { cur.triggered.clone() } else { Vec::new() },
            items,
            loading: self.slots.iter().any(|s| s.loading),
            ghost,
            fuzzy: cur.fuzzy,
        }
    }

    /// Local inline ghost (SPEC §6.11). Also computed for parse-error replies (commands without a spec).
    fn ghost(&self, req: &Request, cursor: usize, items: Option<&[Item]>) -> Option<String> {
        if !self.settings.bool("inline.enabled", true) || req.buffer.is_empty() || cursor != req.buffer.len() {
            return None;
        }
        let history = || inline::history_ghost(self.history.entries(), &req.buffer);
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
        if name.contains('↪') {
            return;
        }
        if let Ok(mut r) = self.recency.lock() {
            r.entry(cmd).or_default().insert(name, now_ms());
        }
        let Some(path) = self.state_path.clone() else { return };
        if self.saving.swap(true, SeqCst) {
            return; // a save is already scheduled; it writes the latest map
        }
        let (rec, saving) = (self.recency.clone(), self.saving.clone());
        // ponytail: throttled to one write per second (first insert + 1 s), not a trailing debounce
        let spawned = std::thread::Builder::new().name("recency".into()).spawn(move || {
            std::thread::sleep(Duration::from_secs(1));
            saving.store(false, SeqCst);
            let snapshot = rec.lock().map(|r| r.clone()).unwrap_or_default();
            if let Err(e) = save_recency(&path, &snapshot) {
                logger::warn(&format!("engine: saving recency: {e:#}"));
            }
        });
        if spawned.is_err() {
            self.saving.store(false, SeqCst);
            self.save_recency_now();
        }
    }

    fn save_recency_now(&self) {
        let (Some(path), Ok(rec)) = (&self.state_path, self.recency.lock()) else { return };
        if let Err(e) = save_recency(path, &rec) {
            logger::warn(&format!("engine: saving recency: {e:#}"));
        }
    }

    fn llm(&mut self, req: u64, buffer: String, cwd: PathBuf, shell: Shell) {
        let tx = self.tx.clone();
        let Ok(Source::Profile(profile)) = inline::source(&self.settings) else {
            let _ = tx.send(Msg::Ghost { req, text: None });
            return;
        };
        if let Some(text) = self.ghost_cache.lock().ok().and_then(|c| c.lookup(&buffer)) {
            let _ = tx.send(Msg::Ghost { req, text: Some(text) });
            return;
        }
        let history = llm_history(&self.settings, self.history.entries());
        let env = self.env().clone();
        let cache = self.ghost_cache.clone();
        let spawned = std::thread::Builder::new().name("llm".into()).spawn(move || {
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

    #[test]
    fn recency_round_trip_keeps_other_keys() {
        let dir = std::env::temp_dir().join(format!("tabsmith-engine-{}-{}", std::process::id(), now_ms()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("state.json");
        std::fs::write(&path, r#"{"doctor.prompt-restart-terminal": true}"#).unwrap();
        assert!(load_recency(&path).is_empty());
        let rec: Recency = [("git".to_string(), [("checkout".to_string(), 1_700_000_000_000u64)].into())].into();
        save_recency(&path, &rec).unwrap();
        assert_eq!(load_recency(&path), rec);
        let store = Store::load(&path).unwrap();
        assert_eq!(store.map.get("doctor.prompt-restart-terminal"), Some(&json!(true)));
        assert!(load_recency(&dir.join("missing.json")).is_empty());
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn recency_save_merges_other_sessions() {
        // recency-cross-session-overwrite: two sessions loaded the same file; neither drops the other's entry
        let dir = crate::specs::testutil::TmpDir::new("recency");
        let path = dir.path().join("state.json");
        std::fs::write(&path, "{}").unwrap();
        let one = |cmd: &str, name: &str, ts: u64| -> Recency { [(cmd.to_string(), [(name.to_string(), ts)].into())].into() };
        save_recency(&path, &one("git", "checkout", 5)).unwrap();
        save_recency(&path, &one("git", "status", 2)).unwrap();
        save_recency(&path, &one("git", "checkout", 3)).unwrap(); // older than disk: newest wins
        let got = load_recency(&path);
        assert_eq!(got["git"], [("checkout".to_string(), 5u64), ("status".to_string(), 2)].into());
    }

    #[test]
    fn llm_history_honours_history_count() {
        // llm-history-count-ignored: only the last `inline.historyCount` usable entries leave the machine
        let e = |c: &str| HistoryEntry { command: c.into(), exit_code: Some(0), ..Default::default() };
        let entries: Vec<HistoryEntry> = (0..100).map(|i| e(&format!("echo {i}"))).collect();
        let with = |v: Value| Settings::from_map(json!({ "inline.historyCount": v }).as_object().unwrap().clone());
        let got: Vec<String> = llm_history(&with(json!(2)), &entries).into_iter().map(|h| h.command).collect();
        assert_eq!(got, ["echo 98", "echo 99"]);
        assert!(llm_history(&with(json!(0)), &entries).is_empty());
        assert_eq!(llm_history(&Settings::default(), &entries).len(), 49);
    }

    #[test]
    fn legacy_string_script_runs_through_sh_c() {
        // win-legacy-script-cmd: same `sh -c` form as `executeShellCommand(string)` on every platform
        assert_eq!(script_exec_input(json!("git log --format=\"%h %s\""), "/w"), Some(json!({"command": "sh", "args": ["-c", "git log --format=\"%h %s\""], "cwd": "/w"})));
        assert_eq!(script_exec_input(json!(["git", "branch"]), "/w"), Some(json!({"command": "git", "args": ["branch"], "cwd": "/w"})));
        assert_eq!(script_exec_input(json!({"command": "ls", "cwd": "/x"}), "/w"), Some(json!({"command": "ls", "cwd": "/x"})));
        assert_eq!(script_exec_input(json!(""), "/w"), None);
        assert_eq!(script_exec_input(json!([]), "/w"), None);
    }

    #[test]
    fn js_truthiness() {
        assert!(!truthy(&json!(null)) && !truthy(&json!(false)) && !truthy(&json!(0)) && !truthy(&json!("")));
        assert!(truthy(&json!(1)) && truthy(&json!("x")) && truthy(&json!([])) && truthy(&json!({})));
    }

    #[test]
    fn post_process_filtering() {
        // strings become arg rows carrying isDangerous; falsy entries and nameless objects are dropped
        let items = post_process_items(&json!(["a", "", null, 0, {"description": "no name"}]), true);
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].names, ["a"]);
        assert_eq!(items[0].insert_value.as_deref(), Some("a"));
        assert!(items[0].is_dangerous);
        assert!(post_process_items(&json!(null), false).is_empty());
    }

    #[test]
    fn session_update_refreshes_first_token_items() {
        // first-token-cache-ignores-session-update: `alias zzal=…` at this prompt shows `zzal` at the next one
        let dir = crate::specs::testutil::TmpDir::new("engine-session");
        let (tx, _rx) = std::sync::mpsc::channel();
        let mut e = Engine::with(tx, Settings::default(), SpecIndex::default(), History::load_from(dir.path().join("h.jsonl")), None);
        let (jtx, jrx) = std::sync::mpsc::channel();
        let mut js = Js::new(jrx, jtx, Settings::default()).unwrap();
        assert!(e.first_token_items(&HashMap::new(), None, None).is_empty());
        assert!(e.handle(EngineMsg::Session(SessionCtx::default()), &mut None, &mut js));
        let aliases = HashMap::from([("zzal".to_string(), "git status".to_string())]);
        assert_eq!(e.first_token_items(&aliases, None, None).iter().map(|s| s.names[0].as_str()).collect::<Vec<_>>(), ["zzal"]);
    }

    #[test]
    fn generator_keys_differ_per_index() {
        let a = Rc::new(Arg::default());
        assert_ne!(gen_key(&a, 0), gen_key(&a, 1));
        assert_eq!(gen_key(&a, 0), gen_key(&a, 0));
    }
}
