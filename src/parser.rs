//! Spec loading (resolution, diff-versioned merge) and the argv state machine (SPEC §6.3–6.4).
//! OWNER: parser agent. `ParseResult` is consumed by the suggest agent — extend, don't rename.
use crate::js::{ExecInput, GenCtx, Js, JsArg, JsError, TaskId};
use crate::settings::domain::services::settings_catalog::{
    AUTOCOMPLETE_DEVELOPER_MODE, AUTOCOMPLETE_DISABLE_FOR_COMMANDS,
};
use crate::settings::domain::value_objects::settings::Settings;
use crate::shell_parser::CommandInfo;
use crate::specs::domain::entities::subcommand::Subcommand;
use crate::specs::domain::value_objects::arg::Arg;
use crate::specs::domain::value_objects::parser_directives::ParserDirectives;
use crate::specs::domain::value_objects::repeatability::Repeatability;
use crate::specs::domain::value_objects::semantic_version::semver_clean;
use crate::specs::domain::value_objects::spec_function_ref::SpecFunctionRef;
use crate::specs::domain::value_objects::spec_load_directive::SpecLoadDirective;
use crate::specs::domain::services::spec_locator::spec_location;
use crate::specs::domain::services::spec_version_selector::versions_to_apply;
use crate::specs::domain::value_objects::spec_location::SpecLocation;
use crate::specs::domain::value_objects::spec_option::SpecOption;
use crate::specs::domain::value_objects::text_or_function::TextOrFunction;
use crate::specs::infrastructure::adapters::fig_spec_mapper::{self, normalise};
use crate::specs::domain::value_objects::spec_index::SpecIndex;
use crate::specs::infrastructure::adapters::embedded_spec_bundle::EmbeddedSpecBundle;
use crate::shared::domain::value_objects::shell::Shell;
use crate::shared::infrastructure::data_directory_from_env::{data_directory_from_env, user_home_from_env};
use crate::shared::infrastructure::logger;
use serde_json::{Map, Value};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::time::{Duration, Instant};

#[cfg(test)]
mod tests;

/// Deadline for one spec load / spec callback (SPEC §6.3).
const TIMEOUT: Duration = Duration::from_secs(5);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AnnKind {
    Subcommand,
    Option,
    OptionArg,
    SubcommandArg,
    /// Token that could not be consumed (only possible for the final token).
    None,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Annotation {
    /// For a composite token: the kind of its last sub-token.
    pub kind: AnnKind,
    pub text: String,
    /// Sub-tokens of a composite token (`-abc`, `--opt=val`), else empty.
    pub parts: Vec<(AnnKind, String)>,
}

/// Outcome of parsing the current command (SPEC §6.4 `result(st)`).
#[derive(Clone, Debug)]
pub struct ParseResult {
    /// The subcommand level the cursor is in.
    pub completion: Rc<Subcommand>,
    /// Root..=current chain of subcommands (for the `help` template).
    pub path: Vec<Rc<Subcommand>>,
    pub current_arg: Option<Rc<Arg>>,
    pub search_term: String,
    pub suggest_subcommands: bool,
    pub suggest_options: bool,
    pub suggest_args: bool,
    /// Options already passed at this level.
    pub passed_options: Vec<Rc<SpecOption>>,
    /// Persistent options inherited at this level, in lookup order (parent first: parent wins on name clash).
    pub persistent: Vec<Rc<SpecOption>>,
    pub annotations: Vec<Annotation>,
    /// Index of the token that names the spec in use (`sudo git ch` → 1).
    pub command_index: usize,
    /// Token texts after alias / `parserDirectives.alias` substitution.
    pub tokens: Vec<String>,
    /// Raw text of the last token as typed; for an option value inside the token (`--opt=val`, `-oval`) only
    /// the value part.
    pub raw_last_token: String,
    pub directives: ParserDirectives,
    /// The composite option chain being typed (`-la`), if the last annotation is one ending in an option.
    pub option_chain: Option<String>,
    /// Single-token command: suggest first tokens (SPEC §6.9) instead of spec items.
    pub first_token: bool,
    /// Name of the root spec (`git`), for recency keys and caches.
    pub spec_name: String,
}

#[derive(Debug)]
pub enum ParseError {
    Aborted,
    /// No suggestions (unknown spec, unconsumable token, load failure). Logged by the caller.
    NoSuggestions(String),
}

impl From<JsError> for ParseError {
    fn from(e: JsError) -> Self {
        match e {
            JsError::Aborted => ParseError::Aborted,
            other => ParseError::NoSuggestions(other.to_string()),
        }
    }
}

fn no(msg: impl Into<String>) -> ParseError {
    ParseError::NoSuggestions(msg.into())
}

/// Everything the parser needs from the session.
pub struct ParseCtx<'a> {
    pub cwd: &'a Path,
    pub settings: &'a Settings,
    pub env: &'a HashMap<String, String>,
    pub shell: Shell,
    pub index: &'a SpecIndex,
}

/// Spec loader + parse caches (engine thread).
pub struct Parser {
    loader: Loader,
    caches: ParseCaches,
}

impl Parser {
    pub fn new() -> Parser {
        Parser {
            loader: Loader::default(),
            caches: ParseCaches::default(),
        }
    }

    /// Drop caches (dev mode toggles, settings reload). The CLI-version memo survives, and so
    /// do spec identities (`by_json`, the first-token spec): unchanged specs are not a "new argument".
    pub fn clear_caches(&mut self) {
        self.loader.specs.clear();
        self.caches = ParseCaches {
            first: self.caches.first.clone(),
            ..ParseCaches::default()
        };
    }

    /// Parse the command under the cursor against its spec (SPEC §6.4).
    pub fn parse(
        &mut self,
        cmd: &CommandInfo,
        ctx: &ParseCtx,
        js: &mut Js,
    ) -> Result<ParseResult, ParseError> {
        let dev = dev_mode(ctx.settings);
        let mut host = JsHost {
            loader: &mut self.loader,
            js,
            ctx,
            generator: gen_ctx(ctx),
        };
        self.caches.parse(cmd, ctx.cwd, dev, &mut host)
    }

    /// Load (and cache) the spec at `loc`, applying diff versions (SPEC §6.3). `Ok(None)` when it does not exist
    /// or is disabled.
    pub fn load_spec(
        &mut self,
        loc: &SpecLocation,
        ctx: &ParseCtx,
        js: &mut Js,
    ) -> Result<Option<Rc<Subcommand>>, ParseError> {
        self.loader.load(loc, ctx, js, &gen_ctx(ctx))
    }
}

// ---------------------------------------------------------------------------------------------------------------
// Loading (SPEC §6.3)

fn dev_mode(settings: &Settings) -> bool {
    settings.bool(AUTOCOMPLETE_DEVELOPER_MODE)
}

fn gen_ctx(ctx: &ParseCtx) -> GenCtx {
    GenCtx {
        cwd: ctx.cwd.to_string_lossy().into_owned(),
        env: ctx.env.clone(),
        shell: ctx.shell.name().into(),
        ..Default::default()
    }
}

#[derive(Default)]
struct Loader {
    /// Spec cache by `SpecLocation::cache_key` (None = missing, cached too).
    specs: HashMap<String, Option<Rc<Subcommand>>>,
    /// `cliVersion-<name>` memo (successful detections only).
    versions: HashMap<String, String>,
    /// Version detections still running after an aborted parse, reused by the next one (no respawn per keystroke).
    inflight: HashMap<String, TaskId>,
    /// Last normalised spec per cache key with the JSON it came from: an unchanged spec keeps its `Rc` through
    /// developer-mode reloads and `clear_caches` (the engine keys arg/generator identity on the address).
    by_json: HashMap<String, (Value, Rc<Subcommand>)>,
}

impl Loader {
    fn load(
        &mut self,
        loc: &SpecLocation,
        ctx: &ParseCtx,
        js: &mut Js,
        generator: &GenCtx,
    ) -> Result<Option<Rc<Subcommand>>, ParseError> {
        let (SpecLocation::Global { name } | SpecLocation::Local { name, .. }) = loc;
        if ctx
            .settings
            .str_list(AUTOCOMPLETE_DISABLE_FOR_COMMANDS)
            .contains(name)
        {
            return Ok(None);
        }
        let dev = dev_mode(ctx.settings);
        let key = loc.cache_key();
        if let Some(spec) = self.specs.get(&key).filter(|_| !dev) {
            return Ok(spec.clone());
        }
        let Some(path) = spec_file(loc, ctx) else {
            self.specs.insert(key, None);
            return Ok(None);
        };
        let exports = js.load_module(&path)?;
        let json = match exports.get("default") {
            Some(v) if fig_spec_mapper::fn_id(v).is_some() => {
                self.versioned(&exports, name, &path, dev, js, generator)?
            }
            Some(v) if v.is_object() => Some(v.clone()),
            _ => None,
        };
        let spec = match json {
            Some(j) => match self.by_json.get(&key).filter(|(old, _)| *old == j) {
                Some((_, rc)) => Some(rc.clone()),
                None => {
                    let rc = Rc::new(normalise(&j).map_err(|e| no(format!("{name}: {e}")))?);
                    self.by_json.insert(key.clone(), (j, rc.clone()));
                    Some(rc)
                }
            },
            None => None,
        };
        self.specs.insert(key, spec.clone());
        Ok(spec)
    }

    /// `default` is `async (version) => Subcommand | {versionedSpecPath, version}` (SPEC §6.3).
    fn versioned(
        &mut self,
        exports: &Map<String, Value>,
        name: &str,
        path: &Path,
        dev: bool,
        js: &mut Js,
        generator: &GenCtx,
    ) -> Result<Option<Value>, ParseError> {
        let memo = format!("cliVersion-{name}");
        let version = match self.versions.get(&memo).filter(|_| !dev) {
            Some(v) => Some(v.clone()),
            None => cli_version(exports, name, js, generator, &mut self.inflight)?,
        };
        if let Some(v) = &version {
            self.versions.insert(memo, v.clone());
        }
        let default = exports
            .get("default")
            .and_then(fig_spec_mapper::fn_id)
            .ok_or_else(|| no("default is not a function"))?;
        let arg = version.map_or(Value::Null, Value::String);
        let r = js.call(default, vec![JsArg::Json(arg)], Some(generator), TIMEOUT)?;
        let Some(vsp) = r.get("versionedSpecPath").and_then(Value::as_str) else {
            return Ok(Some(r).filter(Value::is_object));
        };
        // ponytail: the file sits next to `<name>/index.js`; true for all 6 bundled diff-versioned specs.
        let base = vsp.rsplit('/').next().unwrap_or(vsp);
        let file = path.with_file_name(format!("{base}.js"));
        let ex = js.load_module(&file)?;
        match (
            ex.get("default"),
            ex.get("versions").and_then(Value::as_object),
        ) {
            (Some(spec), Some(diffs)) => Ok(Some(apply_versions(
                spec.clone(),
                diffs,
                r.get("version").and_then(Value::as_str),
            ))),
            _ => Err(no(format!(
                "{}: invalid versioned spec file",
                file.display()
            ))),
        }
    }
}

/// The file a location loads from: [`EmbeddedSpecBundle::resolve`] for globals, `<dir>/.fig/autocomplete/build/<name>.js` (else
/// `<name>/index.js`) for locals.
fn spec_file(loc: &SpecLocation, ctx: &ParseCtx) -> Option<PathBuf> {
    match loc {
        SpecLocation::Global { name } => {
            EmbeddedSpecBundle::new(data_directory_from_env()).resolve(name, ctx.settings, ctx.index)
        }
        SpecLocation::Local { dir, name } => {
            let dir = match dir.strip_prefix("~") {
                Ok(rest) => user_home_from_env().ok()?.join(rest),
                Err(_) => dir.clone(),
            };
            let build = dir.join(".fig").join("autocomplete").join("build");
            [
                build.join(format!("{name}.js")),
                build.join(name).join("index.js"),
            ]
            .into_iter()
            .find(|p| p.is_file())
        }
    }
}

/// `getVersionCommand(exec)` if exported, else `semver_clean(<name> --version)`; failures → None. An aborted
/// wait leaves the detection running in `inflight`, and the next parse waits on that same task.
fn cli_version(
    exports: &Map<String, Value>,
    name: &str,
    js: &mut Js,
    generator: &GenCtx,
    inflight: &mut HashMap<String, TaskId>,
) -> Result<Option<String>, ParseError> {
    if name == "@usermn/sdc" {
        return Ok(None); // its version command runs `npx`, which may download a package
    }
    let f = exports
        .get("getVersionCommand")
        .and_then(fig_spec_mapper::fn_id);
    let id = *inflight.entry(name.to_string()).or_insert_with(|| match f {
        Some(f) => js.start_call(f, vec![JsArg::Exec], Some(generator)),
        None => js.start_exec(
            ExecInput {
                command: name.into(),
                args: vec!["--version".into()],
                ..Default::default()
            },
            generator,
        ),
    });
    let r = js.wait(&[id], Instant::now() + TIMEOUT);
    if matches!(r, Err(JsError::Aborted)) {
        return Err(ParseError::Aborted);
    }
    inflight.remove(name);
    js.forget(&[id]);
    let Ok(Some(Ok(v))) = r.map(|mut r| r.pop().flatten()) else {
        return Ok(None);
    };
    Ok(match f {
        Some(_) => v.as_str().map(String::from),
        None => v
            .get("stdout")
            .and_then(Value::as_str)
            .and_then(semver_clean),
    })
}

// ---------------------------------------------------------------------------------------------------------------
// Diff-versioned specs (SPEC §6.3)

/// Apply every diff whose version is `<= target` (all when `target` is None), in semver order.
fn apply_versions(base: Value, diffs: &Map<String, Value>, target: Option<&str>) -> Value {
    versions_to_apply(diffs.keys().map(String::as_str), target)
        .into_iter()
        .fold(base, |base, version| merge_subcommand(&base, &diffs[version]))
}

fn merge_subcommand(base: &Value, diff: &Value) -> Value {
    if diff.is_null() {
        return base.clone();
    }
    let mut out = shallow_merge(base, diff);
    if truthy(diff.get("subcommands")) {
        out["subcommands"] = merge_named(
            base.get("subcommands"),
            diff.get("subcommands"),
            merge_subcommand,
        );
    }
    if truthy(diff.get("options")) {
        out["options"] = merge_named(base.get("options"), diff.get("options"), merge_option);
    }
    if truthy(diff.get("args")) {
        out["args"] = merge_args(base.get("args"), diff.get("args"));
    }
    out
}

fn merge_option(base: &Value, diff: &Value) -> Value {
    let mut out = shallow_merge(base, diff);
    if truthy(diff.get("args")) {
        out["args"] = merge_args(base.get("args"), diff.get("args"));
    }
    out
}

/// Diff items matched to base items by name overlap; `remove` drops the match. Order: diff items, then untouched base.
fn merge_named(
    base: Option<&Value>,
    diff: Option<&Value>,
    merge: fn(&Value, &Value) -> Value,
) -> Value {
    let (bl, dl) = (make_array(base), make_array(diff));
    let mut used = vec![false; bl.len()];
    let mut out = Vec::new();
    for d in &dl {
        let dn = names(d);
        match (0..bl.len()).find(|&j| !used[j] && names(&bl[j]).iter().any(|n| dn.contains(n))) {
            None => out.push(d.clone()),
            Some(j) => {
                used[j] = true;
                if d.get("remove").is_none() {
                    out.push(merge(&bl[j], d));
                }
            }
        }
    }
    out.extend(
        bl.iter()
            .zip(used)
            .filter(|(_, u)| !u)
            .map(|(b, _)| b.clone()),
    );
    Value::Array(out)
}

/// Positional merge; the index is taken after filtering out removed args (quirk kept).
fn merge_args(base: Option<&Value>, diff: Option<&Value>) -> Value {
    let bl = make_array(base);
    let dl = make_array(diff)
        .into_iter()
        .filter(|d| !truthy(d.get("remove")));
    Value::Array(
        dl.enumerate()
            .map(|(i, d)| shallow_merge(bl.get(i).unwrap_or(&Value::Null), &d))
            .collect(),
    )
}

/// `{...base, ...diff}`.
fn shallow_merge(base: &Value, diff: &Value) -> Value {
    let mut out = base.as_object().cloned().unwrap_or_default();
    for (k, v) in diff.as_object().into_iter().flatten() {
        out.insert(k.clone(), v.clone());
    }
    Value::Object(out)
}

fn make_array(v: Option<&Value>) -> Vec<Value> {
    match v {
        Some(Value::Array(a)) => a.clone(),
        Some(x) if truthy(Some(x)) => vec![x.clone()],
        _ => Vec::new(),
    }
}

fn names(v: &Value) -> Vec<Value> {
    make_array(v.get("name"))
}

/// JS truthiness.
fn truthy(v: Option<&Value>) -> bool {
    match v {
        None | Some(Value::Null) => false,
        Some(Value::Bool(b)) => *b,
        Some(Value::Number(n)) => n.as_f64().is_some_and(|f| f != 0.0),
        Some(Value::String(s)) => !s.is_empty(),
        Some(_) => true,
    }
}

// ---------------------------------------------------------------------------------------------------------------
// Argv state machine (SPEC §6.4)

/// What the state machine needs from outside (spec files, JS callbacks, re-tokenizing); mocked in tests.
trait Host {
    fn load(&mut self, loc: &SpecLocation) -> Result<Option<Rc<Subcommand>>, ParseError>;
    /// Call a spec function; `JsArg::Exec` is bound to the session's exec.
    fn call(&mut self, f: SpecFunctionRef, args: Vec<JsArg>) -> Result<Value, ParseError>;
    fn substitute(&mut self, cmd: &CommandInfo, index: usize, value: &str) -> Option<CommandInfo>;
}

struct JsHost<'a, 'c> {
    loader: &'a mut Loader,
    js: &'a mut Js,
    ctx: &'a ParseCtx<'c>,
    generator: GenCtx,
}

impl Host for JsHost<'_, '_> {
    fn load(&mut self, loc: &SpecLocation) -> Result<Option<Rc<Subcommand>>, ParseError> {
        self.loader.load(loc, self.ctx, self.js, &self.generator)
    }

    fn call(&mut self, f: SpecFunctionRef, args: Vec<JsArg>) -> Result<Value, ParseError> {
        Ok(self.js.call(f, args, Some(&self.generator), TIMEOUT)?)
    }

    fn substitute(&mut self, cmd: &CommandInfo, index: usize, value: &str) -> Option<CommandInfo> {
        crate::shell_parser::substitute_token(cmd, index, value, self.ctx.shell)
    }
}

#[derive(Clone, Debug, Default)]
struct ArgState {
    /// Empty = no arg slot (`args: null`).
    args: Vec<Rc<Arg>>,
    index: usize,
    variadic_count: usize,
}

impl ArgState {
    fn new(args: &[Rc<Arg>]) -> ArgState {
        ArgState {
            args: args.to_vec(),
            ..Default::default()
        }
    }

    fn cur(&self) -> Option<&Rc<Arg>> {
        self.args.get(self.index)
    }

    fn advance(&mut self) {
        match self.cur() {
            Some(a) if a.is_variadic() => self.variadic_count += 1,
            Some(_) if self.index + 1 < self.args.len() => {
                *self = ArgState {
                    args: std::mem::take(&mut self.args),
                    index: self.index + 1,
                    variadic_count: 0,
                }
            }
            _ => *self = ArgState::default(),
        }
    }
}

fn mandatory_or_var(a: Option<&Rc<Arg>>) -> bool {
    a.is_some_and(|a| a.is_variadic() || !a.is_optional())
}

#[derive(Clone, Debug)]
struct State {
    completion: Rc<Subcommand>,
    path: Vec<Rc<Subcommand>>,
    /// Effective parserDirectives (inherited downwards without mutating the spec).
    directives: ParserDirectives,
    /// Inherited persistent options, lookup order (parent first).
    persistent: Vec<Rc<SpecOption>>,
    opt_args: ArgState,
    sub_args: ArgState,
    annotations: Vec<Annotation>,
    passed: Vec<Rc<SpecOption>>,
    command_index: usize,
    entered_sub_args: bool,
    end_of_options: bool,
    /// Token texts from this parse's start, after `parserDirectives.alias` substitution (last one is stale when
    /// cached). Callers prepend their own prefix, so a cached nested state never carries another command's prefix.
    tokens: Vec<String>,
    spec_name: String,
}

fn ann(kind: AnnKind, text: &str) -> Annotation {
    Annotation {
        kind,
        text: text.into(),
        parts: Vec::new(),
    }
}

impl State {
    fn initial(spec: Rc<Subcommand>, text: &str, spec_name: &str, tokens: Vec<String>) -> State {
        State {
            path: vec![spec.clone()],
            directives: spec.directives().clone(),
            persistent: spec.persistent().clone(),
            opt_args: ArgState::default(),
            sub_args: ArgState::new(spec.args()),
            annotations: vec![ann(AnnKind::Subcommand, text)],
            passed: Vec::new(),
            command_index: 0,
            entered_sub_args: false,
            end_of_options: false,
            tokens,
            spec_name: spec_name.into(),
            completion: spec,
        }
    }

    /// Replace the current subcommand in place (inline loadSpec, generateSpec merge).
    fn set_completion(&mut self, c: Rc<Subcommand>) {
        if let Some(last) = self.path.last_mut() {
            *last = c.clone();
        }
        self.completion = c;
    }

    fn prefer_opt_arg(&self) -> bool {
        mandatory_or_var(self.opt_args.cur()) || self.sub_args.cur().is_none()
    }

    fn arg_state(&self) -> &ArgState {
        if self.prefer_opt_arg() {
            &self.opt_args
        } else {
            &self.sub_args
        }
    }

    fn can_subcommands(&self) -> bool {
        !mandatory_or_var(self.opt_args.cur()) && !self.entered_sub_args
    }

    fn can_options(&self) -> bool {
        if (self.entered_sub_args && self.directives.options_must_precede_args())
            || self.end_of_options
        {
            return false;
        }
        if let Some(o) = self.opt_args.cur().filter(|a| mandatory_or_var(Some(a))) {
            return o.is_variadic()
                && self.opt_args.variadic_count > 0
                && o.options_can_break_variadic() != Some(false);
        }
        !self.sub_args.cur().is_some_and(|s| {
            s.is_variadic()
                && self.sub_args.variadic_count > 0
                && s.options_can_break_variadic() == Some(false)
        })
    }

    /// `options[flag] || persistentOptions[flag]`.
    fn find_option(&self, flag: &str) -> Option<Rc<SpecOption>> {
        match self.completion.opt_index().get(flag) {
            Some(&(false, i)) => self.completion.options().get(i).cloned(),
            _ => self
                .persistent
                .iter()
                .find(|o| o.names().iter().any(|n| n == flag))
                .cloned(),
        }
    }
}

/// First successful interpretation of `token`; None = it fits nowhere.
fn update(st: &State, token: &str, fin: bool) -> Option<State> {
    (st.can_subcommands()
        .then(|| subcommand(st, token, fin))
        .flatten())
    .or_else(|| {
        st.can_options()
            .then(|| option_or_chain(st, token, fin))
            .flatten()
    })
    .or_else(|| {
        st.prefer_opt_arg()
            .then(|| option_arg(st, token, fin))
            .flatten()
    })
    .or_else(|| subcommand_arg(st, token, fin))
}

fn subcommand(st: &State, token: &str, fin: bool) -> Option<State> {
    if st.entered_sub_args {
        return None;
    }
    let child = st
        .completion
        .subcommands()
        .get(*st.completion.sub_index().get(token)?)?
        .clone();
    let mut s = st.clone();
    s.annotations.push(ann(AnnKind::Subcommand, token));
    if fin {
        return Some(s); // siblings stay suggestable
    }
    if child.has_directives() {
        s.directives = child.directives().clone();
    }
    s.persistent.extend(child.persistent().iter().cloned());
    s.passed.clear();
    s.opt_args = ArgState::default();
    s.sub_args = ArgState::new(child.args());
    s.path.push(child.clone());
    s.completion = child;
    Some(s)
}

fn option(st: &State, flag: &str, fin: bool) -> Option<State> {
    let o = st.find_option(flag)?;
    if let Repeatability::Times(n) = o.is_repeatable() {
        let count = st
            .passed
            .iter()
            .filter(|p| p.names().iter().any(|x| o.names().contains(x)))
            .count();
        if count >= n as usize {
            return None;
        }
    }
    let mut s = st.clone();
    s.annotations.push(ann(AnnKind::Option, flag));
    if !fin {
        s.opt_args = ArgState::new(o.args());
        s.passed.push(o);
    }
    Some(s)
}

fn option_arg(st: &State, token: &str, fin: bool) -> Option<State> {
    st.opt_args.cur()?;
    let mut s = st.clone();
    s.annotations.push(ann(AnnKind::OptionArg, token));
    if !fin {
        s.opt_args.advance();
    }
    Some(s)
}

fn subcommand_arg(st: &State, token: &str, fin: bool) -> Option<State> {
    st.sub_args.cur()?;
    let mut s = st.clone();
    s.annotations.push(ann(AnnKind::SubcommandArg, token));
    if !fin {
        s.sub_args.advance();
        s.entered_sub_args = true;
    }
    Some(s)
}

fn option_or_chain(st: &State, token: &str, fin: bool) -> Option<State> {
    if fin && (token == "-" || token == "--") {
        return None;
    }
    if token == "--" {
        let mut s = st.clone();
        s.end_of_options = true;
        s.annotations.push(ann(AnnKind::Option, token));
        s.opt_args = ArgState::default();
        return Some(s);
    }
    let composite = |mut s: State, parts: Vec<(AnnKind, String)>| {
        s.annotations.truncate(st.annotations.len());
        let kind = parts.last().map_or(AnnKind::Option, |p| p.0);
        s.annotations.push(Annotation {
            kind,
            text: token.into(),
            parts,
        });
        s
    };
    if st.directives.flags_posix_noncompliant() || token.starts_with("--") || !token.starts_with('-')
    {
        // Separators are matched char by char (a Set of strings vs. the token's chars).
        let seps = st.directives.option_arg_separators();
        if let Some((pos, c)) = token.char_indices().find(|&(_, c)| {
            seps.iter()
                .any(|s| s.len() == c.len_utf8() && s.starts_with(c))
        }) {
            let (flag, val) = (&token[..pos], &token[pos + c.len_utf8()..]);
            let s = option(st, flag, false)?;
            if s.opt_args.args.len() > 1 {
                return None;
            }
            let s = option_arg(&s, val, fin)?; // no arg slot → the whole token falls back to a positional
            return Some(composite(
                s,
                vec![
                    (AnnKind::Option, format!("{flag}{c}")),
                    (AnnKind::OptionArg, val.into()),
                ],
            ));
        }
        let mut s = option(st, token, fin)?;
        let o = st.find_option(token)?;
        if o.requires_equals() || o.requires_separator().is_some() {
            s.opt_args = ArgState::default(); // the next word is not its value
        }
        return Some(s);
    }
    // POSIX chain `-abc`.
    let chars: Vec<char> = token.chars().collect();
    let (mut cur, mut passed) = (st.clone(), st.passed.clone());
    let (mut opt_val, mut parts) = (String::new(), Vec::new());
    for i in 1..chars.len() {
        let flag = format!("-{}", chars[i]);
        passed = cur.passed.clone();
        match option(&cur, &flag, false) {
            Some(s) => cur = s,
            None if i > 1 => {
                opt_val = chars[i..].iter().collect();
                break;
            }
            None => return None,
        }
        parts.push((
            AnnKind::Option,
            if i == 1 { flag } else { chars[i].to_string() },
        ));
        if mandatory_or_var(cur.opt_args.cur()) {
            opt_val = chars[i + 1..].iter().collect();
            break;
        }
    }
    if !opt_val.is_empty() {
        if cur.opt_args.args.len() > 1 {
            return None;
        }
        cur = option_arg(&cur, &opt_val, fin)?;
        passed = cur.passed.clone();
        parts.push((AnnKind::OptionArg, opt_val));
    }
    if fin {
        cur.passed = passed; // the letter being typed stays suggestable
    }
    Some(composite(cur, parts))
}

fn result(st: State, last_text: &str, raw_last_token: &str, first_token: bool) -> ParseResult {
    let last = st.annotations.last();
    let mut search_term = last.map(|a| a.text.clone()).unwrap_or_default();
    let mut only_args = st.end_of_options;
    let mut arg_st = st.arg_state();
    let mut option_chain = None;
    let mut raw = raw_last_token.to_string();
    if let Some(a) = last.filter(|a| !a.parts.is_empty()) {
        arg_st = &st.opt_args;
        match a.parts.last() {
            Some((AnnKind::OptionArg, t)) => {
                search_term = t.clone();
                only_args = true;
                // `--color=al` / `-oal`: only the value is replaced, never the flag in front of it
                let flag: String = a.parts[..a.parts.len() - 1]
                    .iter()
                    .map(|p| p.1.as_str())
                    .collect();
                raw = raw_last_token
                    .strip_prefix(flag.as_str())
                    .unwrap_or(t)
                    .to_string();
            }
            _ => option_chain = Some(a.text.clone()),
        }
    }
    let mut tokens = st.tokens.clone();
    if let Some(t) = tokens.last_mut() {
        *t = last_text.into();
    }
    ParseResult {
        current_arg: arg_st.cur().cloned(),
        search_term,
        suggest_subcommands: !only_args && st.can_subcommands(),
        suggest_options: !only_args && st.can_options(),
        suggest_args: true,
        completion: st.completion.clone(),
        path: st.path.clone(),
        passed_options: st.passed.clone(),
        persistent: st.persistent.clone(),
        command_index: st.command_index,
        tokens,
        raw_last_token: raw,
        directives: st.directives.clone(),
        option_chain,
        first_token,
        spec_name: st.spec_name.clone(),
        annotations: st.annotations,
    }
}

/// Prefix-parse cache and generateSpec cache (SPEC §6.4); pure over a [`Host`].
#[derive(Default)]
struct ParseCaches {
    prefix: HashMap<String, State>,
    generated: HashMap<String, Rc<Subcommand>>,
    /// The empty first-token spec, one Rc for every parse (the engine keys arg identity on its address).
    first: Rc<Subcommand>,
}

/// ponytail: caches are cleared wholesale past this size (entries are never evicted otherwise).
const MAX_CACHED: usize = 1000;
/// Guards loadSpec cycles in user specs (they would recurse forever).
const MAX_DEPTH: usize = 64;

impl ParseCaches {
    fn parse(
        &mut self,
        cmd: &CommandInfo,
        cwd: &Path,
        dev: bool,
        host: &mut dyn Host,
    ) -> Result<ParseResult, ParseError> {
        let last = cmd.tokens.last().ok_or_else(|| no("no tokens"))?;
        let texts: Vec<String> = cmd.tokens.iter().map(|t| t.text.clone()).collect();
        if cmd.tokens.len() == 1 {
            if !last.text.contains('/') {
                let st = State::initial(self.first.clone(), &last.text, "", texts);
                return Ok(result(st, &last.text, &last.raw, true));
            }
            let name = if last.text == "bin/console" {
                "php/bin-console"
            } else {
                "dotslash"
            };
            let spec = host
                .load(&SpecLocation::Global { name: name.into() })?
                .ok_or_else(|| no(format!("no {name} spec")))?;
            return Ok(result(
                State::initial(spec, &last.text, name, texts),
                &last.text,
                &last.raw,
                false,
            ));
        }
        let loc = spec_location(&texts[0], cwd, None);
        let st = self.parse_prefix(cmd, vec![loc], 0, cwd, dev, host, 0)?;
        let st = update(&st, &last.text, true).unwrap_or_else(|| {
            let mut st = st;
            st.annotations.push(ann(AnnKind::None, &last.text));
            st
        });
        Ok(result(st, &last.text, &last.raw, false))
    }

    /// Parse `cmd.tokens[start..len-1]` from the first location that loads (SPEC §6.4 `parse_prefix`).
    #[allow(clippy::too_many_arguments)]
    fn parse_prefix(
        &mut self,
        cmd: &CommandInfo,
        locations: Vec<SpecLocation>,
        start: usize,
        cwd: &Path,
        dev: bool,
        host: &mut dyn Host,
        depth: usize,
    ) -> Result<State, ParseError> {
        if depth > MAX_DEPTH {
            return Err(no("loadSpec recursion too deep"));
        }
        let texts: Vec<String> = cmd.tokens[start..].iter().map(|t| t.text.clone()).collect();
        let key = |loc: &SpecLocation| {
            format!(
                "{},{},{}",
                texts[..texts.len() - 1].join(" "),
                loc.cache_key(),
                cwd.display()
            )
        };
        if !dev {
            if let Some(st) = locations.iter().find_map(|l| self.prefix.get(&key(l))) {
                return Ok(st.clone());
            }
        }
        let mut found = None;
        for loc in &locations {
            if let Some(spec) = host.load(loc)? {
                found = Some((spec, loc));
                break;
            }
        }
        let (spec, loc) =
            found.ok_or_else(|| no(format!("failed loading spec for {}", texts[0])))?;
        let (SpecLocation::Global { name } | SpecLocation::Local { name, .. }) = loc;
        let cache_key = key(loc);
        let mut st = State::initial(spec, &texts[0], name, texts.clone());
        let mut cur = cmd.clone();
        let root_ls = st.completion.load_spec().clone();
        if let Some(nested) = self.apply_load_spec(
            &mut st, root_ls, 0, None, &cur, start, cwd, dev, host, depth,
        )? {
            return Ok(nested);
        }
        let mut substituted: HashSet<String> = HashSet::new();
        let mut i = 1;
        while i < cur.tokens.len() - start {
            if st.completion.generate_spec().is_some() {
                let tokens: Vec<String> =
                    cur.tokens[start..].iter().map(|t| t.text.clone()).collect();
                st = self.generate_spec(st, &tokens, dev, host)?;
            }
            if i == cur.tokens.len() - start - 1 {
                break; // the final token never loads specs
            }
            let token = cur.tokens[start + i].text.clone();
            let last_arg = st.arg_state().cur().cloned();
            let last_kind = if st.prefer_opt_arg() {
                AnnKind::OptionArg
            } else {
                AnnKind::SubcommandArg
            };
            let last_state = st.clone();
            st = update(&st, &token, false)
                .ok_or_else(|| no(format!("cannot consume {token:?}")))?;
            let last_type = st.annotations.last().map_or(AnnKind::None, |a| a.kind);
            let mut alias_err = None;
            if let Some(alias) = last_arg
                .as_ref()
                .and_then(|a| a.alias_directive().clone())
                .filter(|_| last_type == last_kind && !substituted.contains(&token))
            {
                match self.substitute_alias(&cur, start + i, &token, &alias, host) {
                    Ok(next) => {
                        substituted.insert(token);
                        cur = next;
                        st = last_state;
                        continue; // re-parse this position with the expanded words
                    }
                    Err(e) if substituted.is_empty() || matches!(e, ParseError::Aborted) => {
                        return Err(e)
                    }
                    Err(e) => alias_err = Some(e),
                }
            }
            let mut ls = if last_type == AnnKind::Subcommand {
                st.completion.load_spec().clone()
            } else {
                None
            };
            if let Some(a) = last_arg.filter(|_| last_type == last_kind) {
                if a.load_spec().is_some() {
                    ls = a.load_spec().clone();
                } else if a.is_command() || a.is_script() {
                    ls = Some(SpecLoadDirective::Locations(vec![spec_location(
                        &token,
                        cwd,
                        Some(a.is_script()),
                    )]));
                } else if let Some(prefix) = a.is_module() {
                    ls = Some(SpecLoadDirective::Locations(vec![SpecLocation::Global {
                        name: format!("{prefix}{token}"),
                    }]));
                }
            }
            if let Some(nested) = self.apply_load_spec(
                &mut st,
                ls,
                i,
                Some(&token),
                &cur,
                start,
                cwd,
                dev,
                host,
                depth,
            )? {
                return Ok(nested);
            }
            if let Some(e) = alias_err {
                return Err(e);
            }
            substituted.clear();
            i += 1;
        }
        st.tokens = cur.tokens[start..].iter().map(|t| t.text.clone()).collect();
        if !dev {
            if self.prefix.len() >= MAX_CACHED {
                self.prefix.clear();
            }
            self.prefix.insert(cache_key, st.clone());
        }
        Ok(st)
    }

    /// `parserDirectives.alias` (git aliases): the token's expansion re-tokenized into the command.
    fn substitute_alias(
        &mut self,
        cmd: &CommandInfo,
        index: usize,
        token: &str,
        alias: &TextOrFunction,
        host: &mut dyn Host,
    ) -> Result<CommandInfo, ParseError> {
        let value = match alias {
            TextOrFunction::Str(s) => s.clone(),
            TextOrFunction::Fn(f) => {
                let v = host.call(*f, vec![JsArg::Json(token.into()), JsArg::Exec])?;
                v.as_str()
                    .ok_or_else(|| no("alias did not return a string"))?
                    .to_string()
            }
        };
        // `git config --get` output ends with "\n"; a newline would not split words in the re-tokenized command.
        host.substitute(cmd, index, value.trim_end())
            .ok_or_else(|| no(format!("invalid alias for {token:?}")))
    }

    /// `Some(state)` when a location loadSpec took over parsing (SPEC §6.4); inline subcommands replace the level.
    #[allow(clippy::too_many_arguments)]
    fn apply_load_spec(
        &mut self,
        st: &mut State,
        ls: Option<SpecLoadDirective>,
        index: usize,
        token: Option<&str>,
        cmd: &CommandInfo,
        start: usize,
        cwd: &Path,
        dev: bool,
        host: &mut dyn Host,
        depth: usize,
    ) -> Result<Option<State>, ParseError> {
        let ls = match (ls, token) {
            (Some(SpecLoadDirective::Fn(f)), Some(t)) => fig_spec_mapper::load_spec_result(
                &host.call(f, vec![JsArg::Json(t.into()), JsArg::Exec])?,
            ),
            (Some(SpecLoadDirective::Fn(_)), None) => None, // function loadSpecs are skipped at the root
            (ls, _) => ls,
        };
        match ls {
            Some(SpecLoadDirective::Locations(l)) => {
                let mut nested =
                    self.parse_prefix(cmd, l, start + index, cwd, dev, host, depth + 1)?;
                nested.command_index += index;
                let prefix = cmd.tokens[start..start + index]
                    .iter()
                    .map(|t| t.text.clone());
                nested.tokens = prefix.chain(nested.tokens).collect();
                Ok(Some(nested))
            }
            Some(SpecLoadDirective::Inline(sub)) => {
                // ponytail: parserDirectives merge per object, not per field (4 inline specs upstream).
                if sub.has_directives() {
                    st.directives = sub.directives().clone();
                }
                st.persistent = sub.persistent().clone();
                st.opt_args = ArgState::default();
                st.passed.clear();
                st.sub_args = ArgState::new(sub.args());
                st.entered_sub_args = false;
                st.set_completion(sub);
                Ok(None)
            }
            _ => Ok(None),
        }
    }

    /// Merge `generateSpec(tokens, exec)` into the current level (SPEC §6.4); errors (except an abort) leave the
    /// state unchanged.
    fn generate_spec(
        &mut self,
        mut st: State,
        tokens: &[String],
        dev: bool,
        host: &mut dyn Host,
    ) -> Result<State, ParseError> {
        let Some(f) = st.completion.generate_spec() else {
            return Ok(st);
        };
        let cache_key = match st.completion.generate_spec_cache_key() {
            Some(TextOrFunction::Str(k)) => Some(k.clone()),
            Some(TextOrFunction::Fn(kf)) => match host.call(
                *kf,
                vec![JsArg::Json(serde_json::json!({ "tokens": tokens }))],
            ) {
                Err(ParseError::Aborted) => return Err(ParseError::Aborted),
                r => r.ok().and_then(|v| v.as_str().map(String::from)),
            },
            None => None,
        }
        .filter(|_| !dev)
        .map(|k| format!("{}:{k}", tokens[0]));
        let generator = match cache_key.as_ref().and_then(|k| self.generated.get(k)) {
            Some(g) => g.clone(),
            None => {
                let generated = host
                    .call(f, vec![JsArg::Json(serde_json::json!(tokens)), JsArg::Exec])
                    .and_then(|v| normalise(&v).map_err(|e| no(e.to_string())));
                match generated {
                    Ok(g) => {
                        let g = Rc::new(g);
                        if let Some(k) = cache_key {
                            if self.generated.len() >= MAX_CACHED {
                                self.generated.clear();
                            }
                            self.generated.insert(k, g.clone());
                        }
                        g
                    }
                    Err(ParseError::Aborted) => return Err(ParseError::Aborted),
                    Err(e) => {
                        logger::warn(&format!("generateSpec ({}): {e:?}", tokens[0])); // spec name only: logs never hold typed text
                        return Ok(st);
                    }
                }
            }
        };
        let args_were_empty = st.completion.args().is_empty();
        let c = st.completion.merged_with_generated(&generator);
        if args_were_empty {
            st.sub_args = ArgState::new(c.args());
        }
        st.persistent = generator
            .persistent()
            .iter()
            .chain(&st.persistent)
            .cloned()
            .collect();
        st.set_completion(Rc::new(c));
        Ok(st)
    }
}
