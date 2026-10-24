//! Parser tests: parseArguments and loadSpec cases, loadSpec recursion, aliases, diffs.
use super::*;
use crate::settings::infrastructure::adapters::setting_value_json::settings_from_json;
use crate::shell_parser::Token;
use serde_json::json;
use std::cell::RefCell;

const S: u8 = 1;
const O: u8 = 2;
const A: u8 = 4;
const ANY: u8 = 7;
const NOT_S: u8 = O | A;

/// Whitespace tokenizer good enough for these buffers (trailing space → trailing empty token).
fn cmd(s: &str) -> CommandInfo {
    let mut tokens = Vec::new();
    let mut pos = 0;
    for w in s.split(' ') {
        if !w.is_empty() {
            tokens.push(Token {
                text: w.into(),
                raw: w.into(),
                start: pos,
                end: pos + w.len(),
            });
        }
        pos += w.len() + 1;
    }
    if s.ends_with(' ') {
        tokens.push(Token {
            text: String::new(),
            raw: String::new(),
            start: s.len(),
            end: s.len(),
        });
    }
    CommandInfo {
        tokens,
        text: s.into(),
        original_text: s.into(),
    }
}

fn spec(v: Value) -> Rc<Subcommand> {
    Rc::new(normalise(&v).unwrap())
}

fn fixture(name: &str) -> Rc<Subcommand> {
    let path = format!(
        "{}/tests/fixtures/parser/{name}.json",
        env!("CARGO_MANIFEST_DIR")
    );
    spec(serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap())
}

fn sub(s: &Rc<Subcommand>, name: &str) -> Rc<Subcommand> {
    s.subcommands[s.sub_index[name]].clone()
}

fn pos(s: &Rc<Subcommand>, i: usize) -> Option<Rc<Arg>> {
    Some(s.args[i].clone())
}

fn oarg(s: &Rc<Subcommand>, flag: &str) -> Option<Rc<Arg>> {
    let (_, i) = s.opt_index[flag];
    Some(s.options[i].args[0].clone())
}

fn flags(r: &ParseResult) -> u8 {
    r.suggest_subcommands as u8 | (r.suggest_options as u8) << 1 | (r.suggest_args as u8) << 2
}

fn ptr(a: &Option<Rc<Arg>>) -> Option<*const Arg> {
    a.as_ref().map(Rc::as_ptr)
}

type Callback = Box<dyn Fn(&[JsArg]) -> Result<Value, ParseError>>;

/// Specs by location (`name` for globals, `dir|name` for locals), JS functions by id; records every load.
#[derive(Default)]
struct Mock {
    specs: HashMap<String, Rc<Subcommand>>,
    fns: HashMap<FnId, Callback>,
    loads: Vec<SpecLocation>,
}

impl Mock {
    fn with(specs: &[(&str, &Rc<Subcommand>)]) -> Mock {
        Mock {
            specs: specs
                .iter()
                .map(|(k, v)| (k.to_string(), (*v).clone()))
                .collect(),
            ..Default::default()
        }
    }
}

impl Host for Mock {
    fn load(&mut self, loc: &SpecLocation) -> Result<Option<Rc<Subcommand>>, ParseError> {
        self.loads.push(loc.clone());
        let key = match loc {
            SpecLocation::Global { name } => name.clone(),
            SpecLocation::Local { dir, name } => format!("{}|{name}", dir.display()),
        };
        Ok(self.specs.get(&key).cloned())
    }

    fn call(&mut self, f: FnId, args: Vec<JsArg>) -> Result<Value, ParseError> {
        (self.fns.get(&f).ok_or_else(|| no("unknown fn"))?)(&args)
    }

    fn substitute(&mut self, c: &CommandInfo, index: usize, value: &str) -> Option<CommandInfo> {
        let mut words: Vec<String> = c.tokens.iter().map(|t| t.text.clone()).collect();
        words.splice(index..=index, value.split(' ').map(String::from));
        Some(cmd(&words.join(" ")))
    }
}

fn json_arg(args: &[JsArg], i: usize) -> Value {
    match &args[i] {
        JsArg::Json(v) => v.clone(),
        JsArg::Exec => panic!("expected json"),
    }
}

fn run(mock: &mut Mock, buf: &str) -> Result<ParseResult, ParseError> {
    ParseCaches::default().parse(&cmd(buf), Path::new("/cwd"), false, mock)
}

#[test]
fn parse_arguments_table() {
    let c = fixture("cmd_spec");
    let (man, oas, pnf, ompa, var, opt) = (
        sub(&c, "man"),
        sub(&c, "oas"),
        sub(&c, "pnf"),
        sub(&c, "ompa"),
        sub(&c, "var"),
        sub(&c, "opt"),
    );
    // (buffer, flags, completion, current_arg, search_term)
    type Row<'a> = (&'a str, u8, &'a Rc<Subcommand>, Option<Rc<Arg>>, &'a str);
    #[rustfmt::skip]
    let rows: Vec<Row> = vec![
        // basic subcommand / options
        ("cmd man ", ANY, &man, pos(&man, 0), ""),
        ("cmd man arg1 arg2 ", NOT_S, &man, None, ""),
        ("cmd -o ", ANY, &c, oarg(&c, "-o"), ""),
        ("cmd --opt ", ANY, &c, oarg(&c, "--opt"), ""),
        ("cmd pnf -pnfo ", ANY, &pnf, oarg(&pnf, "-pnfo"), ""),
        // end of options
        ("cmd man --", ANY, &man, pos(&man, 0), "--"),
        ("cmd man -- ", A, &man, pos(&man, 0), ""),
        ("cmd man -- -o ", A, &man, pos(&man, 1), ""),
        // invariant under most final tokens
        ("cmd man normal", ANY, &man, pos(&man, 0), "normal"),
        ("cmd man a", ANY, &man, pos(&man, 0), "a"),
        ("cmd man -o", ANY, &man, oarg(&man, "-o"), "-o"),
        // option with equals sign
        ("cmd man --opt=arg ", ANY, &man, pos(&man, 0), ""),
        ("cmd man --man=arg ", ANY, &man, pos(&man, 0), ""),
        ("cmd man --none=arg ", NOT_S, &man, pos(&man, 1), ""),
        ("cmd man --opt=arg", A, &man, oarg(&man, "--opt"), "arg"),
        ("cmd man --man=arg", A, &man, oarg(&man, "--man"), "arg"),
        ("cmd man --none=arg", ANY, &man, pos(&man, 0), "--none=arg"),
        // pnf options with equals sign
        ("cmd pnf -pnfo=arg ", ANY, &pnf, None, ""),
        ("cmd pnf -pnfm=arg ", ANY, &pnf, None, ""),
        ("cmd pnf -pnfo=arg", A, &pnf, oarg(&pnf, "-pnfo"), "arg"),
        ("cmd pnf -pnfm=arg", A, &pnf, oarg(&pnf, "-pnfm"), "arg"),
        // requiresEquals / requiresSeparator
        ("cmd man --req-eq=arg ", ANY, &man, pos(&man, 0), ""),
        ("cmd man --req-sep=arg ", ANY, &man, pos(&man, 0), ""),
        ("cmd man --req-eq=", A, &man, oarg(&man, "--req-eq"), ""),
        ("cmd man --req-sep=", A, &man, oarg(&man, "--req-sep"), ""),
        ("cmd man --req-eq ", ANY, &man, pos(&man, 0), ""),
        ("cmd oas --req-sep:arg ", ANY, &oas, pos(&oas, 0), ""),
        ("cmd oas --req-sep=arg ", ANY, &oas, pos(&oas, 0), ""),
        ("cmd oas --req-sep:", A, &oas, oarg(&oas, "--req-sep"), ""),
        ("cmd oas --req-sep=", A, &oas, oarg(&oas, "--req-sep"), ""),
        ("cmd oas --req-sep ", ANY, &oas, pos(&oas, 0), ""),
        // chained options
        ("cmd man -oarg", A, &man, oarg(&man, "-o"), "arg"),
        ("cmd man -o=arg", A, &man, oarg(&man, "-o"), "=arg"),
        ("cmd man -oarg ", ANY, &man, pos(&man, 0), ""),
        ("cmd man -marg", A, &man, oarg(&man, "-m"), "arg"),
        ("cmd man -m=arg", A, &man, oarg(&man, "-m"), "=arg"),
        ("cmd man -marg ", ANY, &man, pos(&man, 0), ""),
        ("cmd man -omarg", A, &man, oarg(&man, "-m"), "arg"),
        ("cmd man -moarg", A, &man, oarg(&man, "-m"), "oarg"),
        ("cmd man -nb", ANY, &man, pos(&man, 0), "-nb"),
        ("cmd man -on", ANY, &man, None, "-on"),
        ("cmd man --opt", ANY, &man, pos(&man, 0), "--opt"),
        ("cmd man --opt=", A, &man, oarg(&man, "-o"), ""),
        // passing args to options
        ("cmd -m arg ", ANY, &c, None, ""),
        ("cmd -o arg ", ANY, &c, None, ""),
        ("cmd --man arg ", ANY, &c, None, ""),
        ("cmd --opt arg ", ANY, &c, None, ""),
        ("cmd -v arg ", A | O, &c, oarg(&c, "-v"), ""),
        ("cmd -n ", ANY, &c, None, ""),
        // options after args
        ("cmd ompa argument ", A, &ompa, None, ""),
        ("cmd ompa -o ", ANY, &ompa, pos(&ompa, 0), ""),
        ("cmd ompa -m ", A, &ompa, oarg(&ompa, "-m"), ""),
        ("cmd ompa -m argument ", ANY, &ompa, pos(&ompa, 0), ""),
        // arg matrix
        ("cmd man -m ", A, &man, oarg(&man, "-m"), ""),
        ("cmd man -o ", ANY, &man, pos(&man, 0), ""),
        ("cmd man -v ", A, &man, oarg(&man, "-v"), ""),
        ("cmd man -n ", ANY, &man, pos(&man, 0), ""),
        ("cmd var -m ", A, &var, oarg(&var, "-m"), ""),
        ("cmd var -o ", ANY, &var, pos(&var, 0), ""),
        ("cmd var -v ", A, &var, oarg(&var, "-v"), ""),
        ("cmd var -n ", ANY, &var, pos(&var, 0), ""),
        ("cmd var -marg", A, &var, oarg(&var, "-m"), "arg"),
        ("cmd var -oarg", A, &var, oarg(&var, "-o"), "arg"),
        ("cmd opt -m ", A, &opt, oarg(&opt, "-m"), ""),
        ("cmd opt -o ", ANY, &opt, pos(&opt, 0), ""),
        ("cmd opt -v ", A, &opt, oarg(&opt, "-v"), ""),
        ("cmd opt -n ", ANY, &opt, pos(&opt, 0), ""),
        ("cmd opt -marg", A, &opt, oarg(&opt, "-m"), "arg"),
        ("cmd opt -oarg", A, &opt, oarg(&opt, "-o"), "arg"),
        ("cmd -m ", A, &c, oarg(&c, "-m"), ""),
        ("cmd -v ", A, &c, oarg(&c, "-v"), ""),
        ("cmd -marg", A, &c, oarg(&c, "-m"), "arg"),
        ("cmd -oarg", A, &c, oarg(&c, "-o"), "arg"),
    ];
    for (buf, fl, comp, arg, term) in rows {
        let r =
            run(&mut Mock::with(&[("cmd", &c)]), buf).unwrap_or_else(|e| panic!("{buf:?}: {e:?}"));
        assert_eq!((flags(&r), r.search_term.as_str()), (fl, term), "{buf:?}");
        assert!(
            Rc::ptr_eq(&r.completion, comp),
            "{buf:?}: completion {:?}",
            r.completion.names
        );
        assert_eq!(
            ptr(&r.current_arg),
            ptr(&arg),
            "{buf:?}: current_arg {:?}",
            r.current_arg
        );
    }
}

#[test]
fn result_fields() {
    let c = fixture("cmd_spec");
    let mut m = Mock::with(&[("cmd", &c)]);
    let r = run(&mut m, "cmd man -on").unwrap();
    assert_eq!(r.option_chain.as_deref(), Some("-on"));
    assert_eq!(r.passed_options.len(), 1); // `-o`; the letter being typed stays suggestable
    assert_eq!(
        r.annotations.last().unwrap().parts,
        [
            (AnnKind::Option, "-o".into()),
            (AnnKind::Option, "n".into())
        ]
    );
    assert_eq!(run(&mut m, "cmd man -oarg").unwrap().option_chain, None);
    let r = run(&mut m, "cmd man -m x ").unwrap();
    assert_eq!(
        r.path
            .iter()
            .map(|s| s.names[0].as_str())
            .collect::<Vec<_>>(),
        ["cmd", "man"]
    );
    assert_eq!(
        (r.spec_name.as_str(), r.command_index, r.first_token),
        ("cmd", 0, false)
    );
    assert_eq!(r.tokens, ["cmd", "man", "-m", "x", ""]);
    assert_eq!(r.passed_options[0].names, ["-m", "--man"]);
    let kinds: Vec<AnnKind> = r.annotations.iter().map(|a| a.kind).collect();
    assert_eq!(
        kinds,
        [
            AnnKind::Subcommand,
            AnnKind::Subcommand,
            AnnKind::Option,
            AnnKind::OptionArg,
            AnnKind::SubcommandArg
        ]
    );
}

#[test]
fn exceptions_and_first_token() {
    let c = fixture("cmd_spec");
    let mut m = Mock::with(&[("cmd", &c)]);
    assert!(run(&mut m, "").is_err());
    let r = run(&mut m, "cmd").unwrap();
    assert!(r.first_token && r.current_arg.is_none() && m.loads.is_empty());
    assert_eq!(
        (r.search_term.as_str(), r.raw_last_token.as_str()),
        ("cmd", "cmd")
    );
    assert!(matches!(
        run(&mut m, "cmd notASubcommand "),
        Err(ParseError::NoSuggestions(_))
    ));
    assert!(run(&mut m, "unknown x").is_err());
}

#[test]
fn single_token_paths() {
    let dotslash =
        spec(json!({"name": "dotslash", "args": {"template": ["filepaths", "folders"]}}));
    let mut m = Mock::with(&[("dotslash", &dotslash), ("php/bin-console", &dotslash)]);
    let r = run(&mut m, "./test").unwrap();
    assert!(!r.first_token && Rc::ptr_eq(&r.completion, &dotslash));
    assert_eq!(ptr(&r.current_arg), ptr(&pos(&dotslash, 0)));
    run(&mut m, "bin/console").unwrap();
    assert_eq!(
        m.loads,
        [
            SpecLocation::Global {
                name: "dotslash".into()
            },
            SpecLocation::Global {
                name: "php/bin-console".into()
            }
        ]
    );
}

#[test]
fn relative_path_first_token_loads_local_spec() {
    let mut m = Mock::default();
    assert!(run(&mut m, "./dir/test ").is_err());
    assert_eq!(
        m.loads,
        [SpecLocation::Local {
            dir: "/cwd/dir/".into(),
            name: "test".into()
        }]
    );
}

#[test]
fn load_spec_and_is_module() {
    let c = fixture("cmd_spec");
    let to_load = spec(json!({"name": "specToLoad", "args": [{}]}));
    for (buf, name) in [
        ("cmd loadSpec ", "loadSpecName"),
        ("cmd module moduleName ", "python/moduleName"),
    ] {
        let mut m = Mock::with(&[("cmd", &c), (name, &to_load)]);
        let r = run(&mut m, buf).unwrap();
        assert_eq!(m.loads.len(), 2);
        assert_eq!(m.loads[1], SpecLocation::Global { name: name.into() });
        assert_eq!((flags(&r), r.search_term.as_str()), (ANY, ""));
        assert!(Rc::ptr_eq(&r.completion, &to_load));
        assert_eq!(ptr(&r.current_arg), ptr(&pos(&to_load, 0)));
    }
}

#[test]
fn recursive_load_spec() {
    let c = fixture("cmd_spec");
    let (nested, nested2) = (
        sub(&c, "recursiveLoadSpecNested"),
        sub(&c, "recursiveLoadSpecNestedNested"),
    );
    let specs = [
        ("cmd", &c),
        ("recursiveLoadSpecNested", &nested),
        ("recursiveLoadSpecNestedNested", &nested2),
    ];

    let mut m = Mock::with(&specs);
    let r = run(&mut m, "cmd recursiveLoadSpec nestedSubcommand ").unwrap();
    assert_eq!(
        (
            m.loads.len(),
            flags(&r),
            r.annotations.len(),
            r.command_index
        ),
        (3, ANY, 2, 2)
    );
    assert!(Rc::ptr_eq(&r.completion, &nested2));
    assert_eq!(r.spec_name, "recursiveLoadSpecNestedNested");

    let mut m = Mock::with(&specs);
    let r = run(
        &mut m,
        "cmd recursiveLoadSpec nestedSubcommand doubleNestedSubcommand ",
    )
    .unwrap();
    let double = sub(&nested2, "doubleNestedSubcommand");
    assert_eq!((m.loads.len(), flags(&r), r.annotations.len()), (3, ANY, 3));
    assert!(Rc::ptr_eq(&r.completion, &double));
    assert_eq!(ptr(&r.current_arg), ptr(&pos(&double, 0)));
}

#[test]
fn is_command_recursion() {
    let c = fixture("cmd_spec");
    let sudo = sub(&c, "sudo");
    let mut m = Mock::with(&[("cmd", &c), ("sudo", &sudo)]);
    let r = run(&mut m, "cmd sudo ").unwrap();
    assert_eq!((m.loads.len(), flags(&r), r.annotations.len()), (1, ANY, 3));
    assert!(Rc::ptr_eq(&r.completion, &sudo));

    let mut m = Mock::with(&[("cmd", &c), ("sudo", &sudo)]);
    let r = run(&mut m, "cmd sudo sudo sudo sudo ").unwrap();
    assert_eq!(
        (
            m.loads.len(),
            flags(&r),
            r.annotations.len(),
            r.command_index
        ),
        (4, ANY, 2, 4)
    );
    assert_eq!(r.search_term, "");
    assert!(Rc::ptr_eq(&r.completion, &sudo));
    assert_eq!(ptr(&r.current_arg), ptr(&pos(&sudo, 0)));
}

#[test]
fn sudo_git_command_index() {
    let (sudo, git) = (fixture("sudo"), fixture("git_min"));
    let mut m = Mock::with(&[("sudo", &sudo), ("git", &git)]);
    m.fns.insert(7, Box::new(|_| Ok(json!({"name": "git"}))));
    let r = run(&mut m, "sudo -u root git ch").unwrap();
    assert_eq!(
        (
            r.command_index,
            r.spec_name.as_str(),
            r.search_term.as_str()
        ),
        (3, "git", "ch")
    );
    assert_eq!(r.tokens[r.command_index..], ["git", "ch"]);
    assert!(r.completion.names == ["git"] && r.suggest_subcommands); // a generateSpec merge makes a new level
}

#[test]
fn generate_spec_merge() {
    let c = fixture("cmd_spec");
    let calls = Rc::new(RefCell::new(Vec::new()));
    let mut m = Mock::with(&[("cmd", &c)]);
    let seen = calls.clone();
    m.fns.insert(
        1,
        Box::new(move |args| {
            let tokens = json_arg(args, 0);
            seen.borrow_mut().push(tokens.clone());
            if tokens.as_array().unwrap().iter().any(|t| t.as_str().unwrap().starts_with('-')) {
                return Err(no("Error in generateSpec"));
            }
            let subs: Vec<Value> = ["these", "are", "generated", "subcommands"]
                .iter()
                .map(|n| json!({"name": n, "args": {"name": "string", "suggestions": ["generated", "arg", "suggestions"]}}))
                .collect();
            Ok(json!({"name": "generateSpec", "description": "Fake generated spec", "subcommands": subs, "args": {}, "options": [{"name": "--genOpt"}]}))
        }),
    );
    let r = run(&mut m, "cmd generateSpec ").unwrap();
    assert_eq!(*calls.borrow(), [json!(["cmd", "generateSpec", ""])]);
    assert_eq!((flags(&r), r.search_term.as_str()), (ANY, ""));
    let comp = &r.completion;
    assert_eq!(comp.names, ["generateSpec"]);
    assert!(["these", "are", "generated", "subcommands"]
        .iter()
        .all(|n| comp.sub_index.contains_key(*n)));
    assert!(comp.opt_index.contains_key("--anOpt") && comp.opt_index.contains_key("--genOpt"));
    assert_eq!(comp.args.len(), 1);
    assert_eq!(ptr(&r.current_arg), ptr(&pos(comp, 0)));
    assert!(Rc::ptr_eq(r.path.last().unwrap(), comp));

    // generateSpec throws → state unchanged
    let r = run(&mut m, "cmd -mmsg generateSpec ").unwrap();
    assert_eq!((flags(&r), r.search_term.as_str()), (ANY, ""));
    assert!(Rc::ptr_eq(&r.completion, &sub(&c, "generateSpec")));
    assert!(r.current_arg.is_none());
}

#[test]
fn generate_spec_cache_key() {
    let root =
        spec(json!({"name": "g", "generateSpec": {"$fn": 1}, "generateSpecCacheKey": {"$fn": 2}}));
    let count = Rc::new(RefCell::new(0));
    let mut m = Mock::with(&[("g", &root)]);
    let n = count.clone();
    m.fns.insert(
        1,
        Box::new(move |_| {
            *n.borrow_mut() += 1;
            Ok(json!({"name": "g", "subcommands": [{"name": "gen"}]}))
        }),
    );
    m.fns.insert(
        2,
        Box::new(|args| {
            assert_eq!(json_arg(args, 0)["tokens"][0], "g");
            Ok(json!("k"))
        }),
    );
    let mut caches = ParseCaches::default();
    for buf in ["g gen x", "g gen y", "g other "] {
        let _ = caches.parse(&cmd(buf), Path::new("/cwd"), false, &mut m);
    }
    assert_eq!(*count.borrow(), 1);
    assert!(caches.generated.contains_key("g:k"));
}

#[test]
fn persistent_options_inherit_without_mutation() {
    let p = spec(json!({
        "name": "p",
        "options": [{"name": "--verbose", "isPersistent": true}, {"name": "-x", "isPersistent": true, "args": {"name": "parent-x"}}, {"name": "--root-only"}],
        "subcommands": [{"name": "sub", "options": [{"name": "-x", "isPersistent": true}, {"name": "--bar"}],
                         "subcommands": [{"name": "leaf", "args": {"name": "leafarg"}}]}]
    }));
    let mut m = Mock::with(&[("p", &p)]);
    let r = run(&mut m, "p sub --verbose ").unwrap();
    assert_eq!(r.passed_options[0].names, ["--verbose"]);
    let r = run(&mut m, "p sub -x ").unwrap();
    assert_eq!(r.current_arg.unwrap().name.as_deref(), Some("parent-x")); // parent wins
    assert!(run(&mut m, "p sub --root-only ").is_err());
    let r = run(&mut m, "p sub leaf -x ").unwrap();
    assert_eq!(r.current_arg.unwrap().name.as_deref(), Some("parent-x"));
    assert_eq!(r.persistent.len(), 3);
    assert_eq!(sub(&p, "sub").persistent.len(), 1);
    let r = run(&mut m, "p sub leaf --verbose ").unwrap();
    assert_eq!(r.current_arg.unwrap().name.as_deref(), Some("leafarg"));
}

#[test]
fn is_repeatable() {
    let x = spec(json!({
        "name": "x",
        "options": [{"name": "-c", "isRepeatable": 2, "args": {}}, {"name": "-f", "isRepeatable": false},
                    {"name": "-v", "isRepeatable": true}, {"name": "-q"}],
        "args": {"name": "file", "isOptional": true, "isVariadic": true}
    }));
    let mut m = Mock::with(&[("x", &x)]);
    let kinds = |m: &mut Mock, buf: &str| {
        run(m, buf)
            .unwrap()
            .annotations
            .iter()
            .map(|a| a.kind)
            .collect::<Vec<_>>()
    };
    use AnnKind::{Option as Op, OptionArg as OA, Subcommand as Sc, SubcommandArg as SA};
    assert_eq!(kinds(&mut m, "x -f -f"), [Sc, Op, SA]);
    assert_eq!(kinds(&mut m, "x -v -v -v"), [Sc, Op, Op, Op]);
    assert_eq!(kinds(&mut m, "x -q -q"), [Sc, Op, Op]); // absent: unlimited in the parser
    assert_eq!(kinds(&mut m, "x -c 1 -c 2 -c"), [Sc, Op, OA, Op, OA, SA]);
}

#[test]
fn options_can_break_variadic() {
    let x = spec(json!({
        "name": "x", "options": [{"name": "-n"}, {"name": "-l", "args": {"isVariadic": true, "optionsCanBreakVariadicArg": false}}],
        "args": {"name": "files", "isVariadic": true, "optionsCanBreakVariadicArg": false}
    }));
    let mut m = Mock::with(&[("x", &x)]);
    assert_eq!(flags(&run(&mut m, "x a ").unwrap()), A); // in the variadic positional: no options
    assert_eq!(flags(&run(&mut m, "x -n ").unwrap()), ANY);
    assert_eq!(flags(&run(&mut m, "x -l a ").unwrap()), A);
}

#[test]
fn inline_and_function_load_specs() {
    let y = spec(json!({
        "name": "y",
        "subcommands": [
            {"name": "run", "loadSpec": {"$fn": 3}},
            {"name": "inl", "loadSpec": {"name": "inner", "args": {"name": "a"}, "parserDirectives": {"optionsMustPrecedeArguments": true}}}
        ]
    }));
    let created = spec(json!({"name": "create-run", "args": {"name": "target"}}));
    let mut m = Mock::with(&[("y", &y), ("create-run", &created)]);
    m.fns.insert(3, Box::new(|args| Ok(json!({"name": format!("create-{}", json_arg(args, 0).as_str().unwrap()), "type": "global"}))));
    let r = run(&mut m, "y run foo ").unwrap();
    assert!(Rc::ptr_eq(&r.completion, &created));
    assert_eq!((r.command_index, r.annotations.len()), (1, 3));
    assert!(r.current_arg.is_none()); // `foo` took the only arg

    let r = run(&mut m, "y inl ").unwrap();
    assert_eq!(r.completion.names, ["inner"]);
    assert_eq!(r.current_arg.unwrap().name.as_deref(), Some("a"));
    assert!(r.directives.options_must_precede_args);
}

#[test]
fn git_alias_directive() {
    let git = fixture("git_min");
    let mut m = Mock::with(&[("git", &git)]);
    m.fns.insert(7, Box::new(|_| Ok(json!({"name": "git"}))));
    m.fns.insert(
        8,
        Box::new(|args| match json_arg(args, 0).as_str() {
            Some("co") => Ok(json!("checkout\n")),
            Some("cm") => Ok(json!("commit -m")),
            _ => Err(no("no alias")),
        }),
    );
    let r = run(&mut m, "git co ").unwrap();
    assert_eq!(r.tokens, ["git", "checkout", ""]);
    assert!(Rc::ptr_eq(&r.completion, &sub(&git, "checkout")));
    assert_eq!(
        r.current_arg.unwrap().name.as_deref(),
        Some("branch, file, tag or commit")
    );

    let r = run(&mut m, "git cm msg").unwrap();
    assert_eq!(r.tokens, ["git", "commit", "-m", "msg"]);
    assert_eq!((r.search_term.as_str(), r.suggest_options), ("msg", false));
    assert!(run(&mut m, "git zz ").is_err()); // unknown alias: nothing substituted yet → error
}

#[test]
fn prefix_cache() {
    let c = fixture("cmd_spec");
    let mut m = Mock::with(&[("cmd", &c)]);
    let mut caches = ParseCaches::default();
    let cwd = Path::new("/cwd");
    caches
        .parse(&cmd("cmd man -m x"), cwd, false, &mut m)
        .unwrap();
    let r = caches
        .parse(&cmd("cmd man -m xyz"), cwd, false, &mut m)
        .unwrap();
    assert_eq!(m.loads.len(), 1);
    assert_eq!(
        (r.search_term.as_str(), r.tokens.last().unwrap().as_str()),
        ("xyz", "xyz")
    );
    caches
        .parse(&cmd("cmd man -m xyz"), Path::new("/other"), false, &mut m)
        .unwrap();
    caches
        .parse(&cmd("cmd man -m xyz"), cwd, true, &mut m)
        .unwrap(); // dev mode: no cache
    assert_eq!(m.loads.len(), 3);
}

#[test]
fn load_spec_cycle_is_an_error() {
    let a = spec(json!({"name": "a", "loadSpec": "b"}));
    let b = spec(json!({"name": "b", "loadSpec": "a"}));
    assert!(run(&mut Mock::with(&[("a", &a), ("b", &b)]), "a x").is_err());
}

#[test]
fn spec_location_table() {
    let cwd = Path::new("test_cwd");
    let g = |n: &str| SpecLocation::Global { name: n.into() };
    let l = |d: &str, n: &str| SpecLocation::Local {
        dir: d.into(),
        name: n.into(),
    };
    assert_eq!(spec_location("git", cwd, None), g("git"));
    assert_eq!(
        spec_location("@withfig/autocomplete-tools", cwd, Some(false)),
        g("@withfig/autocomplete-tools")
    );
    assert_eq!(
        spec_location("@withfig/autocomplete-tools", cwd, None),
        l("test_cwd/@withfig/", "autocomplete-tools")
    );
    assert_eq!(spec_location("./test", cwd, None), l("test_cwd/", "test"));
    assert_eq!(spec_location("~/test", cwd, None), l("~/", "test"));
    assert_eq!(spec_location("/test", cwd, None), l("/", "test"));
    assert_eq!(spec_location("/dir/test", cwd, None), l("/dir/", "test"));
    assert_eq!(spec_location("~/dir/test", cwd, None), l("~/dir/", "test"));
    assert_eq!(
        spec_location("./dir/test", cwd, None),
        l("test_cwd/dir/", "test")
    );
    assert_eq!(
        spec_location("bin/console", cwd, None),
        g("php/bin-console")
    );
    assert_eq!(
        spec_location("vendor/bin/console", cwd, None),
        g("php/bin-console")
    );
    assert_eq!(
        spec_location("./run.sh", cwd, Some(false)),
        l("test_cwd/", "run.sh")
    );
    assert_eq!(
        spec_location("script", cwd, Some(true)),
        l("test_cwd", "script")
    );
}

#[test]
fn diff_merge() {
    let base = json!({
        "name": "h", "description": "old",
        "subcommands": [{"name": "a", "description": "A"}, {"name": ["b", "bb"], "description": "B"}, {"name": "c"}],
        "options": [{"name": "--x", "args": [{"name": "x1"}, {"name": "x2"}]}, {"name": "--y"}],
        "args": [{"name": "p0", "description": "d0"}, {"name": "p1", "description": "d1"}]
    });
    let diff = json!({
        "description": "new",
        "subcommands": [{"name": "bb", "description": "B2"}, {"name": "new"}, {"name": "c", "remove": true}],
        "options": [{"name": "--x", "args": [{"name": "gone", "remove": true}, {"description": "second"}]}],
        "args": [{"remove": true}, {"description": "z"}]
    });
    let out = merge_subcommand(&base, &diff);
    assert_eq!(out["description"], "new");
    // diff items first (merged, new), then untouched base items; `remove` drops the match
    assert_eq!(
        out["subcommands"],
        json!([{"name": "bb", "description": "B2"}, {"name": "new"}, {"name": "a", "description": "A"}])
    );
    // merge_args indexes after filtering: "second" lands on x1, "z" on p0 (quirk kept)
    assert_eq!(
        out["options"],
        json!([{"name": "--x", "args": [{"name": "x1", "description": "second"}]}, {"name": "--y"}])
    );
    assert_eq!(out["args"], json!([{"name": "p0", "description": "z"}]));
    assert_eq!(merge_subcommand(&base, &Value::Null), base);
    assert_eq!(
        merge_subcommand(&base, &json!({"subcommands": []}))["subcommands"],
        base["subcommands"]
    );
}

#[test]
fn version_pick() {
    let base = json!({"name": "h", "description": "base"});
    let diffs = json!({
        "9.0.0": {"description": "nine"},
        "8.10.0": {"description": "eight-ten"},
        "8.2.0": {"description": "eight-two", "subcommands": [{"name": "x"}]}
    });
    let diffs = diffs.as_object().unwrap();
    let desc = |t: Option<&str>| {
        apply_versions(base.clone(), diffs, t)["description"]
            .as_str()
            .unwrap()
            .to_string()
    };
    assert_eq!(desc(Some("8.10.0")), "eight-ten"); // semver order, not string order
    assert_eq!(desc(Some("8.5.0")), "eight-two");
    assert_eq!(desc(None), "nine");
    assert_eq!(desc(Some("1.0.0")), "base");
    assert_eq!(desc(Some("garbage")), "nine");
    assert_eq!(
        apply_versions(base.clone(), diffs, Some("8.10.0"))["subcommands"],
        json!([{"name": "x"}])
    );
}

#[test]
fn semver_helpers() {
    assert_eq!(semver_clean("1.2.3\n").as_deref(), Some("1.2.3"));
    assert_eq!(semver_clean(" =v1.2.3 ").as_deref(), Some("1.2.3"));
    assert_eq!(
        semver_clean("1.2.3-beta.1+build.5").as_deref(),
        Some("1.2.3-beta.1")
    );
    for bad in [
        "heroku/8.1.0 darwin-x64",
        "1.2",
        "01.2.3",
        "1.2.3.4",
        "",
        "git version 2.39.0",
    ] {
        assert_eq!(semver_clean(bad), None, "{bad:?}");
    }
    assert_eq!(semver_cmp("1.10.0", "1.9.0"), Ordering::Greater);
    assert_eq!(semver_cmp("1.0.0-alpha", "1.0.0"), Ordering::Less);
    assert_eq!(
        semver_cmp("1.0.0-alpha.1", "1.0.0-alpha.beta"),
        Ordering::Less
    );
    assert_eq!(semver_cmp("1.0.0-2", "1.0.0-10"), Ordering::Less);
    assert_eq!(semver_cmp("1.0.0-alpha", "1.0.0-alpha.1"), Ordering::Less);
    assert_eq!(semver_cmp("2.53.0", "2.53.0"), Ordering::Equal);
}

#[test]
fn cached_nested_state_keeps_this_commands_prefix() {
    let wrapper =
        |n: &str| spec(json!({"name": n, "args": {"name": "command", "isCommand": true}}));
    let (sudo, doas, inner) = (
        wrapper("sudo"),
        wrapper("doas"),
        spec(json!({"name": "inner", "subcommands": [{"name": "ch"}]})),
    );
    let mut m = Mock::with(&[("sudo", &sudo), ("doas", &doas), ("inner", &inner)]);
    let mut caches = ParseCaches::default();
    caches
        .parse(&cmd("sudo inner c"), Path::new("/cwd"), false, &mut m)
        .unwrap();
    let r = caches
        .parse(&cmd("doas inner c"), Path::new("/cwd"), false, &mut m)
        .unwrap();
    assert_eq!(
        (r.tokens.join(" "), r.command_index),
        ("doas inner c".to_string(), 1)
    );
    assert_eq!(m.loads.len(), 3); // sudo, inner, doas: the nested prefix came from the cache
}

#[test]
fn composite_option_value_raw_is_the_value_only() {
    // composite-optarg-delete: completing `--opt=al` must replace `al`, not the whole token
    let c = fixture("cmd_spec");
    let mut m = Mock::with(&[("cmd", &c)]);
    for (buf, raw) in [
        ("cmd man --opt=al", "al"),
        ("cmd man --req-eq=", ""),
        ("cmd man -oal", "al"),
        ("cmd man -omarg", "arg"),
        ("cmd man -o=arg", "=arg"),
        ("cmd pnf -pnfo=arg", "arg"),
        ("cmd man --opt", "--opt"), // not composite: the whole token
        ("cmd man -on", "-on"),     // option chain without a value
    ] {
        assert_eq!(run(&mut m, buf).unwrap().raw_last_token, raw, "{buf:?}");
    }
}

#[test]
fn first_token_spec_identity_is_stable() {
    // first-token-identity-churn: one Rc for every first-token parse, so the engine's arg key does not change
    let mut m = Mock::default();
    let mut caches = ParseCaches::default();
    let a = caches
        .parse(&cmd("g"), Path::new("/cwd"), false, &mut m)
        .unwrap();
    let b = caches
        .parse(&cmd("gi"), Path::new("/cwd"), false, &mut m)
        .unwrap();
    assert!(a.first_token && Rc::ptr_eq(&a.completion, &b.completion));
}

#[test]
fn cli_version_detection_survives_an_aborted_parse() {
    // cli-version-respawn-per-keystroke: an abort keeps the detection running; the next parse reuses it
    use crate::types::{EngineMsg, Request};
    let (tx, rx) = std::sync::mpsc::channel();
    let mut js = Js::new(rx, tx.clone(), Settings::default()).unwrap();
    let dir = crate::shared::infrastructure::testing::temp_dir::TempDir::new("cliver");
    let p = dir.path().join("v.js");
    let src = "var v=async()=>{await new Promise(r=>setTimeout(r,200));globalThis.n=(globalThis.n||0)+1;return `${globalThis.n}.0.0`};export{v as getVersionCommand};";
    std::fs::write(&p, src).unwrap();
    let exports = js.load_module(&p).unwrap();
    let (generator, mut inflight) = (GenCtx::default(), HashMap::new());
    let req = Request {
        id: 1,
        buffer: "az ".into(),
        cursor: 3,
        cwd: "/".into(),
        shell: Shell::Zsh,
        history_mode: false,
    };
    tx.send(EngineMsg::Complete(req)).unwrap();
    assert!(matches!(
        cli_version(&exports, "az", &mut js, &generator, &mut inflight),
        Err(ParseError::Aborted)
    ));
    assert_eq!(inflight.len(), 1);
    assert!(matches!(js.next_msg(), Some(EngineMsg::Complete(_))));
    assert_eq!(
        cli_version(&exports, "az", &mut js, &generator, &mut inflight)
            .unwrap()
            .as_deref(),
        Some("1.0.0")
    );
    assert!(inflight.is_empty());
}

#[test]
fn spec_identity_survives_dev_reload_and_cache_clear() {
    // arg-key-pointer-identity-devmode: an unchanged spec keeps its Rc (the engine's arg/generator keys)
    let (tx, rx) = std::sync::mpsc::channel();
    let mut js = Js::new(rx, tx, Settings::default()).unwrap();
    let dir = crate::shared::infrastructure::testing::temp_dir::TempDir::new("devid");
    let build = dir.path().join(".fig").join("autocomplete").join("build");
    std::fs::create_dir_all(&build).unwrap();
    std::fs::write(
        build.join("mk.js"),
        r#"var s={name:"mk",args:{name:"t"}};export{s as default};"#,
    )
    .unwrap();
    let settings = settings_from_json(
        json!({"autocomplete.developerMode": true})
            .as_object()
            .unwrap()
            .clone(),
    );
    let (env, index) = (HashMap::new(), SpecIndex::default());
    let ctx = ParseCtx {
        cwd: dir.path(),
        settings: &settings,
        env: &env,
        shell: Shell::Zsh,
        index: &index,
    };
    let loc = SpecLocation::Local {
        dir: dir.path().to_path_buf(),
        name: "mk".into(),
    };
    let mut p = Parser::new();
    let a = p.load_spec(&loc, &ctx, &mut js).unwrap().unwrap();
    let b = p.load_spec(&loc, &ctx, &mut js).unwrap().unwrap();
    let first = p.caches.first.clone();
    p.clear_caches(); // settings save
    let c = p.load_spec(&loc, &ctx, &mut js).unwrap().unwrap();
    assert!(Rc::ptr_eq(&a, &b) && Rc::ptr_eq(&a, &c) && Rc::ptr_eq(&first, &p.caches.first));
    // an edited spec still reloads in developer mode
    std::fs::write(
        build.join("mk.js"),
        r#"var s={name:"mk",args:{name:"target"}};export{s as default};"#,
    )
    .unwrap();
    let d = p.load_spec(&loc, &ctx, &mut js).unwrap().unwrap();
    assert_eq!(d.args[0].name.as_deref(), Some("target"));
}

#[test]
fn aborts_propagate() {
    let g = spec(json!({"name": "g", "generateSpec": {"$fn": 1}}));
    let mut m = Mock::with(&[("g", &g)]);
    m.fns.insert(1, Box::new(|_| Err(ParseError::Aborted)));
    assert!(matches!(run(&mut m, "g x"), Err(ParseError::Aborted)));
}
