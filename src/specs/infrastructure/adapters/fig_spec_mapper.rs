//! The Fig spec JSON produced by the JS host, read into the spec model (SPEC §6.2), and back.
//! Functions inside specs arrive as `{"$fn": <id>}` and are kept as `SpecFunctionRef`s.
use crate::specs::domain::entities::subcommand::Subcommand;
use crate::specs::domain::spec_error::SpecError;
use crate::specs::domain::value_objects::arg::Arg;
use crate::specs::domain::value_objects::custom_generator_result::CustomGeneratorResult;
use crate::specs::domain::value_objects::exec_request::ExecRequest;
use crate::specs::domain::value_objects::filter_strategy::FilterStrategy;
use crate::specs::domain::value_objects::generator::Generator;
use crate::specs::domain::value_objects::generator_cache_policy::GeneratorCachePolicy;
use crate::specs::domain::value_objects::generator_context::GeneratorContext;
use crate::specs::domain::value_objects::generator_script::GeneratorScript;
use crate::specs::domain::value_objects::generator_trigger::GeneratorTrigger;
use crate::specs::domain::value_objects::parser_directives::ParserDirectives;
use crate::specs::domain::value_objects::repeatability::Repeatability;
use crate::specs::domain::value_objects::script_resolution::ScriptResolution;
use crate::specs::domain::value_objects::spec_function_kind::SpecFunctionKind;
use crate::specs::domain::value_objects::spec_function_ref::SpecFunctionRef;
use crate::specs::domain::value_objects::spec_function_result::SpecFunctionResult;
use crate::specs::domain::value_objects::spec_load_directive::SpecLoadDirective;
use crate::specs::domain::value_objects::spec_location::SpecLocation;
use crate::specs::domain::value_objects::spec_option::SpecOption;
use crate::specs::domain::value_objects::spec_suggestion::SpecSuggestion;
use crate::specs::domain::value_objects::spec_value::SpecValue;
use crate::specs::domain::value_objects::suggestion_kind::SuggestionKind;
use crate::specs::domain::value_objects::text_or_function::TextOrFunction;
use crate::specs::infrastructure::adapters::exec_input::ExecInput;
use serde_json::{json, Number, Value};
use std::path::PathBuf;
use std::rc::Rc;

/// Normalise a spec JSON object (SPEC §6.2). Never panics on odd specs; skips malformed parts.
pub(crate) fn normalise(v: &Value) -> Result<Subcommand, SpecError> {
    if !v.is_object() {
        return Err(SpecError::Invalid("spec is not an object".into()));
    }
    Ok(subcommand(v))
}

/// Parse one suggestion: a string or an object with `name` (string or array).
fn suggestion_from_json(v: &Value) -> Option<SpecSuggestion> {
    if let Some(s) = v.as_str() {
        return Some(SpecSuggestion::default().with_names(vec![s.to_string()]));
    }
    let names = strs(v.get("name"));
    if !v.is_object() || names.is_empty() {
        return None;
    }
    Some(
        SpecSuggestion::default()
            .with_names(names)
            .with_display_name(s(v, "displayName"))
            .with_insert_value(s(v, "insertValue"))
            .with_description(s(v, "description"))
            .with_kind(
                v.get("type")
                    .and_then(Value::as_str)
                    .map(from_fig),
            )
            .with_priority(v.get("priority").and_then(Value::as_f64))
            .with_hidden(b(v, "hidden"))
            .with_is_dangerous(b(v, "isDangerous"))
            .with_template_type(v.get("context").and_then(|c| s(c, "templateType")))
            .with_icon(s(v, "icon")),
    )
}

/// Fig type string → type (`mixin` → Special, unknown → Arg).
fn from_fig(s: &str) -> SuggestionKind {
    match s {
        "subcommand" => SuggestionKind::Subcommand,
        "option" => SuggestionKind::Option,
        "file" => SuggestionKind::File,
        "folder" => SuggestionKind::Folder,
        "special" | "mixin" => SuggestionKind::Special,
        "shortcut" => SuggestionKind::Shortcut,
        "history" => SuggestionKind::History,
        "auto-execute" => SuggestionKind::AutoExecute,
        _ => SuggestionKind::Arg,
    }
}

/// Result of a function `loadSpec` (SPEC §6.2): `SpecLocation[]`, one location (`{name, type}`) or a Subcommand.
pub(crate) fn load_spec_result(v: &Value) -> Option<SpecLoadDirective> {
    match v {
        Value::Array(a) => Some(SpecLoadDirective::Locations(a.iter().filter_map(location).collect())),
        Value::Object(o) if o.contains_key("type") => {
            Some(SpecLoadDirective::Locations(location(v).into_iter().collect()))
        }
        Value::Object(_) => Some(SpecLoadDirective::Inline(Rc::new(subcommand(v)))),
        _ => None,
    }
}

/// `{name, type: "global"|"local", path?}`; a local without `path` lives in `~/` (`(path || "~/") + name`).
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
    let mut sc = Subcommand::default()
        .with_names(strs(v.get("name")))
        .with_description(s(v, "description"))
        .with_display_name(s(v, "displayName"))
        .with_insert_value(s(v, "insertValue"))
        .with_icon(s(v, "icon"))
        .with_priority(v.get("priority").and_then(Value::as_f64))
        .with_hidden(b(v, "hidden"))
        .with_is_dangerous(b(v, "isDangerous"))
        .with_subcommands(
            objects(v.get("subcommands"))
                .map(|x| Rc::new(subcommand(x)))
                .collect(),
        )
        .with_args(args(v.get("args")))
        .with_load_spec(v.get("loadSpec").and_then(load_spec))
        .with_generate_spec(v.get("generateSpec").and_then(fn_id))
        .with_generate_spec_cache_key(v.get("generateSpecCacheKey").and_then(str_or_fn))
        .with_additional_suggestions(
            items(v.get("additionalSuggestions"))
                .filter_map(suggestion_from_json)
                .collect(),
        )
        .with_filter_strategy(strategy(v))
        .with_requires_subcommand(v.get("requiresSubcommand").and_then(Value::as_bool));
    if let Some(d) = v.get("parserDirectives").filter(|d| d.is_object()) {
        let directives = ParserDirectives::default()
            .with_flags_posix_noncompliant(b(d, "flagsArePosixNoncompliant"))
            .with_options_must_precede_args(b(d, "optionsMustPrecedeArguments"))
            // The separators form a set: a string is a set of chars.
            .with_option_arg_separators(match d.get("optionArgSeparators") {
                Some(Value::String(x)) => x.chars().map(String::from).collect(),
                Some(Value::Array(_)) => strs(d.get("optionArgSeparators")),
                _ => vec!["=".into()],
            });
        sc = sc.with_directives(directives);
    }
    let (mut persistent, mut options) = (Vec::new(), Vec::new());
    for o in objects(v.get("options")).map(option) {
        if o.is_persistent() {
            persistent.push(Rc::new(o));
        } else {
            options.push(Rc::new(o));
        }
    }
    sc = sc.with_persistent(persistent).with_options(options);
    sc.reindex();
    sc
}

fn option(v: &Value) -> SpecOption {
    SpecOption::default()
        .with_names(strs(v.get("name")))
        .with_description(s(v, "description"))
        .with_display_name(s(v, "displayName"))
        .with_insert_value(s(v, "insertValue"))
        .with_icon(s(v, "icon"))
        .with_priority(v.get("priority").and_then(Value::as_f64))
        .with_hidden(b(v, "hidden"))
        .with_is_dangerous(b(v, "isDangerous"))
        .with_args(args(v.get("args")))
        .with_is_persistent(b(v, "isPersistent"))
        .with_is_repeatable(match v.get("isRepeatable") {
            Some(Value::Bool(true)) => Repeatability::Unlimited,
            // Explicit `false` is enforced by the parser (absent is not): same limit as `No` for suggestions.
            Some(Value::Bool(false)) => Repeatability::Times(1),
            Some(n) if n.is_number() => Repeatability::Times(n.as_f64().unwrap_or(1.0) as u32),
            _ => Repeatability::No,
        })
        .with_requires_equals(b(v, "requiresEquals"))
        .with_requires_separator(match v.get("requiresSeparator") {
            Some(Value::String(x)) => Some(x.clone()),
            Some(Value::Bool(true)) => Some(String::new()),
            _ => None,
        })
        .with_exclusive_on(strs(v.get("exclusiveOn")))
        .with_depends_on(strs(v.get("dependsOn")))
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
    Arg::default()
        .with_name(s(v, "name"))
        .with_description(s(v, "description"))
        .with_is_optional(b(v, "isOptional"))
        .with_is_variadic(b(v, "isVariadic"))
        .with_is_dangerous(b(v, "isDangerous"))
        .with_is_command(b(v, "isCommand"))
        .with_is_script(b(v, "isScript"))
        .with_is_module(s(v, "isModule"))
        .with_suggest_current_token(v.get("suggestCurrentToken").and_then(Value::as_bool))
        .with_debounce(match v.get("debounce") {
            Some(Value::Bool(true)) => Some(200),
            Some(n) => n.as_f64().filter(|&ms| ms > 0.0).map(|ms| ms as u64),
            None => None,
        })
        .with_options_can_break_variadic(
            v.get("optionsCanBreakVariadicArg").and_then(Value::as_bool),
        )
        .with_suggestions(
            items(v.get("suggestions"))
                .filter_map(suggestion_from_json)
                .collect(),
        )
        .with_generators(generators)
        .with_load_spec(v.get("loadSpec").and_then(load_spec))
        .with_filter_strategy(strategy(v))
        .with_alias_directive(
            v
                .get("parserDirectives")
                .and_then(|d| d.get("alias"))
                .and_then(str_or_fn),
        )
        .with_default(s(v, "default"))
}

fn generator(v: &Value) -> Generator {
    let template = strs(v.get("template"));
    // Path templates re-run when the directory part changes and query only the basename (SPEC §6.2, §6.6.4).
    let path_template = template.iter().any(|t| t == "filepaths" || t == "folders");
    Generator::default()
        .with_script(v.get("script").and_then(|x| match x {
            Value::Array(_) => Some(GeneratorScript::Argv(strs(Some(x)))),
            Value::String(c) => Some(GeneratorScript::Shell(c.clone())),
            Value::Object(_) => {
                Some(fn_id(x).map_or_else(|| GeneratorScript::Object(spec_value_from_json(x)), GeneratorScript::Fn))
            }
            _ => None,
        }))
        .with_post_process(v.get("postProcess").and_then(fn_id))
        .with_split_on(s(v, "splitOn"))
        .with_custom(v.get("custom").and_then(fn_id))
        .with_trigger(match v.get("trigger") {
            Some(Value::String(t)) => Some(GeneratorTrigger::Str(t.clone())),
            Some(t) if fn_id(t).is_some() => fn_id(t).map(GeneratorTrigger::Fn),
            Some(t) => match t.get("on").and_then(Value::as_str) {
                Some("change") => Some(GeneratorTrigger::Change),
                Some("threshold") => Some(GeneratorTrigger::Threshold(
                    t.get("length").and_then(Value::as_f64).unwrap_or(0.0) as usize,
                )),
                Some("match") => Some(GeneratorTrigger::Match(strs(t.get("string")))),
                _ => None,
            },
            None if path_template => Some(GeneratorTrigger::Str("/".into())),
            None => None,
        })
        .with_get_query_term(
            v.get("getQueryTerm")
                .and_then(str_or_fn)
                .or_else(|| path_template.then(|| TextOrFunction::Str("/".into()))),
        )
        .with_cache(v.get("cache").filter(|c| c.is_object()).map(|c| {
            GeneratorCachePolicy::default()
                .with_ttl_ms(c.get("ttl").and_then(Value::as_f64).unwrap_or(0.0) as u64)
                .with_max_age(c.get("strategy").and_then(Value::as_str) == Some("max-age"))
                .with_cache_by_directory(b(c, "cacheByDirectory"))
                .with_cache_key(s(c, "cacheKey"))
        }))
        .with_script_timeout(
            v.get("scriptTimeout")
                .and_then(Value::as_f64)
                .map(|ms| ms as u64),
        )
        .with_filter_template_suggestions(v.get("filterTemplateSuggestions").and_then(fn_id))
        .with_template(template)
}

/// String → one global location; `{"$fn"}` → function; other object → inline subcommand.
fn load_spec(v: &Value) -> Option<SpecLoadDirective> {
    match v {
        Value::String(name) => Some(SpecLoadDirective::Locations(vec![SpecLocation::Global {
            name: name.clone(),
        }])),
        Value::Object(_) => {
            Some(fn_id(v).map_or_else(|| SpecLoadDirective::Inline(Rc::new(subcommand(v))), SpecLoadDirective::Fn))
        }
        _ => None,
    }
}

fn str_or_fn(v: &Value) -> Option<TextOrFunction> {
    v.as_str()
        .map(|x| TextOrFunction::Str(x.into()))
        .or_else(|| fn_id(v).map(TextOrFunction::Fn))
}

fn strategy(v: &Value) -> FilterStrategy {
    match v.get("filterStrategy").and_then(Value::as_str) {
        Some("prefix") => FilterStrategy::Prefix,
        Some("fuzzy") => FilterStrategy::Fuzzy,
        _ => FilterStrategy::Default,
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
pub(crate) fn fn_id(v: &Value) -> Option<SpecFunctionRef> {
    v.get("$fn").and_then(Value::as_u64).map(|n| n as SpecFunctionRef)
}

fn fig_type(k: SuggestionKind) -> &'static str {
    match k {
        SuggestionKind::Subcommand => "subcommand",
        SuggestionKind::Option => "option",
        SuggestionKind::Arg => "arg",
        SuggestionKind::File => "file",
        SuggestionKind::Folder => "folder",
        SuggestionKind::Special => "special",
        SuggestionKind::Shortcut => "shortcut",
        SuggestionKind::History => "history",
        SuggestionKind::AutoExecute => "auto-execute",
    }
}

/// Template results as Fig suggestion objects (input of `filterTemplateSuggestions`).
pub(crate) fn template_json(items: &[SpecSuggestion]) -> Value {
    Value::Array(
        items
            .iter()
            .map(|s| {
                json!({"name": s.names().first(), "type": fig_type(s.kind().unwrap_or(SuggestionKind::Arg)),
                       "insertValue": s.insert_value(), "isDangerous": s.is_dangerous(),
                       "context": {"templateType": s.template_type()}})
            })
            .collect(),
    )
}

/// JS truthiness of a callback result.
pub(crate) fn truthy(v: &Value) -> bool {
    match v {
        Value::Null => false,
        Value::Bool(b) => *b,
        Value::Number(n) => n.as_f64().is_some_and(|f| f != 0.0 && !f.is_nan()),
        Value::String(s) => !s.is_empty(),
        _ => true,
    }
}

/// Script value → exec input JSON (SPEC §6.6.3); `None` for falsy values. A legacy string runs as `sh -c`, the
/// same form as `executeShellCommand(string)`, so the js host picks sh / Git Bash / `cmd /C` on Windows.
pub(crate) fn script_exec_input(v: Value, cwd: &str) -> Option<Value> {
    let cwd = json!(cwd);
    Some(match v {
        Value::Array(a) if !a.is_empty() => json!({"command": a[0], "args": a[1..], "cwd": cwd}),
        Value::String(s) if !s.is_empty() => {
            json!({"command": "sh", "args": ["-c", s], "cwd": cwd})
        } // none in 2.692.3
        Value::Object(o) => {
            let mut m = serde_json::Map::new();
            m.insert("cwd".into(), cwd);
            m.extend(o);
            Value::Object(m)
        }
        _ => return None,
    })
}

/// Suggestion objects (custom / filterTemplateSuggestions results).
pub(crate) fn json_items(v: &Value) -> Vec<SpecSuggestion> {
    v.as_array()
        .map(|a| {
            a.iter()
                .filter_map(suggestion_from_json)
                .collect()
        })
        .unwrap_or_default()
}

/// `postProcess` result: drop falsy items and objects without a name; strings become arg rows.
pub(crate) fn post_process_items(v: &Value, is_dangerous: bool) -> Vec<SpecSuggestion> {
    let Some(a) = v.as_array() else {
        return Vec::new();
    };
    a.iter()
        .filter_map(|x| match x {
            Value::String(s) if !s.is_empty() => {
                Some(SpecSuggestion::from_script_output(s, is_dangerous))
            }
            Value::Object(o) if o.get("name").is_some_and(truthy) => {
                suggestion_from_json(x)
            }
            _ => None,
        })
        .collect()
}

/// `generatorContext` for custom generators (SPEC §6.5): the env as is, `{}` when it is empty.
pub(crate) fn generator_context_json(context: &GeneratorContext) -> Value {
    json!({"currentWorkingDirectory": context.cwd(), "currentProcess": context.shell(), "sshPrefix": "",
           "searchTerm": context.search_term(), "environmentVariables": context.env(), "isDangerous": context.is_dangerous()})
}

/// A spec JSON value as a `SpecValue`, integers and floats kept apart.
pub(crate) fn spec_value_from_json(v: &Value) -> SpecValue {
    match v {
        Value::Null => SpecValue::Null,
        Value::Bool(b) => SpecValue::Bool(*b),
        Value::Number(n) => spec_number(n),
        Value::String(s) => SpecValue::Text(s.clone()),
        Value::Array(a) => SpecValue::List(a.iter().map(spec_value_from_json).collect()),
        Value::Object(o) => SpecValue::Object(
            o.iter()
                .map(|(k, v)| (k.clone(), spec_value_from_json(v)))
                .collect(),
        ),
    }
}

/// serde_json's `PosInt`, `NegInt` and `Float`, read in that order.
fn spec_number(n: &Number) -> SpecValue {
    n.as_u64()
        .map(SpecValue::Unsigned)
        .or_else(|| n.as_i64().map(SpecValue::Negative))
        .or_else(|| n.as_f64().map(SpecValue::Float))
        .expect("a serde_json number is an integer or a float")
}

/// A `SpecValue` as the serde_json value it was read from: integers stay integers, floats keep their bits.
pub(crate) fn spec_value_to_json(v: &SpecValue) -> Value {
    match v {
        SpecValue::Null => Value::Null,
        SpecValue::Bool(b) => Value::Bool(*b),
        SpecValue::Unsigned(n) => Value::from(*n),
        SpecValue::Negative(n) => Value::from(*n),
        SpecValue::Float(f) => {
            Value::Number(Number::from_f64(*f).expect("a float serde_json read is finite"))
        }
        SpecValue::Text(s) => Value::String(s.clone()),
        SpecValue::List(a) => Value::Array(a.iter().map(spec_value_to_json).collect()),
        SpecValue::Object(o) => Value::Object(
            o.iter()
                .map(|(k, v)| (k.clone(), spec_value_to_json(v)))
                .collect(),
        ),
    }
}

/// What a spec function returned, read the way its call's kind reads it.
pub(crate) fn function_result(
    kind: SpecFunctionKind,
    v: &Value,
    context: &GeneratorContext,
) -> SpecFunctionResult {
    match kind {
        SpecFunctionKind::QueryTerm => SpecFunctionResult::QueryTerm(match v {
            Value::String(s) => Some(s.clone()),
            _ => None,
        }),
        SpecFunctionKind::Trigger => SpecFunctionResult::Trigger(truthy(v)),
        SpecFunctionKind::Alias => SpecFunctionResult::Alias(v.as_str().map(String::from)),
        SpecFunctionKind::LoadSpec => SpecFunctionResult::LoadSpec(load_spec_result(v)),
        SpecFunctionKind::GenerateSpecCacheKey => {
            SpecFunctionResult::GenerateSpecCacheKey(v.as_str().map(String::from))
        }
        SpecFunctionKind::GenerateSpec => SpecFunctionResult::GenerateSpec(
            normalise(v).map(Box::new).map_err(|e| e.to_string()),
        ),
        SpecFunctionKind::Script => SpecFunctionResult::Script(script_resolution(v, context.cwd())),
        SpecFunctionKind::Custom => {
            SpecFunctionResult::Custom(CustomGeneratorResult::new(false, Vec::new()))
        }
        SpecFunctionKind::PostProcess => SpecFunctionResult::PostProcess(Vec::new()),
        SpecFunctionKind::FilterTemplateSuggestions => {
            SpecFunctionResult::FilterTemplateSuggestions(Vec::new())
        }
    }
}

/// A script value read as the engine's script_input reads it (SPEC §6.6.3), up to the timeout: the request asks
/// for the script's own timeout (0 when it has none), and the caller still raises it to the settings' and the
/// generator's. The cache key is built from the input before its timeout is set.
fn script_resolution(v: &Value, cwd: &str) -> ScriptResolution {
    let Some(mut input) = script_exec_input(v.clone(), cwd) else {
        return ScriptResolution::Skip;
    };
    let cache_key_source = input.to_string();
    let requested = input["timeout"].as_u64();
    input["timeout"] = json!(requested.unwrap_or(0));
    match serde_json::from_value::<ExecInput>(input) {
        Ok(exec) => ScriptResolution::Ready {
            request: ExecRequest::from(exec),
            cache_key_source,
        },
        Err(e) => ScriptResolution::Invalid(e.to_string()),
    }
}

#[cfg(test)]
mod tests;
