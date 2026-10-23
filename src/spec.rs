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
    pub icon: Option<String>,
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

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Repeat {
    /// Absent: may be passed once (suggestions); the parser does not enforce it (qac). Explicit `false` → `Times(1)`.
    #[default]
    No,
    Times(u32),
    Unlimited,
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
    pub icon: Option<String>,
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

impl Subcommand {
    /// Rebuild `sub_index`/`opt_index` from the lists (later wins; options win over persistent, like qac's
    /// `options[t] || persistentOptions[t]`).
    pub fn reindex(&mut self) {
        self.sub_index.clear();
        for (i, s) in self.subcommands.iter().enumerate() {
            for n in &s.names {
                self.sub_index.insert(n.clone(), i);
            }
        }
        self.opt_index.clear();
        for (persistent, list) in [(true, &self.persistent), (false, &self.options)] {
            for (i, o) in list.iter().enumerate() {
                for n in &o.names {
                    self.opt_index.insert(n.clone(), (persistent, i));
                }
            }
        }
    }
}

/// Normalise a spec JSON object (SPEC §6.2). Never panics on odd specs; skips malformed parts.
pub fn normalise(v: &Value) -> anyhow::Result<Subcommand> {
    anyhow::ensure!(v.is_object(), "spec is not an object");
    Ok(subcommand(v))
}

/// Parse one suggestion: a string or an object with `name` (string or array).
pub fn suggestion_from_json(v: &Value) -> Option<Suggestion> {
    if let Some(s) = v.as_str() {
        return Some(Suggestion {
            names: vec![s.to_string()],
            ..Default::default()
        });
    }
    let names = strs(v.get("name"));
    if !v.is_object() || names.is_empty() {
        return None;
    }
    Some(Suggestion {
        names,
        display_name: s(v, "displayName"),
        insert_value: s(v, "insertValue"),
        description: s(v, "description"),
        kind: v
            .get("type")
            .and_then(Value::as_str)
            .map(ItemType::from_fig),
        priority: v.get("priority").and_then(Value::as_f64),
        hidden: b(v, "hidden"),
        is_dangerous: b(v, "isDangerous"),
        template_type: v.get("context").and_then(|c| s(c, "templateType")),
        icon: s(v, "icon"),
    })
}

/// Result of a function `loadSpec` (SPEC §6.2): `SpecLocation[]`, one location (`{name, type}`) or a Subcommand.
pub fn load_spec_result(v: &Value) -> Option<LoadSpec> {
    match v {
        Value::Array(a) => Some(LoadSpec::Locations(a.iter().filter_map(location).collect())),
        Value::Object(o) if o.contains_key("type") => {
            Some(LoadSpec::Locations(location(v).into_iter().collect()))
        }
        Value::Object(_) => Some(LoadSpec::Inline(Rc::new(subcommand(v)))),
        _ => None,
    }
}

/// `{name, type: "global"|"local", path?}`; a local without `path` lives in `~/` (qac `(path || "~/") + name`).
fn location(v: &Value) -> Option<SpecLocation> {
    let name = s(v, "name")?;
    match v.get("type")?.as_str()? {
        "global" => Some(SpecLocation::Global { name }),
        "local" => Some(SpecLocation::Local {
            dir: PathBuf::from(s(v, "path").unwrap_or_else(|| "~/".into())),
            name,
        }),
        _ => None,
    }
}

fn subcommand(v: &Value) -> Subcommand {
    let mut sc = Subcommand {
        names: strs(v.get("name")),
        description: s(v, "description"),
        display_name: s(v, "displayName"),
        insert_value: s(v, "insertValue"),
        icon: s(v, "icon"),
        priority: v.get("priority").and_then(Value::as_f64),
        hidden: b(v, "hidden"),
        is_dangerous: b(v, "isDangerous"),
        subcommands: objects(v.get("subcommands"))
            .map(|x| Rc::new(subcommand(x)))
            .collect(),
        args: args(v.get("args")),
        load_spec: v.get("loadSpec").and_then(load_spec),
        generate_spec: v.get("generateSpec").and_then(fn_id),
        generate_spec_cache_key: v.get("generateSpecCacheKey").and_then(str_or_fn),
        additional_suggestions: items(v.get("additionalSuggestions"))
            .filter_map(suggestion_from_json)
            .collect(),
        filter_strategy: strategy(v),
        requires_subcommand: v.get("requiresSubcommand").and_then(Value::as_bool),
        ..Default::default()
    };
    if let Some(d) = v.get("parserDirectives").filter(|d| d.is_object()) {
        sc.has_directives = true;
        sc.directives = Directives {
            flags_posix_noncompliant: b(d, "flagsArePosixNoncompliant"),
            options_must_precede_args: b(d, "optionsMustPrecedeArguments"),
            // qac builds a Set from it: a string is a set of chars.
            option_arg_separators: match d.get("optionArgSeparators") {
                Some(Value::String(x)) => x.chars().map(String::from).collect(),
                Some(Value::Array(_)) => strs(d.get("optionArgSeparators")),
                _ => vec!["=".into()],
            },
        };
    }
    for o in objects(v.get("options")).map(option) {
        if o.is_persistent {
            sc.persistent.push(Rc::new(o));
        } else {
            sc.options.push(Rc::new(o));
        }
    }
    sc.reindex();
    sc
}

fn option(v: &Value) -> Opt {
    Opt {
        names: strs(v.get("name")),
        description: s(v, "description"),
        display_name: s(v, "displayName"),
        insert_value: s(v, "insertValue"),
        icon: s(v, "icon"),
        priority: v.get("priority").and_then(Value::as_f64),
        hidden: b(v, "hidden"),
        is_dangerous: b(v, "isDangerous"),
        args: args(v.get("args")),
        is_persistent: b(v, "isPersistent"),
        is_repeatable: match v.get("isRepeatable") {
            Some(Value::Bool(true)) => Repeat::Unlimited,
            // Explicit `false` is enforced by the parser in qac (absent is not): same limit as `No` for suggestions.
            Some(Value::Bool(false)) => Repeat::Times(1),
            Some(n) if n.is_number() => Repeat::Times(n.as_f64().unwrap_or(1.0) as u32),
            _ => Repeat::No,
        },
        requires_equals: b(v, "requiresEquals"),
        requires_separator: match v.get("requiresSeparator") {
            Some(Value::String(x)) => Some(x.clone()),
            Some(Value::Bool(true)) => Some(String::new()),
            _ => None,
        },
        exclusive_on: strs(v.get("exclusiveOn")),
        depends_on: strs(v.get("dependsOn")),
    }
}

fn args(v: Option<&Value>) -> Vec<Rc<Arg>> {
    objects(v).map(|a| Rc::new(arg(a))).collect()
}

fn arg(v: &Value) -> Arg {
    let mut generators: Vec<Generator> = objects(v.get("generators")).map(generator).collect();
    let template = strs(v.get("template"));
    if !template.is_empty() {
        generators.insert(0, generator(&serde_json::json!({ "template": template })));
    }
    Arg {
        name: s(v, "name"),
        description: s(v, "description"),
        is_optional: b(v, "isOptional"),
        is_variadic: b(v, "isVariadic"),
        is_dangerous: b(v, "isDangerous"),
        is_command: b(v, "isCommand"),
        is_script: b(v, "isScript"),
        is_module: s(v, "isModule"),
        suggest_current_token: v.get("suggestCurrentToken").and_then(Value::as_bool),
        debounce: match v.get("debounce") {
            Some(Value::Bool(true)) => Some(200),
            Some(n) => n.as_f64().filter(|&ms| ms > 0.0).map(|ms| ms as u64),
            None => None,
        },
        options_can_break_variadic: v.get("optionsCanBreakVariadicArg").and_then(Value::as_bool),
        suggestions: items(v.get("suggestions"))
            .filter_map(suggestion_from_json)
            .collect(),
        generators,
        load_spec: v.get("loadSpec").and_then(load_spec),
        filter_strategy: strategy(v),
        alias_directive: v
            .get("parserDirectives")
            .and_then(|d| d.get("alias"))
            .and_then(str_or_fn),
        default: s(v, "default"),
    }
}

fn generator(v: &Value) -> Generator {
    let template = strs(v.get("template"));
    // Path templates re-run when the directory part changes and query only the basename (SPEC §6.2, §6.6.4).
    let path_template = template.iter().any(|t| t == "filepaths" || t == "folders");
    Generator {
        script: v.get("script").and_then(|x| match x {
            Value::Array(_) => Some(Script::Argv(strs(Some(x)))),
            Value::String(c) => Some(Script::Shell(c.clone())),
            Value::Object(_) => {
                Some(fn_id(x).map_or_else(|| Script::Object(x.clone()), Script::Fn))
            }
            _ => None,
        }),
        post_process: v.get("postProcess").and_then(fn_id),
        split_on: s(v, "splitOn"),
        custom: v.get("custom").and_then(fn_id),
        trigger: match v.get("trigger") {
            Some(Value::String(t)) => Some(Trigger::Str(t.clone())),
            Some(t) if fn_id(t).is_some() => fn_id(t).map(Trigger::Fn),
            Some(t) => match t.get("on").and_then(Value::as_str) {
                Some("change") => Some(Trigger::Change),
                Some("threshold") => Some(Trigger::Threshold(
                    t.get("length").and_then(Value::as_f64).unwrap_or(0.0) as usize,
                )),
                Some("match") => Some(Trigger::Match(strs(t.get("string")))),
                _ => None,
            },
            None if path_template => Some(Trigger::Str("/".into())),
            None => None,
        },
        get_query_term: v
            .get("getQueryTerm")
            .and_then(str_or_fn)
            .or_else(|| path_template.then(|| StrOrFn::Str("/".into()))),
        cache: v.get("cache").filter(|c| c.is_object()).map(|c| CacheCfg {
            ttl_ms: c.get("ttl").and_then(Value::as_f64).unwrap_or(0.0) as u64,
            max_age: c.get("strategy").and_then(Value::as_str) == Some("max-age"),
            cache_by_directory: b(c, "cacheByDirectory"),
            cache_key: s(c, "cacheKey"),
        }),
        script_timeout: v
            .get("scriptTimeout")
            .and_then(Value::as_f64)
            .map(|ms| ms as u64),
        filter_template_suggestions: v.get("filterTemplateSuggestions").and_then(fn_id),
        template,
    }
}

/// String → one global location; `{"$fn"}` → function; other object → inline subcommand.
fn load_spec(v: &Value) -> Option<LoadSpec> {
    match v {
        Value::String(name) => Some(LoadSpec::Locations(vec![SpecLocation::Global {
            name: name.clone(),
        }])),
        Value::Object(_) => {
            Some(fn_id(v).map_or_else(|| LoadSpec::Inline(Rc::new(subcommand(v))), LoadSpec::Fn))
        }
        _ => None,
    }
}

fn str_or_fn(v: &Value) -> Option<StrOrFn> {
    v.as_str()
        .map(|x| StrOrFn::Str(x.into()))
        .or_else(|| fn_id(v).map(StrOrFn::Fn))
}

fn strategy(v: &Value) -> Strategy {
    match v.get("filterStrategy").and_then(Value::as_str) {
        Some("prefix") => Strategy::Prefix,
        Some("fuzzy") => Strategy::Fuzzy,
        _ => Strategy::Default,
    }
}

/// `makeArray`: absent/null → [], array → its items, anything else → [it].
fn items(v: Option<&Value>) -> impl Iterator<Item = &Value> {
    let list: &[Value] = match v {
        None | Some(Value::Null) => &[],
        Some(Value::Array(a)) => a,
        Some(x) => std::slice::from_ref(x),
    };
    list.iter()
}

fn objects(v: Option<&Value>) -> impl Iterator<Item = &Value> {
    items(v).filter(|x| x.is_object())
}

fn strs(v: Option<&Value>) -> Vec<String> {
    items(v)
        .filter_map(|x| x.as_str().map(String::from))
        .collect()
}

fn s(v: &Value, key: &str) -> Option<String> {
    v.get(key).and_then(Value::as_str).map(String::from)
}

fn b(v: &Value, key: &str) -> bool {
    v.get(key).and_then(Value::as_bool).unwrap_or(false)
}

/// `{"$fn": id}` → id.
pub fn fn_id(v: &Value) -> Option<FnId> {
    v.get("$fn").and_then(Value::as_u64).map(|n| n as FnId)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn fixture(name: &str) -> Subcommand {
        let path = format!(
            "{}/tests/fixtures/parser/{name}.json",
            env!("CARGO_MANIFEST_DIR")
        );
        normalise(&serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()).unwrap()
    }

    fn sub<'a>(s: &'a Subcommand, name: &str) -> &'a Subcommand {
        &s.subcommands[s.sub_index[name]]
    }

    fn opt<'a>(s: &'a Subcommand, flag: &str) -> &'a Opt {
        match s.opt_index[flag] {
            (true, i) => &s.persistent[i],
            (false, i) => &s.options[i],
        }
    }

    #[test]
    fn normalise_sudo() {
        let s = fixture("sudo");
        assert_eq!(s.names, ["sudo"]);
        assert_eq!(s.options.len(), 3);
        assert_eq!(opt(&s, "--group").names, ["-g", "--group"]);
        assert!(std::ptr::eq(opt(&s, "-u"), opt(&s, "--user")));
        assert_eq!(opt(&s, "-g").args[0].name.as_deref(), Some("group"));
        assert!(opt(&s, "-h").args.is_empty());
        assert_eq!(s.args.len(), 1);
        assert!(s.args[0].is_command && !s.args[0].is_optional);
        assert_eq!(s.directives, Directives::default());
        assert!(!s.has_directives);
    }

    #[test]
    fn normalise_cd_function_generator() {
        let s = fixture("cd");
        let a = &s.args[0];
        assert_eq!(a.filter_strategy, Strategy::Fuzzy);
        let g = &a.generators[0];
        assert_eq!(
            (g.trigger.clone(), g.get_query_term.clone(), g.custom),
            (Some(Trigger::Fn(0)), Some(StrOrFn::Fn(1)), Some(2))
        );
        assert_eq!(
            a.suggestions
                .iter()
                .map(|x| (x.names[0].as_str(), x.hidden))
                .collect::<Vec<_>>(),
            [("-", true), ("~", true)]
        );
    }

    #[test]
    fn normalise_templates_and_persistent() {
        let s = fixture("tailcall");
        let check = sub(&s, "check");
        assert_eq!((check.persistent.len(), check.options.len()), (2, 1));
        assert_eq!(check.opt_index["--schema"], (true, 1));
        assert_eq!(check.opt_index["--format"], (false, 0));
        assert_eq!(opt(check, "--format").args[0].suggestions.len(), 5);
        let g = &check.args[0].generators[0];
        assert_eq!(g.template, ["filepaths"]);
        assert_eq!(
            (g.trigger.clone(), g.get_query_term.clone()),
            (
                Some(Trigger::Str("/".into())),
                Some(StrOrFn::Str("/".into()))
            )
        );
        assert!(check.args[0].is_variadic);
        assert_eq!(
            sub(&s, "start").args[0].generators[0].template,
            ["filepaths", "folders"]
        );
        assert_eq!(
            sub(&s, "init").args[0].generators[0].trigger,
            Some(Trigger::Str("/".into()))
        );
    }

    #[test]
    fn normalise_requires_separator() {
        let s = fixture("do-release-upgrade");
        assert_eq!(opt(&s, "--mode").requires_separator.as_deref(), Some(""));
        assert_eq!(opt(&s, "-m").requires_separator, None);
        assert!(opt(&s, "-m").hidden);
        assert_eq!(opt(&s, "--mode").args[0].suggestions[1].names, ["server"]);
    }

    #[test]
    fn normalise_git_like() {
        let s = fixture("git_min");
        assert_eq!(s.generate_spec, Some(7));
        assert_eq!(s.args[0].alias_directive, Some(StrOrFn::Fn(8)));
        assert!(s.has_directives);
        assert_eq!(s.directives.option_arg_separators, ["=", ":"]); // a string is a set of chars
        let short = &s.additional_suggestions[0];
        assert_eq!(
            (short.kind, short.insert_value.as_deref()),
            (Some(ItemType::Shortcut), Some("commit -m '{cursor}'"))
        );
        assert_eq!(opt(&s, "-C").is_repeatable, Repeat::Unlimited);
        assert_eq!(opt(&s, "-c").is_repeatable, Repeat::Times(3));
        assert_eq!(opt(&s, "--version").is_repeatable, Repeat::No);
        assert!(opt(&s, "--no-pager").is_persistent);
        assert_eq!(s.opt_index["--no-pager"], (true, 0));

        let commit = sub(&s, "ci");
        assert!(std::ptr::eq(commit, sub(&s, "commit")));
        assert_eq!(opt(commit, "--amend").is_repeatable, Repeat::Times(1)); // explicit false
        assert_eq!(opt(commit, "--amend").exclusive_on, ["--no-edit"]);
        assert_eq!(
            opt(commit, "--pathspec-file-nul").depends_on,
            ["--pathspec-from-file"]
        );
        assert_eq!(opt(commit, "-m").args[0].debounce, Some(200));
        let a = &commit.args[0];
        assert_eq!(a.options_can_break_variadic, Some(false));
        let g = &a.generators[0];
        assert_eq!(
            g.script,
            Some(Script::Argv(vec![
                "git".into(),
                "--no-optional-locks".into(),
                "status".into(),
                "--short".into()
            ]))
        );
        assert_eq!(g.post_process, Some(9));
        assert_eq!(
            g.cache,
            Some(CacheCfg {
                ttl_ms: 1000,
                max_age: true,
                cache_by_directory: true,
                cache_key: None
            })
        );

        let co = sub(&s, "checkout");
        let (g0, g1) = (&co.args[0].generators[0], &co.args[0].generators[1]);
        assert_eq!(
            (g0.script.clone(), g0.trigger.clone(), g0.script_timeout),
            (
                Some(Script::Fn(10)),
                Some(Trigger::Threshold(2)),
                Some(15000)
            )
        );
        assert_eq!(g0.get_query_term, Some(StrOrFn::Str("/".into())));
        assert_eq!(
            (
                g1.custom,
                g1.trigger.clone(),
                g1.filter_template_suggestions
            ),
            (Some(12), Some(Trigger::Change), Some(13))
        );
        assert_eq!(co.args[0].suggest_current_token, Some(true));
        assert_eq!(co.args[1].generators[0].template, ["filepaths"]);

        assert!(
            matches!(&sub(&s, "flow").load_spec, Some(LoadSpec::Locations(l)) if l == &[SpecLocation::Global { name: "git-flow".into() }])
        );
        let help = &sub(&s, "help").args[0];
        assert_eq!(
            (help.default.as_deref(), help.generators[0].trigger.clone()),
            (Some("git"), None)
        );
    }

    #[test]
    fn normalise_mock_spec() {
        let s = fixture("cmd_spec");
        let man = sub(&s, "man");
        assert_eq!(
            (man.args.len(), man.options.len(), man.subcommands.len()),
            (2, 10, 4)
        );
        assert_eq!(opt(man, "--multiple").args.len(), 2);
        assert_eq!(
            opt(man, "--req-sep").requires_separator.as_deref(),
            Some(":")
        );
        assert!(opt(man, "--req-eq").requires_equals);
        assert_eq!(sub(&s, "oas").directives.option_arg_separators, ["=", ":"]);
        assert!(sub(&s, "pnf").directives.flags_posix_noncompliant);
        assert!(sub(&s, "ompa").directives.options_must_precede_args);
        assert_eq!(sub(&s, "generateSpec").generate_spec, Some(1));
        assert_eq!(
            sub(&s, "module").args[0].is_module.as_deref(),
            Some("python/")
        );
    }

    #[test]
    fn normalise_shapes_and_collisions() {
        assert!(normalise(&json!([1])).is_err());
        let s = normalise(&json!({
            "name": "x",
            "subcommands": [{"name": ["a", "b"]}, {"name": "b"}, "junk"],
            "options": [{"name": "-o", "isPersistent": true}, {"name": "-o"}],
            "args": {"name": "single", "debounce": 50, "loadSpec": {"name": "inner", "args": {}}},
            "loadSpec": {"$fn": 4}
        }))
        .unwrap();
        assert_eq!(
            (s.sub_index["a"], s.sub_index["b"], s.subcommands.len()),
            (0, 1, 2)
        ); // later wins
        assert_eq!(s.opt_index["-o"], (false, 0)); // options win over persistent
        assert_eq!((s.args.len(), s.args[0].debounce), (1, Some(50)));
        assert!(
            matches!(&s.args[0].load_spec, Some(LoadSpec::Inline(i)) if i.names == ["inner"] && i.args.len() == 1)
        );
        assert!(matches!(s.load_spec, Some(LoadSpec::Fn(4))));
    }

    #[test]
    fn suggestions_and_load_spec_results() {
        assert_eq!(suggestion_from_json(&json!("a")).unwrap().names, ["a"]);
        let o = suggestion_from_json(&json!({"name": ["x", "y"], "type": "folder", "context": {"templateType": "folders"}, "priority": 80})).unwrap();
        assert_eq!(
            (
                o.names.len(),
                o.kind,
                o.template_type.as_deref(),
                o.priority
            ),
            (2, Some(ItemType::Folder), Some("folders"), Some(80.0))
        );
        assert!(suggestion_from_json(&json!({"description": "no name"})).is_none());
        assert!(suggestion_from_json(&json!(3)).is_none());

        let locs = |v: Value| match load_spec_result(&v) {
            Some(LoadSpec::Locations(l)) => l,
            other => panic!("{other:?}"),
        };
        assert_eq!(
            locs(json!({"name": "create-x", "type": "global"})),
            [SpecLocation::Global {
                name: "create-x".into()
            }]
        );
        assert_eq!(
            locs(
                json!([{"name": "a", "type": "local", "path": "/p/"}, {"name": "b", "type": "local"}])
            ),
            [
                SpecLocation::Local {
                    dir: "/p/".into(),
                    name: "a".into()
                },
                SpecLocation::Local {
                    dir: "~/".into(),
                    name: "b".into()
                }
            ]
        );
        assert!(matches!(
            load_spec_result(&json!({"name": "sub", "args": {}})),
            Some(LoadSpec::Inline(_))
        ));
        assert!(load_spec_result(&json!("str")).is_none());
    }
}
