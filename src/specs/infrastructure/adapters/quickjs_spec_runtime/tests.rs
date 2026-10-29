use super::*;
use crate::specs::application::ports::runtime_event_source::ChannelRuntimeEventSource;
use crate::specs::infrastructure::adapters::fig_spec_mapper::fn_id;
#[cfg(unix)]
use crate::specs::infrastructure::adapters::process_executor::fake_bin;
use crate::shared::infrastructure::testing::temp_dir::TempDir;

fn js() -> (QuickJsSpecRuntime, Rc<ChannelRuntimeEventSource>) {
    let events = Rc::new(ChannelRuntimeEventSource::new());
    (QuickJsSpecRuntime::new(events.clone(), &Settings::default()).unwrap(), events)
}

/// Write a module `name` with `src` into `dir` and load it.
fn module(js: &QuickJsSpecRuntime, dir: &TempDir, name: &str, src: &str) -> Map<String, Value> {
    let p = dir.path().join(name);
    std::fs::write(&p, src).unwrap();
    js.load_module(&p).unwrap()
}

fn call_default(
    js: &QuickJsSpecRuntime,
    ns: &Map<String, Value>,
    args: Vec<JsArg>,
    ctx: Option<&GeneratorContext>,
) -> Result<Value, SpecError> {
    js.call_json(
        fn_id(&ns["default"]).unwrap(),
        args,
        ctx,
        Duration::from_secs(5),
    )
}

fn find_key<'a>(v: &'a Value, key: &str) -> Option<&'a Value> {
    match v {
        Value::Object(m) => m
            .get(key)
            .or_else(|| m.values().find_map(|x| find_key(x, key))),
        Value::Array(a) => a.iter().find_map(|x| find_key(x, key)),
        _ => None,
    }
}

/// The `generatorContext` object for `custom(tokens, exec, generatorContext)` (SPEC §6.5). An empty `env`
/// means "session env unknown" → the wrapper's own environment. Test-only: production builds it with
/// `fig_spec_mapper::generator_context_json`, which keeps an empty env as `{}` (D1, D7).
fn context_json(context: &GeneratorContext) -> Value {
    let env: Map<String, Value> = if context.env().is_empty() {
        std::env::vars_os()
            .map(|(k, v)| {
                (
                    k.to_string_lossy().into_owned(),
                    v.to_string_lossy().into_owned().into(),
                )
            })
            .collect()
    } else {
        context
            .env()
            .iter()
            .map(|(k, v)| (k.clone(), v.clone().into()))
            .collect()
    };
    json!({
        "currentWorkingDirectory": context.cwd(),
        "currentProcess": context.shell(),
        "sshPrefix": "",
        "environmentVariables": env,
        "searchTerm": context.search_term(),
        "isDangerous": context.is_dangerous(),
    })
}

#[test]
fn loads_hand_written_es_modules() {
    let (js, _tx) = js();
    let dir = TempDir::new("esm");
    let src = "export const sub = { name: \"ok\" };\nconst spec = { name: \"zz\", subcommands: [sub] };\nexport default spec;\n";
    let ns = module(&js, &dir, "zz.js", src);
    assert_eq!(
        ns["default"],
        json!({"name": "zz", "subcommands": [{"name": "ok"}]})
    );
    let ns = module(
        &js,
        &dir,
        "inline.js",
        "export default {\n  name: \"inline\",\n} // end",
    );
    assert_eq!(ns["default"], json!({"name": "inline"}));
    // The esbuild form still wins.
    assert_eq!(
        module(
            &js,
            &dir,
            "eb.js",
            "var s={name:\"eb\"};export{s as default};"
        )["default"],
        json!({"name": "eb"})
    );
}

#[test]
fn serializes_functions_and_values() {
    let (js, _tx) = js();
    let dir = TempDir::new("ser");
    let ns = module(
        &js,
        &dir,
        "m.js",
        "var f=()=>1,c={};c.self=c;var s={a:f,b:[f,1,undefined],u:undefined,r:/x+/g,n:null,big:10n,c,sym:Symbol()};\
         function g(){}export{s as default,g as helper};",
    );
    let d = &ns["default"];
    assert_eq!(d["a"], d["b"][0], "same function → same id");
    let (fa, fg) = (fn_id(&d["a"]).unwrap(), fn_id(&ns["helper"]).unwrap());
    assert_ne!(fa, fg);
    assert_eq!(d["b"], json!([{"$fn": fa}, 1, null]));
    assert_eq!(d.get("u"), None);
    assert_eq!(d.get("sym"), None);
    assert_eq!(d["r"], "/x+/g");
    assert_eq!(d["n"], Value::Null);
    assert_eq!(d["big"], "10");
    assert_eq!(d["c"], json!({"self": null}), "cycles → null");
    // Cached by path: same ids, no re-evaluation.
    assert_eq!(js.load_module(&dir.path().join("m.js")).unwrap(), ns);
    assert_eq!(
        js.call_json(fa, vec![], None, Duration::from_secs(2)),
        Ok(json!(1))
    );
    // Empty file → empty namespace; syntax error → Other.
    assert!(module(&js, &dir, "empty.js", " \n").is_empty());
    std::fs::write(dir.path().join("bad.js"), "var = ;export{a as default};").unwrap();
    assert!(matches!(
        js.load_module(&dir.path().join("bad.js")),
        Err(SpecError::Invalid(_))
    ));
}

#[cfg(feature = "embed-specs")]
#[test]
fn loads_bundled_specs() {
    let b = crate::specs::infrastructure::adapters::embedded_spec_bundle::testing::bundle_subset(&[
        "ls.js",
        "git.js",
        "aws/s3.js",
        "heroku/index.js",
    ]);
    let (js, _tx) = js();
    let ls = js.load_module(&b.path().join("ls.js")).unwrap();
    assert_eq!(ls["default"]["name"], "ls");
    let s3 = js.load_module(&b.path().join("aws/s3.js")).unwrap();
    assert_eq!(s3["default"]["name"], "s3");
    assert!(s3["default"]["subcommands"]
        .as_array()
        .is_some_and(|a| !a.is_empty()));

    let git = js.load_module(&b.path().join("git.js")).unwrap();
    assert_eq!(git["default"]["name"], "git");
    assert!(fn_id(&git["gitGenerators"]["commits"]["postProcess"]).is_some());
    let pp = fn_id(&git["gitGenerators"]["remotes"]["postProcess"]).unwrap();
    let stdout = "origin\tgit@github.com:a/b.git (fetch)\norigin\tgit@github.com:a/b.git (push)\nup\thttps://gitlab.com/x (fetch)";
    let out = js.call_json(
        pp,
        vec![
            JsArg::Json(stdout.into()),
            JsArg::Json(json!(["git", "push", ""])),
        ],
        None,
        Duration::from_secs(2),
    );
    assert_eq!(
        out,
        Ok(json!([
            {"name": "origin", "icon": "fig://icon?type=github", "description": "Remote"},
            {"name": "up", "icon": "fig://icon?type=gitlab", "description": "Remote"},
        ]))
    );

    // Diff-versioned resolver: `default(version)` picks the newest file <= version.
    let heroku = js.load_module(&b.path().join("heroku/index.js")).unwrap();
    assert!(fn_id(&heroku["getVersionCommand"]).is_some());
    for (v, want) in [
        (json!("8.6.0"), "heroku/8.6.0"),
        (json!("8.1.0"), "heroku/8.0.0"),
        (Value::Null, "heroku/8.6.0"),
    ] {
        let r = call_default(&js, &heroku, vec![JsArg::Json(v.clone())], None).unwrap();
        assert_eq!(r["versionedSpecPath"], want, "version {v}");
    }
}

/// `parserDirectives.alias` of git.js run against a fake `git` on a private PATH.
#[cfg(all(unix, feature = "embed-specs"))]
#[test]
fn git_alias_directive_with_fake_git() {
    let bin = fake_bin(&[("git", r#"[ "$3" = alias.co ] && echo checkout || exit 1"#)]);
    let b = crate::specs::infrastructure::adapters::embedded_spec_bundle::testing::bundle_subset(&["git.js"]);
    let (js, _tx) = js();
    let git = js.load_module(&b.path().join("git.js")).unwrap();
    let alias = fn_id(
        find_key(&git["default"], "parserDirectives")
            .unwrap()
            .get("alias")
            .unwrap(),
    )
    .unwrap();
    let ctx = GeneratorContext::new(
        String::new(),
        [("PATH".to_string(), bin.path().display().to_string())].into(),
        String::new(),
        String::new(),
        false,
    );
    let run = |js: &QuickJsSpecRuntime, tok: &str| {
        js.call_json(
            alias,
            vec![JsArg::Json(tok.into()), JsArg::Exec],
            Some(&ctx),
            Duration::from_secs(5),
        )
    };
    assert_eq!(run(&js, "co"), Ok(json!("checkout")));
    assert!(
        matches!(run(&js, "nope"), Err(SpecError::Invalid(e)) if e.contains("Failed parsing alias"))
    );
}

/// A custom generator calling `executeShellCommand` (object and legacy string forms) through a fake `echo`.
#[cfg(unix)]
#[test]
fn custom_generator_executes_commands() {
    let bin = fake_bin(&[("echo", r#"printf 'fake:%s\n' "$*""#)]);
    let cwd = TempDir::new("cwd");
    let (js, _tx) = js();
    let ns = module(
        &js,
        &cwd,
        "gen.js",
        "var g=async(tokens,exec,ctx)=>{let r=await exec({command:\"echo\",args:[tokens[0],ctx.searchTerm]});\
         let l=await exec(\"echo legacy | tr a-z A-Z\");let p=await exec({command:\"sh\",args:[\"-c\",\"pwd -P; printf %s $NO_COLOR$X\"],env:{X:\"y\"}});\
         let e;try{await exec({command:\"no-such-cmd-tabsmith\"})}catch(x){e=String(x)}\
         return [r.stdout,r.status,l,p.stdout,e]};export{g as default};",
    );
    let path = format!("{}:/bin:/usr/bin", bin.path().display());
    let ctx = GeneratorContext::new(
        cwd.path().display().to_string(),
        [("PATH".to_string(), path)].into(),
        String::new(),
        "term".into(),
        false,
    );
    let args = vec![
        JsArg::Json(json!(["git"])),
        JsArg::Exec,
        JsArg::Json(context_json(&ctx)),
    ];
    let out = call_default(&js, &ns, args, Some(&ctx)).unwrap();
    let real_cwd = cwd.path().canonicalize().unwrap().display().to_string();
    assert_eq!(out[0], "fake:git term");
    assert_eq!(out[1], 0);
    assert_eq!(
        out[2], "LEGACY",
        "legacy string form → sh -c, resolves to stdout (echo is a builtin there)"
    );
    assert_eq!(out[3], format!("{real_cwd}\n1y"));
    assert!(
        out[4]
            .as_str()
            .unwrap()
            .contains("failed running executable"),
        "{out}"
    );
}

#[test]
fn set_timeout_ordering_and_intl() {
    let (js, _tx) = js();
    let dir = TempDir::new("timers");
    let ns = module(
        &js,
        &dir,
        "t.js",
        "var f=()=>new Promise(res=>{const o=[];setTimeout(()=>o.push(\"b\"),30);setTimeout(x=>o.push(x),0,\"a\");\
         const c=setTimeout(()=>o.push(\"never\"),10);clearTimeout(c);setTimeout(()=>res(o),60)});export{f as default};",
    );
    assert_eq!(
        call_default(&js, &ns, vec![], None),
        Ok(json!(["a", "b"]))
    );
    let ns = module(
        &js,
        &dir,
        "i.js",
        "var f=n=>new Intl.NumberFormat(void 0,{notation:\"compact\",compactDisplay:\"short\",maximumSignificantDigits:3}).format(n);\
         export{f as default};",
    );
    for (n, want) in [
        (1234567, "1.23M"),
        (15300, "15.3K"),
        (999, "999"),
        (2_000_000_000, "2B"),
    ] {
        assert_eq!(
            call_default(&js, &ns, vec![JsArg::Json(json!(n))], None),
            Ok(json!(want))
        );
    }
}

#[test]
fn interrupt_stops_infinite_loop() {
    let (js, _tx) = js();
    let dir = TempDir::new("loop");
    let ns = module(
        &js,
        &dir,
        "l.js",
        "var f=()=>{while(true){}};var g=x=>x*2;export{f as default,g};",
    );
    let t0 = Instant::now();
    let r = js.call_json(
        fn_id(&ns["default"]).unwrap(),
        vec![],
        None,
        Duration::from_millis(200),
    );
    assert!(
        matches!(r, Err(SpecError::Timeout) | Err(SpecError::Invalid(_))),
        "{r:?}"
    );
    assert!(t0.elapsed() < Duration::from_secs(3));
    // The runtime is still usable.
    assert_eq!(
        js.call_json(
            fn_id(&ns["g"]).unwrap(),
            vec![JsArg::Json(json!(21))],
            None,
            Duration::from_secs(2)
        ),
        Ok(json!(42))
    );
}

/// The engine waits in 50 ms slices; a callback needing more CPU than one slice must still finish.
#[test]
fn long_callback_survives_short_pump_slices() {
    let (js, _tx) = js();
    let dir = TempDir::new("slice");
    let ns = module(&js, &dir, "b.js", "var f=ms=>{const t=Date.now();while(Date.now()-t<ms){}return ms};export{f as default};");
    let f = fn_id(&ns["default"]).unwrap();
    let tasks = [200, 40].map(|ms| js.start_call(f, vec![JsArg::Json(json!(ms))], None));
    let t0 = Instant::now();
    let mut got = vec![None, None];
    while got.iter().any(Option::is_none) && t0.elapsed() < Duration::from_secs(3) {
        for (g, r) in got.iter_mut().zip(
            js.wait_json(&tasks, Instant::now() + Duration::from_millis(50))
                .unwrap(),
        ) {
            if g.is_none() {
                *g = r; // results are delivered once
            }
        }
    }
    assert_eq!(got, vec![Some(Ok(json!(200))), Some(Ok(json!(40)))]);
}

/// Developer mode: an edited spec file is re-evaluated, an unchanged one is not (see `serializes_…`).
#[test]
fn load_module_reloads_edited_file() {
    let (js, _tx) = js();
    let dir = TempDir::new("reload");
    assert_eq!(
        module(
            &js,
            &dir,
            "m.js",
            "var s={name:\"v1\"};export{s as default};"
        )["default"]["name"],
        "v1"
    );
    let ns = module(
        &js,
        &dir,
        "m.js",
        "var s={name:\"v2-edited\"};export{s as default};",
    );
    assert_eq!(ns["default"]["name"], "v2-edited");
}

/// Evaluate every module of the bundle (SPEC §10 `js`):
/// `cargo test all_bundle_modules -- --ignored --nocapture`.
#[cfg(feature = "embed-specs")]
#[test]
#[ignore]
fn all_bundle_modules() {
    let dir = crate::specs::infrastructure::adapters::embedded_spec_bundle::testing::full_bundle();
    let mut files = Vec::new();
    let mut stack = vec![dir.path().to_path_buf()];
    while let Some(d) = stack.pop() {
        for e in std::fs::read_dir(d).unwrap().flatten() {
            let p = e.path();
            if p.is_dir() {
                stack.push(p);
            } else if p.extension().is_some_and(|x| x == "js") {
                files.push(p);
            }
        }
    }
    files.sort();
    let t0 = Instant::now();
    let (mut ok, mut empty, mut no_default, mut failed) = (0, 0, 0, Vec::new());
    let mut host = js();
    let mut slowest = (Duration::ZERO, PathBuf::new());
    for (i, f) in files.iter().enumerate() {
        if i % 100 == 0 {
            host = js(); // bound the namespace cache
        }
        let t = Instant::now();
        let r = host.0.load_module(f);
        if t.elapsed() > slowest.0 {
            slowest = (t.elapsed(), f.clone());
        }
        match r {
            Ok(ns) if ns.is_empty() => empty += 1,
            Ok(ns) if !ns.contains_key("default") => no_default += 1,
            Ok(_) => ok += 1,
            Err(e) => failed.push(format!(
                "{}: {e}",
                f.strip_prefix(dir.path()).unwrap().display()
            )),
        }
    }
    println!(
        "modules={} ok={ok} empty={empty} no_default={no_default} failed={} in {:?} (slowest {:?}: {})",
        files.len(),
        failed.len(),
        t0.elapsed(),
        slowest.0,
        slowest.1.display()
    );
    for f in failed.iter().take(20) {
        println!("  {f}");
    }
    assert!(failed.is_empty());
}

#[test]
fn should_pass_a_query_term_function_the_search_term() {
    let search_term = String::from("ma");
    let call = SpecFunctionCall::QueryTerm {
        function: 3,
        search_term: search_term.clone(),
    };

    let arguments = call_arguments(&call, &GeneratorContext::default());

    assert_eq!(arguments, (3, vec![JsArg::Json(json!(search_term))]));
}

#[test]
fn should_pass_a_trigger_the_new_and_the_old_token() {
    let (new, old) = ("a/", "a");
    let call = SpecFunctionCall::Trigger {
        function: 4,
        new_token: new.into(),
        old_token: old.into(),
    };

    let arguments = call_arguments(&call, &GeneratorContext::default());

    assert_eq!(
        arguments,
        (4, vec![JsArg::Json(json!(new)), JsArg::Json(json!(old))])
    );
}

#[test]
fn should_pass_an_alias_function_the_token_and_the_exec_function() {
    let token = "co";
    let call = SpecFunctionCall::Alias {
        function: 5,
        token: token.into(),
    };

    let arguments = call_arguments(&call, &GeneratorContext::default());

    assert_eq!(arguments, (5, vec![JsArg::Json(token.into()), JsArg::Exec]));
}

#[test]
fn should_pass_a_load_spec_function_the_token_and_the_exec_function() {
    let token = "run";
    let call = SpecFunctionCall::LoadSpec {
        function: 6,
        token: token.into(),
    };

    let arguments = call_arguments(&call, &GeneratorContext::default());

    assert_eq!(arguments, (6, vec![JsArg::Json(token.into()), JsArg::Exec]));
}

#[test]
fn should_pass_a_cache_key_function_the_tokens_in_an_object_and_no_exec_function() {
    let tokens = vec!["npm".to_string(), "run".to_string()];
    let call = SpecFunctionCall::GenerateSpecCacheKey {
        function: 7,
        tokens: tokens.clone(),
    };

    let arguments = call_arguments(&call, &GeneratorContext::default());

    assert_eq!(
        arguments,
        (7, vec![JsArg::Json(json!({ "tokens": tokens }))])
    );
}

#[test]
fn should_pass_generate_spec_the_tokens_and_the_exec_function() {
    let tokens = vec!["npm".to_string(), "run".to_string()];
    let call = SpecFunctionCall::GenerateSpec {
        function: 8,
        tokens: tokens.clone(),
    };

    let arguments = call_arguments(&call, &GeneratorContext::default());

    assert_eq!(arguments, (8, vec![JsArg::Json(json!(tokens)), JsArg::Exec]));
}
