use super::*;
use crate::specs::domain::value_objects::spec_function_kind::SpecFunctionKind;
use crate::specs::domain::value_objects::spec_function_result::SpecFunctionResult;
use serde_json::json;

fn fixture(name: &str) -> Subcommand {
    let path = format!(
        "{}/tests/fixtures/parser/{name}.json",
        env!("CARGO_MANIFEST_DIR")
    );
    normalise(&serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()).unwrap()
}

fn sub<'a>(s: &'a Subcommand, name: &str) -> &'a Subcommand {
    &s.subcommands()[s.sub_index()[name]]
}

fn opt<'a>(s: &'a Subcommand, flag: &str) -> &'a SpecOption {
    match s.opt_index()[flag] {
        (true, i) => &s.persistent()[i],
        (false, i) => &s.options()[i],
    }
}

#[test]
fn normalise_sudo() {
    let s = fixture("sudo");
    assert_eq!(*s.names(), ["sudo"]);
    assert_eq!(s.options().len(), 3);
    assert_eq!(*opt(&s, "--group").names(), ["-g", "--group"]);
    assert!(std::ptr::eq(opt(&s, "-u"), opt(&s, "--user")));
    assert_eq!(opt(&s, "-g").args()[0].name().as_deref(), Some("group"));
    assert!(opt(&s, "-h").args().is_empty());
    assert_eq!(s.args().len(), 1);
    assert!(s.args()[0].is_command() && !s.args()[0].is_optional());
    assert_eq!(*s.directives(), ParserDirectives::default());
    assert!(!s.has_directives());
}

#[test]
fn normalise_cd_function_generator() {
    let s = fixture("cd");
    let a = &s.args()[0];
    assert_eq!(a.filter_strategy(), FilterStrategy::Fuzzy);
    let g = &a.generators()[0];
    assert_eq!(
        (g.trigger().clone(), g.get_query_term().clone(), g.custom()),
        (Some(GeneratorTrigger::Fn(0)), Some(TextOrFunction::Fn(1)), Some(2))
    );
    assert_eq!(
        a.suggestions()
            .iter()
            .map(|x| (x.names()[0].as_str(), x.hidden()))
            .collect::<Vec<_>>(),
        [("-", true), ("~", true)]
    );
}

#[test]
fn normalise_templates_and_persistent() {
    let s = fixture("tailcall");
    let check = sub(&s, "check");
    assert_eq!((check.persistent().len(), check.options().len()), (2, 1));
    assert_eq!(check.opt_index()["--schema"], (true, 1));
    assert_eq!(check.opt_index()["--format"], (false, 0));
    assert_eq!(opt(check, "--format").args()[0].suggestions().len(), 5);
    let g = &check.args()[0].generators()[0];
    assert_eq!(*g.template(), ["filepaths"]);
    assert_eq!(
        (g.trigger().clone(), g.get_query_term().clone()),
        (
            Some(GeneratorTrigger::Str("/".into())),
            Some(TextOrFunction::Str("/".into()))
        )
    );
    assert!(check.args()[0].is_variadic());
    assert_eq!(
        *sub(&s, "start").args()[0].generators()[0].template(),
        ["filepaths", "folders"]
    );
    assert_eq!(
        *sub(&s, "init").args()[0].generators()[0].trigger(),
        Some(GeneratorTrigger::Str("/".into()))
    );
}

#[test]
fn normalise_requires_separator() {
    let s = fixture("do-release-upgrade");
    assert_eq!(opt(&s, "--mode").requires_separator().as_deref(), Some(""));
    assert_eq!(*opt(&s, "-m").requires_separator(), None);
    assert!(opt(&s, "-m").hidden());
    assert_eq!(*opt(&s, "--mode").args()[0].suggestions()[1].names(), ["server"]);
}

#[test]
fn normalise_git_like() {
    let s = fixture("git_min");
    assert_eq!(s.generate_spec(), Some(7));
    assert_eq!(*s.args()[0].alias_directive(), Some(TextOrFunction::Fn(8)));
    assert!(s.has_directives());
    assert_eq!(*s.directives().option_arg_separators(), ["=", ":"]); // a string is a set of chars
    let short = &s.additional_suggestions()[0];
    assert_eq!(
        (short.kind(), short.insert_value().as_deref()),
        (Some(SuggestionKind::Shortcut), Some("commit -m '{cursor}'"))
    );
    assert_eq!(opt(&s, "-C").is_repeatable(), Repeatability::Unlimited);
    assert_eq!(opt(&s, "-c").is_repeatable(), Repeatability::Times(3));
    assert_eq!(opt(&s, "--version").is_repeatable(), Repeatability::No);
    assert!(opt(&s, "--no-pager").is_persistent());
    assert_eq!(s.opt_index()["--no-pager"], (true, 0));

    let commit = sub(&s, "ci");
    assert!(std::ptr::eq(commit, sub(&s, "commit")));
    assert_eq!(opt(commit, "--amend").is_repeatable(), Repeatability::Times(1)); // explicit false
    assert_eq!(*opt(commit, "--amend").exclusive_on(), ["--no-edit"]);
    assert_eq!(
        *opt(commit, "--pathspec-file-nul").depends_on(),
        ["--pathspec-from-file"]
    );
    assert_eq!(opt(commit, "-m").args()[0].debounce(), Some(200));
    let a = &commit.args()[0];
    assert_eq!(a.options_can_break_variadic(), Some(false));
    let g = &a.generators()[0];
    assert_eq!(
        *g.script(),
        Some(GeneratorScript::Argv(vec![
            "git".into(),
            "--no-optional-locks".into(),
            "status".into(),
            "--short".into()
        ]))
    );
    assert_eq!(g.post_process(), Some(9));
    assert_eq!(
        *g.cache(),
        Some(
            GeneratorCachePolicy::default()
                .with_ttl_ms(1000)
                .with_max_age(true)
                .with_cache_by_directory(true)
                .with_cache_key(None)
        )
    );

    let co = sub(&s, "checkout");
    let (g0, g1) = (&co.args()[0].generators()[0], &co.args()[0].generators()[1]);
    assert_eq!(
        (g0.script().clone(), g0.trigger().clone(), g0.script_timeout()),
        (
            Some(GeneratorScript::Fn(10)),
            Some(GeneratorTrigger::Threshold(2)),
            Some(15000)
        )
    );
    assert_eq!(*g0.get_query_term(), Some(TextOrFunction::Str("/".into())));
    assert_eq!(
        (
            g1.custom(),
            g1.trigger().clone(),
            g1.filter_template_suggestions()
        ),
        (Some(12), Some(GeneratorTrigger::Change), Some(13))
    );
    assert_eq!(co.args()[0].suggest_current_token(), Some(true));
    assert_eq!(*co.args()[1].generators()[0].template(), ["filepaths"]);

    assert!(
        matches!(sub(&s, "flow").load_spec(), Some(SpecLoadDirective::Locations(l)) if l == &[SpecLocation::Global { name: "git-flow".into() }])
    );
    let help = &sub(&s, "help").args()[0];
    assert_eq!(
        (help.default_value().as_deref(), help.generators()[0].trigger().clone()),
        (Some("git"), None)
    );
}

#[test]
fn normalise_mock_spec() {
    let s = fixture("cmd_spec");
    let man = sub(&s, "man");
    assert_eq!(
        (man.args().len(), man.options().len(), man.subcommands().len()),
        (2, 10, 4)
    );
    assert_eq!(opt(man, "--multiple").args().len(), 2);
    assert_eq!(
        opt(man, "--req-sep").requires_separator().as_deref(),
        Some(":")
    );
    assert!(opt(man, "--req-eq").requires_equals());
    assert_eq!(*sub(&s, "oas").directives().option_arg_separators(), ["=", ":"]);
    assert!(sub(&s, "pnf").directives().flags_posix_noncompliant());
    assert!(sub(&s, "ompa").directives().options_must_precede_args());
    assert_eq!(sub(&s, "generateSpec").generate_spec(), Some(1));
    assert_eq!(
        sub(&s, "module").args()[0].is_module().as_deref(),
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
        (s.sub_index()["a"], s.sub_index()["b"], s.subcommands().len()),
        (0, 1, 2)
    ); // later wins
    assert_eq!(s.opt_index()["-o"], (false, 0)); // options win over persistent
    assert_eq!((s.args().len(), s.args()[0].debounce()), (1, Some(50)));
    assert!(
        matches!(s.args()[0].load_spec(), Some(SpecLoadDirective::Inline(i)) if *i.names() == ["inner"] && i.args().len() == 1)
    );
    assert!(matches!(s.load_spec(), Some(SpecLoadDirective::Fn(4))));
}

#[test]
fn suggestions_and_load_spec_results() {
    assert_eq!(*suggestion_from_json(&json!("a")).unwrap().names(), ["a"]);
    let o = suggestion_from_json(&json!({"name": ["x", "y"], "type": "folder", "context": {"templateType": "folders"}, "priority": 80})).unwrap();
    assert_eq!(
        (
            o.names().len(),
            o.kind(),
            o.template_type().as_deref(),
            o.priority()
        ),
        (2, Some(SuggestionKind::Folder), Some("folders"), Some(80.0))
    );
    assert!(suggestion_from_json(&json!({"description": "no name"})).is_none());
    assert!(suggestion_from_json(&json!(3)).is_none());

    let locs = |v: Value| match load_spec_result(&v) {
        Some(SpecLoadDirective::Locations(l)) => l,
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
        Some(SpecLoadDirective::Inline(_))
    ));
    assert!(load_spec_result(&json!("str")).is_none());
}

#[test]
fn should_keep_a_script_object_exactly() {
    let script = json!({
        "command": "ls",
        "max": u64::MAX,
        "offset": -5,
        "tiny": 1.0715660391465826e-75,
        "env": {"HOME": null},
        "unknownKey": "kept"
    });
    let spec = json!({"name": "x", "args": {"generators": [{"script": script}, {"script": {"$fn": 7}}]}});

    let s = normalise(&spec).unwrap();

    let generators = s.args()[0].generators();
    let Some(GeneratorScript::Object(object)) = generators[0].script() else {
        panic!("not an object script: {:?}", generators[0].script());
    };
    let back = spec_value_to_json(object);
    assert_eq!(back, script);
    assert_eq!(back.to_string(), script.to_string());
    assert_eq!(*generators[1].script(), Some(GeneratorScript::Fn(7)));
}

#[test]
fn should_keep_the_lists_and_bools_of_a_script_object_exactly() {
    let script = json!({"command": "git", "args": ["branch", "-a", [1, null]], "on": true, "off": false});
    let spec = json!({"name": "x", "args": {"generators": [{"script": script}]}});

    let s = normalise(&spec).unwrap();

    let generators = s.args()[0].generators();
    let Some(GeneratorScript::Object(object)) = generators[0].script() else {
        panic!("not an object script: {:?}", generators[0].script());
    };
    let back = spec_value_to_json(object);
    assert_eq!(back, script);
    assert_eq!(back.to_string(), script.to_string());
}

#[test]
fn legacy_string_script_runs_through_sh_c() {
    // win-legacy-script-cmd: same `sh -c` form as `executeShellCommand(string)` on every platform
    assert_eq!(
        script_exec_input(json!("git log --format=\"%h %s\""), "/w"),
        Some(
            json!({"command": "sh", "args": ["-c", "git log --format=\"%h %s\""], "cwd": "/w"})
        )
    );
    assert_eq!(
        script_exec_input(json!(["git", "branch"]), "/w"),
        Some(json!({"command": "git", "args": ["branch"], "cwd": "/w"}))
    );
    assert_eq!(
        script_exec_input(json!({"command": "ls", "cwd": "/x"}), "/w"),
        Some(json!({"command": "ls", "cwd": "/x"}))
    );
    assert_eq!(script_exec_input(json!(""), "/w"), None);
    assert_eq!(script_exec_input(json!([]), "/w"), None);
}

#[test]
fn js_truthiness() {
    assert!(
        !truthy(&json!(null))
            && !truthy(&json!(false))
            && !truthy(&json!(0))
            && !truthy(&json!(""))
    );
    assert!(
        truthy(&json!(1)) && truthy(&json!("x")) && truthy(&json!([])) && truthy(&json!({}))
    );
}

#[test]
fn post_process_filtering() {
    // strings become arg rows carrying isDangerous; falsy entries and nameless objects are dropped
    let items =
        post_process_items(&json!(["a", "", null, 0, {"description": "no name"}]), true);
    assert_eq!(items.len(), 1);
    assert_eq!(*items[0].names(), ["a"]);
    assert_eq!(items[0].insert_value().as_deref(), Some("a"));
    assert!(items[0].is_dangerous());
    assert!(post_process_items(&json!(null), false).is_empty());
}

const EVERY_KIND: [SpecFunctionKind; 10] = [
    SpecFunctionKind::QueryTerm,
    SpecFunctionKind::Trigger,
    SpecFunctionKind::Alias,
    SpecFunctionKind::LoadSpec,
    SpecFunctionKind::GenerateSpecCacheKey,
    SpecFunctionKind::GenerateSpec,
    SpecFunctionKind::Script,
    SpecFunctionKind::Custom,
    SpecFunctionKind::PostProcess,
    SpecFunctionKind::FilterTemplateSuggestions,
];

fn kind_of(result: &SpecFunctionResult) -> SpecFunctionKind {
    match result {
        SpecFunctionResult::QueryTerm(_) => SpecFunctionKind::QueryTerm,
        SpecFunctionResult::Trigger(_) => SpecFunctionKind::Trigger,
        SpecFunctionResult::Alias(_) => SpecFunctionKind::Alias,
        SpecFunctionResult::LoadSpec(_) => SpecFunctionKind::LoadSpec,
        SpecFunctionResult::GenerateSpecCacheKey(_) => SpecFunctionKind::GenerateSpecCacheKey,
        SpecFunctionResult::GenerateSpec(_) => SpecFunctionKind::GenerateSpec,
        SpecFunctionResult::Script(_) => SpecFunctionKind::Script,
        SpecFunctionResult::Custom(_) => SpecFunctionKind::Custom,
        SpecFunctionResult::PostProcess(_) => SpecFunctionKind::PostProcess,
        SpecFunctionResult::FilterTemplateSuggestions(_) => {
            SpecFunctionKind::FilterTemplateSuggestions
        }
    }
}

/// P18: the parser reads a decoded result with let-else, which is safe only because every input of a kind reads
/// as that kind's variant.
#[test]
fn should_read_every_result_as_the_variant_of_its_call_kind() {
    let values = [
        json!(null),
        json!(false),
        json!(0),
        json!(1.5),
        json!(""),
        json!("checkout"),
        json!([]),
        json!([1]),
        json!(["git", "log"]),
        json!([{"name": "main", "context": null}]),
        json!({}),
        json!({"name": "npm", "subcommands": [{"name": "run"}]}),
        json!({"name": "npm", "type": "global"}),
        json!({"command": "ls", "cwd": null, "timeout": 1.5}),
        json!({"status": 0, "stdout": "main", "stderr": ""}),
    ];
    let context = GeneratorContext::default();
    let asked: Vec<SpecFunctionKind> = EVERY_KIND
        .iter()
        .flat_map(|kind| values.iter().map(|_| *kind))
        .collect();

    let read: Vec<SpecFunctionKind> = EVERY_KIND
        .iter()
        .flat_map(|kind| values.iter().map(|v| kind_of(&function_result(*kind, v, &context))))
        .collect();

    assert_eq!(read, asked);
}

#[test]
fn should_read_a_query_term_only_from_a_string() {
    let context = GeneratorContext::default();

    let (text, list) = (
        function_result(SpecFunctionKind::QueryTerm, &json!("ma"), &context),
        function_result(SpecFunctionKind::QueryTerm, &json!([]), &context),
    );

    assert!(matches!(text, SpecFunctionResult::QueryTerm(Some(term)) if term == "ma"));
    assert!(matches!(list, SpecFunctionResult::QueryTerm(None)));
}

#[test]
fn should_read_a_trigger_by_js_truthiness() {
    let context = GeneratorContext::default();
    let values = [json!("a"), json!(0), json!("")];

    let fired: Vec<bool> = values
        .iter()
        .map(|v| match function_result(SpecFunctionKind::Trigger, v, &context) {
            SpecFunctionResult::Trigger(fired) => fired,
            other => panic!("{other:?}"),
        })
        .collect();

    assert_eq!(fired, vec![true, false, false]);
}

#[test]
fn should_read_an_alias_only_from_a_string_and_keep_it_untrimmed() {
    let context = GeneratorContext::default();

    let (text, number) = (
        function_result(SpecFunctionKind::Alias, &json!("checkout\n"), &context),
        function_result(SpecFunctionKind::Alias, &json!(5), &context),
    );

    assert!(matches!(text, SpecFunctionResult::Alias(Some(alias)) if alias == "checkout\n"));
    assert!(matches!(number, SpecFunctionResult::Alias(None)));
}

#[test]
fn should_read_a_load_spec_result_as_its_directive() {
    let context = GeneratorContext::default();

    let (location, number) = (
        function_result(SpecFunctionKind::LoadSpec, &json!({"name": "npm", "type": "global"}), &context),
        function_result(SpecFunctionKind::LoadSpec, &json!(5), &context),
    );

    assert!(matches!(
        location,
        SpecFunctionResult::LoadSpec(Some(SpecLoadDirective::Locations(l)))
            if l == vec![SpecLocation::Global { name: "npm".into() }]
    ));
    assert!(matches!(number, SpecFunctionResult::LoadSpec(None)));
}
