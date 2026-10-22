//! Completion-spec model and normalisation from the JSON produced by the JS host (SPEC §6.2).
//! Functions inside specs arrive as `{"$fn": <id>}` and are kept as [`FnId`]s, called through `crate::js::Js`.
//! OWNER: parser agent (struct shapes are shared with the suggest agent — extend, don't rename).
use crate::types::ItemType;
use serde_json::Value;
use std::collections::HashMap;
use std::path::PathBuf;
use std::rc::Rc;

/// Index into the JS host's function table (`__ts.fns`).
pub type FnId = u32;

#[derive(Clone, Debug, PartialEq)]
pub enum StrOrFn {
    Str(String),
    Fn(FnId),
}

/// A static suggestion, `additionalSuggestions` entry or generator result, before filtering.
#[derive(Clone, Debug, Default)]
pub struct Suggestion {
    pub names: Vec<String>,
    pub display_name: Option<String>,
    pub insert_value: Option<String>,
    pub description: Option<String>,
    /// `None` → caller decides (static arg suggestions/generator results default to Arg).
    pub kind: Option<ItemType>,
    pub priority: Option<f64>,
    pub hidden: bool,
    pub is_dangerous: bool,
    /// `context.templateType` (filepaths/folders/help).
    pub template_type: Option<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Trigger {
    /// Re-run when the last index of this string in the search term changes.
    Str(String),
    /// `trigger(newToken, oldToken) -> bool`.
    Fn(FnId),
    /// `{on: "change"}`.
    Change,
    /// `{on: "threshold", length}`.
    Threshold(usize),
    /// `{on: "match", string: [...]}`.
    Match(Vec<String>),
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct CacheCfg {
    pub ttl_ms: u64,
    /// `strategy: "max-age"` (else stale-while-revalidate).
    pub max_age: bool,
    pub cache_by_directory: bool,
    pub cache_key: Option<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Script {
    /// `["git", "branch"]`
    Argv(Vec<String>),
    /// Legacy string form (`sh -c`).
    Shell(String),
    /// `script(tokens) -> string[] | string | {command,args,...}`.
    Fn(FnId),
    /// `{command, args, cwd?, env?, timeout?}` object form.
    Object(Value),
}

#[derive(Clone, Debug, Default)]
pub struct Generator {
    /// `filepaths` | `folders` | `history` | `help` (from `template`).
    pub template: Vec<String>,
    pub script: Option<Script>,
    pub post_process: Option<FnId>,
    pub split_on: Option<String>,
    pub custom: Option<FnId>,
    pub trigger: Option<Trigger>,
    pub get_query_term: Option<StrOrFn>,
    pub cache: Option<CacheCfg>,
    pub script_timeout: Option<u64>,
    pub filter_template_suggestions: Option<FnId>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Repeat {
    /// Absent/false: may be passed once.
    No,
    Times(u32),
    Unlimited,
}

impl Default for Repeat {
    fn default() -> Self {
        Repeat::No
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Strategy {
    #[default]
    Default,
    Prefix,
    Fuzzy,
}

#[derive(Clone, Debug, Default)]
pub struct Arg {
    pub name: Option<String>,
    pub description: Option<String>,
    pub is_optional: bool,
    pub is_variadic: bool,
    pub is_dangerous: bool,
    pub is_command: bool,
    pub is_script: bool,
    /// `isModule: "python/"`.
    pub is_module: Option<String>,
    pub suggest_current_token: Option<bool>,
    /// `debounce`: true → 200 ms, number → ms.
    pub debounce: Option<u64>,
    pub options_can_break_variadic: Option<bool>,
    pub suggestions: Vec<Suggestion>,
    pub generators: Vec<Generator>,
    pub load_spec: Option<LoadSpec>,
    pub filter_strategy: Strategy,
    /// `parserDirectives.alias` (git aliases).
    pub alias_directive: Option<StrOrFn>,
    pub default: Option<String>,
}

#[derive(Clone, Debug, Default)]
pub struct Opt {
    pub names: Vec<String>,
    pub description: Option<String>,
    pub display_name: Option<String>,
    pub insert_value: Option<String>,
    pub priority: Option<f64>,
    pub hidden: bool,
    pub is_dangerous: bool,
    pub args: Vec<Rc<Arg>>,
    pub is_persistent: bool,
    pub is_repeatable: Repeat,
    pub requires_equals: bool,
    /// `requiresSeparator`: `Some("=")`/`Some(":")`; `true` → `Some("")` (= use first optionArgSeparator).
    pub requires_separator: Option<String>,
    pub exclusive_on: Vec<String>,
    pub depends_on: Vec<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Directives {
    pub flags_posix_noncompliant: bool,
    pub options_must_precede_args: bool,
    /// Default `["="]`.
    pub option_arg_separators: Vec<String>,
}

impl Default for Directives {
    fn default() -> Self {
        Directives {
            flags_posix_noncompliant: false,
            options_must_precede_args: false,
            option_arg_separators: vec!["=".into()],
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum SpecLocation {
    /// A bundled/user spec by global name, e.g. `git`, `aws/s3`.
    Global { name: String },
    /// `<dir>/.fig/autocomplete/build/<name>.js`.
    Local { dir: PathBuf, name: String },
}

#[derive(Clone, Debug)]
pub enum LoadSpec {
    Locations(Vec<SpecLocation>),
    Inline(Rc<Subcommand>),
    /// `loadSpec(token, executeShellCommand)`.
    Fn(FnId),
}

#[derive(Clone, Debug, Default)]
pub struct Subcommand {
    pub names: Vec<String>,
    pub description: Option<String>,
    pub display_name: Option<String>,
    pub insert_value: Option<String>,
    pub icon: Option<String>,
    pub priority: Option<f64>,
    pub hidden: bool,
    pub is_dangerous: bool,
    pub subcommands: Vec<Rc<Subcommand>>,
    /// Every alias → index into `subcommands`.
    pub sub_index: HashMap<String, usize>,
    /// Non-persistent options.
    pub options: Vec<Rc<Opt>>,
    /// Options declared `isPersistent` on this level.
    pub persistent: Vec<Rc<Opt>>,
    /// Every option name → (is_persistent, index into `options`/`persistent`).
    pub opt_index: HashMap<String, (bool, usize)>,
    pub args: Vec<Rc<Arg>>,
    pub load_spec: Option<LoadSpec>,
    pub generate_spec: Option<FnId>,
    pub generate_spec_cache_key: Option<StrOrFn>,
    pub directives: Directives,
    /// True if `parserDirectives` was given explicitly at this level (for inheritance).
    pub has_directives: bool,
    pub additional_suggestions: Vec<Suggestion>,
    pub filter_strategy: Strategy,
    pub requires_subcommand: Option<bool>,
}

/// Normalise a spec JSON object (SPEC §6.2). Never panics on odd specs; skips malformed parts.
pub fn normalise(v: &Value) -> anyhow::Result<Subcommand> {
    let _ = v;
    todo!("parser agent: SPEC §6.2")
}

/// Parse one suggestion: a string or an object with `name` (string or array).
pub fn suggestion_from_json(v: &Value) -> Option<Suggestion> {
    let _ = v;
    todo!("parser agent: SPEC §6.2")
}

/// `{"$fn": id}` → id.
pub fn fn_id(v: &Value) -> Option<FnId> {
    v.get("$fn").and_then(Value::as_u64).map(|n| n as FnId)
}
