# Phase 5 (specs): preparation

Status: **prepared, not started; re-checked against `0a0a74e`, the end of Phase 4.** Phase 5 starts there, with the
gate green: 761 passed, 0 failed, 1 ignored (`all_bundle_modules`), clippy 3 warnings (budget 3), golden identical,
testlock MISSING 0 and DUPLICATE 0.
The prep was researched on a frozen snapshot of `a3953cf` (the end of Phase 2) by five researchers: model, bundle,
runtime, repository and consumers. A judge then chose the runtime/repository port design out of three candidates,
and this file merges the reports and the verdict. The 17 step prompts (ids `5.1a` … `5.5`, see "Step briefs") are in
`phase-5-specs.args.json`, next to this file. Where this file departs from the judge (`SpecValue` is an exact tree,
P5), it says so.

Plan reference: [the plan](../hexagonal-refactor-plan.md).
- Section 5, "Phase 5: specs" and "Phase 6: completion" (Phase 6 consumes what Phase 5 builds).
- The `specs`, `completion` and `bootstrap` layouts in section 4.
- Sections 2 (allowed dependencies, bridges), 7 (risks), 8 (verification) and 9 (D1-D9).

Detailed traps per legacy file are in [`source-map/spec.md`](source-map/spec.md), [`specs.md`](source-map/specs.md),
[`js.md`](source-map/js.md) and [`parser.md`](source-map/parser.md). For the pieces Phase 5 takes out of other files,
see `engine.md`, `suggest.md` and `types.md`.

**Line numbers.** The source map cites `af1b304`; this file cites **`a3953cf`**.
- Phases 3-4 do not touch `spec.rs`, `specs.rs`, `js.rs`, `parser.rs` or `parser/tests.rs`. Their numbers hold
  until a Phase 5 step moves code (`spec.rs` is byte-identical to `af1b304`).
- `engine.rs`, `types.rs`, `pty.rs`, `inline.rs`, `suggest.rs`, `tracker.rs`, `doctor.rs`, `cli.rs`, `main.rs` and
  `bootstrap/*` changed in Phases 3-4. At `0a0a74e`, for example, suggest's `-ItemType` test string is at 2331 (2329
  here), doctor's `crate::specs` calls at 401/466 (374/437), `Failed to fix` at 156 (142), `cli.rs`'s `set_var`s at
  582-585 (573-576) and pty's `kill_live_children` calls at 114/1309 (111/1297). A citation marked "at `0a0a74e`"
  was re-checked there. Locate items there by
  symbol or content, never by line number. Each Phase 5 step also moves code for the next one.

**What Phases 3-4 leave for Phase 5.**
- Phase 3 is done (`f485ea7`):
  - `SharedDependencies::from_env()` lives in shared infrastructure.
  - The engine builds `HistoryModule` on the engine thread.
  - The fitness test has a `LEGACY_MODULES` rule: a registered module's domain or application may not use a legacy
    flat module. `specs` is not in that list, because the legacy `specs.rs` shares the module's name. So Phase 5
    checks by hand that the specs domain and application never reach legacy `crate::specs::` items.
- Phase 4 is done (`6f38da1..ab7052b`, then `d72a4f6` and `0a0a74e`), and this file was re-checked at `0a0a74e`.
  `REGISTERED_MODULES` = `history`, `settings`, `shared`, `shell_integration`. `src/terminal_session/` exists and stays
  unregistered until Phase 9.
  - `integrations.rs` is deleted.
  - Setup's extraction goes through the `shell_integration::SpecBundleInstaller` port, implemented in
    `src/bootstrap/bridges.rs` by the unit struct `SpecBundleBridge` over `crate::specs::ensure_extracted()`. It maps
    `Err(e)` through the private `spec_bundle_failure(error: anyhow::Error)` to
    `ShellIntegrationError::SpecBundle(format!("{error:#}"))`, whose Display is
    `could not extract the completion specs: {message}`. The bridge's test
    `should_keep_the_whole_chain_of_a_failed_extraction` pins that text. Only `bootstrap/cli.rs`'s
    `shell_integration_module()` builds the bridge, as
    `build_shell_integration_module(&SharedDependencies::from_env(), Rc::new(SpecBundleBridge))`, for the
    `integrations`, `init`, `setup`, `uninstall` and `_ should-launch` arms.
  - `types::SessionCtx` is gone. The engine gets the session context as `EngineMsg::Session { env, alias }` and keeps
    `session_env`/`session_alias`; `SessionContext` is terminal_session's entity, used only by the pty (its
    session-context repository). `session_update_refreshes_first_token_items` sends
    `EngineMsg::Session { env: HashMap::new(), alias: None }`. Phase 4 also added five engine tests on the session
    context, over the test helpers `engine_with_wrapper_env` (a second `Engine::with` and `Js::new`) and
    `report_session_context(e, js: &mut Js, ..)`, which 5.3c-d and 5.4b touch (see "Test moves").
  - `doctor.rs` still calls `crate::specs::{version, ensure_extracted}`, and its check joins `ctx.data_root()` (Phase
    4's `IntegrationPaths`). Doctor itself belongs to Phase 10.
  - `crate::shared::infrastructure::error_chain::display_chain(&(dyn Error + 'static))` renders anyhow's `{:#}` chain:
    the Display, then `: ` and each `source()`. Settings' `load_or_defaults` and shell_integration's
    `os_shell_environment` use it. No other hand-written chain renderer may be written (plan section 9, Phase 4
    notes).
  - The fitness test has rules 6-8 (see "Cross-cutting"), and testlock prints a DUPLICATE line.

## Where the code is today (`a3953cf`)

| What | Where | Used by | Goes to (step) |
|---|---|---|---|
| Spec model: `FnId`, `StrOrFn`, `Suggestion`, `Trigger`, `CacheCfg`, `Script`, `Generator`, `Repeat`, `Strategy`, `Arg`, `Opt`, `Directives` (+ hand-written `Default`), `SpecLocation`, `LoadSpec`, `Subcommand` (+ `reindex`) | `spec.rs:10-231` | `engine.rs:24`; `parser.rs:9-11`; `parser/tests.rs` (`super::*`); `suggest.rs:6`, test 1458; `js.rs:14, 410, 513` (`FnId`) | specs domain (5.1b); private fields (5.1d-5.1e) |
| `normalise`, `suggestion_from_json`, `load_spec_result`, the private readers, `fn_id`; 8 tests | `spec.rs:233-518`, tests `520-791` | `parser.rs:255, 265, 296, 359, 1193, 1257`; `engine.rs:197, 212`; js tests (`fn_id`, 1118) | `fig_spec_mapper.rs` (5.1b) |
| `ItemType` (+ unused serde derives), `ItemType::from_fig` | `types.rs:11-40` | about 250 references: `types.rs` `Item.kind`/`original_kind`; `engine.rs:29, 186`; `suggest.rs` (130); `inline.rs:10, 272`; `menu.rs` (34); `icons.rs` (42); `render.rs:8, 604-606`; `pty.rs` tests | `SuggestionKind` (5.1a); `from_fig` → private fn in `spec.rs` (5.1a) → mapper (5.1b) |
| `fig_type`, `template_json` | `suggest.rs:1347-1373` | `engine.rs:760` | mapper (5.1b) |
| `truthy`, `script_exec_input`, `script_string`, `json_items`, `post_process_items`; tests `legacy_string_script_runs_through_sh_c`, `js_truthiness`, `post_process_filtering` | `engine.rs:128-217`; tests `1282-1326` | `engine.rs:632, 855, 890, 918, 946, 947` | mapper; `SpecSuggestion::from_script_output` (5.1c) |
| `gen_context_json` | `engine.rs:220-223` | `engine.rs:798` | mapper `generator_context_json` (5.3a) |
| `loc_key` | `parser.rs:208-213` | `parser.rs:245, 1035` | `SpecLocation::cache_key` (5.2a) |
| `spec_location`; test `spec_location_table` | `parser.rs:165-189`; `parser/tests.rs:655` | `parser.rs:1005, 1113` | `spec_locator.rs` (5.2a) |
| `SemVer`, `semver`, `semver_clean`, `semver_cmp`; test `semver_helpers` | `parser.rs:510-560`; `parser/tests.rs:759` | `parser.rs:385, 395-399` | `semantic_version.rs` (5.2a) |
| `apply_versions` key selection | `parser.rs:393-404` | `Loader::versioned` | `spec_version_selector.rs` (5.2a) |
| `SpecIndex` | `specs.rs:132-139` | `engine.rs:25, 288, 311-316, 329, 1018`, test 1336; `parser.rs:12, 107, 328`; `parser/tests.rs:899` | `spec_index.rs` (5.2a) |
| `resolve_in` | `specs.rs:205-221` | `specs::resolve` | `spec_locator::find_global_spec` (5.2a), adapter binding (5.2b) |
| cleanup staleness rule (`VERSION_STALE`, `TMP_STALE`) | `specs.rs:24-27, 82-107` | `ensure_extracted`, `extract_to` | `bundle_retention_policy.rs` (5.2a) |
| `ARCHIVE`, `version`, `LAST_USED`, `ensure_extracted`, `extract_to`, `touch`, cleanup loop, `unpack`, `user_dir`, `SpecIndex::{load, load_from}`, `resolve`, `dev_folder`, `testutil`; 9 tests | `specs.rs:13-252`, tests `254-492` | `engine.rs:311-312`; `doctor.rs:374, 437`; Phase 4 `SpecBundleBridge`; `parser.rs:328`; js tests 1255, 1310, 1785 | `embedded_spec_bundle.rs`, `SpecBundle`, `InstallSpecBundleUseCase`, `specs_module.rs` (5.2b) |
| `JsError` | `js.rs:32-49` | `engine.rs:9, 593, 633, 964`; `parser.rs:3, 88-95, 372`; js tests | `SpecError` (5.3a) |
| `GenCtx` (+ test-only `context_json`), `ExecInput` (serde) | `js.rs:51-113` | engine, parser, js | `GeneratorContext`, `ExecRequest` + infra `ExecInput` record (5.3a) |
| `ExecOutput` (serde) | `types.rs:213-219` | `js.rs`, `EngineMsg::ExecDone` | `ProcessOutput` (5.3a) |
| `exec`, `key_eq`, `build_command`, `find_exe`, `is_executable`, `git_bash`, `wait_output`, `LIVE`, `live`, `Live`, `kill_live_children`, `job`, `clean`; `native_ls`; 6 tests | `js.rs:692-1086`, `1088-1113` | exec worker threads; `pty.rs:111, 1297`; the atexit hook | `process_executor.rs`, `native_ls.rs` (5.3b) |
| `PRELUDE`, `wrap_module`; test | `js.rs:127-168`, `655-690` | `Js::new`, `load_module` | `quickjs_module_wrapper.rs` (5.3b) |
| `Js`, `Host`, the consts, `script_timeout`, `JsArg`, `TaskId`, `TaskResult`; 10 tests | `js.rs:92-650` | `engine.rs:62` and every `js` parameter; `parser.rs` | `quickjs_spec_runtime.rs` (5.3c) |
| `Js::{rx, deferred, next_msg, try_next_msg, internal}`; `EngineMsg::{ExecDone, Timer}`; test `wait_aborts_and_defers_on_complete` | `js.rs:222-223, 329-354, 563-578`; `types.rs:252-260`; `js.rs:1720-1777` | the engine loop at `engine.rs:364, 374, 428` | `EngineInbox` in `engine.rs` + `RuntimeEventSource` (5.3d) |
| (new) typed calls and results | | | `SpecFunctionCall`, `SpecFunctionResult`, `SpecRuntime` (5.3e); `QuickJsSpecRuntime` implements it (5.3f) |
| parser `Host::call` + the test `Mock` | `parser.rs:566-592`; `parser/tests.rs:77-118` | parser | typed `SpecFunctionCall` (5.3g) |
| `TIMEOUT`, `Loader`, `versioned`, `spec_file`, `cli_version` | `parser.rs:27, 216-388` | `Parser`, `JsHost` | `QuickJsSpecRepository` (5.4b) |
| `apply_versions` fold, `merge_*`, `shallow_merge`, `make_array`, `names`, `truthy`; tests `diff_merge`, `version_pick` | `parser.rs:393-508`; `parser/tests.rs:699, 733` | `Loader::versioned` | `fig_version_diff.rs` (5.4a) |
| `Parser` (+ `load_spec`, test-only), `JsHost`, `From<JsError> for ParseError` | `parser.rs:88-95, 111-161, 573-592` | `engine.rs:287, 338, 418, 494` | `Parser::new(repository, runtime)` (5.4b) |

## Behaviour that must not change

### Model and Fig mapper (5.1)
- **Rc identity is behaviour.** These sites create `Rc`s: `spec.rs:275, 304, 332, 334, 372, 467`. These sites key on
  their addresses:
  - `gen_key`, `engine.rs:119-120` (this bullet's engine.rs and suggest.rs lines are at `0a0a74e`);
  - the arg key in `Engine::compute`, `engine.rs:516-519`;
  - the `Rc::ptr_eq` slot reuse in `Engine::update_slots`, `engine.rs:624-627`;
  - `same_row`, `suggest.rs:695`;
  - `by_json`, `parser.rs:262-268`;
  - the shallow clone in generateSpec, `parser.rs:1277`.

  Keep `Vec<Rc<_>>` and `derive(Clone)`. Accessors return references into the same `Vec`, so the `std::ptr::eq`
  assertions in `normalise_sudo` and `normalise_git_like` still hold. Never deep-clone.
- **No typed equality.** `Suggestion`, `Arg`, `Opt`, `Generator`, `Subcommand` and `LoadSpec` derive no
  `PartialEq` (`spec.rs:20, 71, 103, 127, 173, 181`). `parser.rs:262` keeps identity by comparing raw JSON
  (`*old == j`). The only exception is `SpecSuggestion: PartialEq`, added in 5.3e for `SpecFunctionCall`. No
  existing comparison uses it.
- **`reindex`** (`spec.rs:215-230`):
  - It runs `for (persistent, list) in [(true, &self.persistent), (false, &self.options)]`. Options win a name
    clash, and within one list the later entry wins.
  - It never dedupes the `Vec`s: a fully shadowed subcommand stays in `subcommands` (test lines 732-735 expect a
    length of 2).
  - `sub_index` and `opt_index` are lookup-only `HashMap`s and are never iterated for output.
  - Only the mapper (`spec.rs:337`) and the generateSpec merge (`parser.rs:1293`) call it. The test struct literals
    in `suggest.rs` (the `sub` helper, `assembly_options_rules`, `option_chain_rewriting`, `help_template`: 1495-1499,
    1816-1828, 1855-1863, 2541-2544 at `0a0a74e`) build un-reindexed specs, so builders must be plain
    setters that never reindex.
- **The persistent split** (`spec.rs:330-336`). An `isPersistent` option lives only in `persistent`, and
  `opt_index`'s bool says which list it is in. The spec test helper `opt` (537-542) honours that bool. The parser
  test helper `oarg` (`parser/tests.rs:63-66`) ignores it and always indexes `options`. Keep both helpers as they are.
- **`ParserDirectives::default` is hand-written** with separators `["="]` (`spec.rs:155-163`). `Subcommand`'s
  derived `Default` relies on it (`normalise_sudo` line 555). Never derive it.
- **`optionArgSeparators`** (`spec.rs:323-327`):
  - a string is a set of chars, and `""` gives an empty list;
  - an array goes through `strs`;
  - anything else, `null` included, gives `["="]`;
  - a non-object `parserDirectives` leaves `has_directives` false (317);
  - `Directives` is replaced whole, never merged, and `has_directives` is only ever set together with it (318-319).
- **The arg-level template generator** is built by `generator(&json!({"template": template}))` and inserted at index 0
  (`spec.rs:376-380`). That index feeds `gen_key`. Because it goes through `generator()`, it gets trigger `Str("/")`
  and getQueryTerm `Str("/")` for filepaths and folders only (414, 438, 444).
- **Trigger vs getQueryTerm fallbacks are asymmetric** (`spec.rs:427-444`):
  - An explicit unrecognised trigger (`null`, a number, `{on:"x"}`) gives `None` and suppresses the path default,
    because `None if path_template` fires only when the key is absent (438).
  - getQueryTerm falls back through `.or_else` (444).
  - The string check comes before the `$fn` check (428-429), and `""` gives `Str("")`.
- **Numeric reads keep their `as` casts**, which saturate or truncate:
  - `isRepeatable`: `n.as_f64().unwrap_or(1.0) as u32` (357);
  - `threshold`: `as usize` (433);
  - `ttl`: `as u64` (446);
  - `scriptTimeout`: `.map(|ms| ms as u64)` (451-454);
  - `debounce`: `true` gives 200, otherwise `filter(ms > 0.0).map(as u64)` (391-395).
- **More reader rules:**
  - `isRepeatable`: absent gives `No`, explicit `false` gives `Times(1)` (353-359).
  - `requiresSeparator`: any string, even `""`, gives `Some`; `true` gives `Some("")`; anything else gives `None`
    (361-365).
  - `location()` (281-291): a missing or non-string path gives the literal `PathBuf` `"~/"`, which `spec_file`
    expands later; path `""` gives an empty `PathBuf`.
  - `objects()` drops non-objects (497-499). `items()` treats a single object as a one-element list and keeps
    falsy scalars (488-495). `strs` drops non-strings (501-505). `b` is a strict bool (511-513).
- **`fn_id`** (`spec.rs:516-518`) is `v.get("$fn").and_then(Value::as_u64).map(|n| n as FnId)`. Floats, negatives
  and strings are not functions, so `{"$fn":"x"}` becomes `Script::Object` (420). Values above `u32::MAX` truncate.
  `js.rs:441` hands that same `u32` to JS `__ts.start`.
- **`load_spec` (461-471) and `load_spec_result` (269-278) differ on purpose.**
  - A static string gives `Global`; a result string gives `None` (test line 789).
  - The static form checks `$fn`; the result form does not.
  - A result object with a `type` key, even `null`, gives `Locations`, possibly empty (272).
  - A result array drops invalid entries.
- **`suggestion_from_json`** (`spec.rs:240-266`):
  - a string gives names `[s]` with kind `None`, and `""` is kept;
  - an object needs a non-empty `strs` name;
  - an unknown `type` string gives `Some(Arg)` (`types.rs:37`), and a non-string `type` gives `None` (the caller
    then defaults to Arg, `suggest.rs:166` at `0a0a74e`, in `Sugg::from_suggestion`);
  - `"mixin"` gives `Special` (`types.rs:33`);
  - `priority` is read through `as_f64`;
  - only `context.templateType` is read.
- **`json_items` (`engine.rs:196-204` at `0a0a74e`) and `post_process_items` (207-220) are different rules.**
  - `json_items` keeps string rows with kind `None`, `""` included.
  - `post_process_items` turns strings into `script_string` rows (kind Arg, `insert_value`, `is_dangerous` from the
    run, 185-193), drops `""`, and requires `o.get("name").is_some_and(truthy)` before calling
    `suggestion_from_json`.
- **Do not merge the duplicate readers.** None of these pairs may be deduped:
  - `engine::truthy(&Value)` (131-139 at `0a0a74e`, with a NaN check) vs `parser::truthy(Option<&Value>)` (500-508);
  - `spec::items` vs `parser::make_array` (487-493, which drops falsy non-arrays);
  - `json_items` vs `post_process_items`.
- **`script_exec_input`** (`engine.rs:167-182` at `0a0a74e`):
  - cwd is inserted first, then `m.extend(o)`, so the object's own `cwd` wins, even when it is `null`;
  - an empty array, an empty string, `null`, a number or a bool gives `None`, which leads to `Done([])`, never to
    `Fail`.
- **A script object is opaque until it runs** (`engine.rs:795, 867-879` at `0a0a74e`: `Engine::begin`,
  `Engine::script_input`):
  - `key_src = input.to_string()` (compact text, sorted keys, unknown keys kept) is taken **before** the timeout is
    set;
  - timeout = `max(engine floor, g.script_timeout, input.timeout.as_u64())` (872-875);
  - a malformed object fails only at run time, with `bad script input: {serde message}` (878), logged as
    `engine: generator failed: …`.

  So `GeneratorScript::Object` must give back **exactly** the `Value` it read. Canonical JSON text is not exact:
  with serde_json 1.0.154 and no `float_roundtrip` feature, 593,094 of 1,999,009 random finite `f64` changed after
  `to_string`/`from_str` (e.g. `1.0715660391465826e-75` came back as `…825e-75`). Short decimals are exact.
- **`normalise`** errors only at the top level, with the text `spec is not an object` (`spec.rs:235`). Malformed
  nested parts are skipped silently. The text reaches logs twice:
  - `parser.rs:265`, as `no(format!("{name}: {e}"))`;
  - `parser.rs:1257` then 1271, as `generateSpec ({tokens[0]}): NoSuggestions("spec is not an object")` (Debug).
- **`template_json`** (`suggest.rs:1362-1374` at `0a0a74e`):
  - `name` is the first name or `null`; `type` is `fig_type(kind or Arg)`; `insertValue` may be `null`;
    `isDangerous`; `context.templateType` may be `null`;
  - keys come out sorted;
  - `fig_type` (1348-1360) is an exhaustive match with no `_` arm.
- **`ItemType`'s serde derives are unused.** Nothing that derives serde holds it, and no `serde_json` call takes it.
  Dropping them is required (std-only domain). `Hash` stays.
- **Glossary renames must be compiler-guided, never a blind sed.**
  - `suggest.rs:2331` (at `0a0a74e`) is a test **input** string (`"-ItemType File"`).
  - `engine.rs:240/790/956` (at `0a0a74e`; 237/777/943 at `a3953cf`) `Step::Script` is the engine's own variant.
  - `Sugg`, `from_suggestion`, `additional_suggestions`, `suggestion_from_json` and the doc comments keep the word
    "suggestion".
- **`Arg` has a field named `default`** (`spec.rs:124`). An inherent `fn default(&self)` shadows `Default::default`
  in path calls: this was verified to fail with E0061 and would break `engine.rs:1470` (at `0a0a74e`,
  `generator_keys_differ_per_index`) `Arg::default()` and every
  builder chain. Name the accessor `default_value()`.
- **The generateSpec merge** (`parser.rs:1276-1301`):
  - a base item is dropped when **all** its names are shadowed, so an item with no names is always dropped;
  - generated items are appended after the kept ones;
  - args are replaced only when the level has none, and `st.sub_args` is reset even when the generated args are
    empty too;
  - `reindex` runs after that;
  - `st.persistent` is the generated list followed by the inherited one (generated wins);
  - each uncached call creates a new `Rc`.
- **Clippy budget.** Unused imports are warnings and count against the gate (at most 3). After `template_json` leaves,
  `suggest.rs:11` (at `0a0a74e`) `json` has no production user left. After `script_string` leaves, `engine.rs:33`
  `ItemType` has
  none either.

### Bundle and location (5.2)
- **`ensure_extracted`** (`specs.rs:31-46`):
  - the "installed" test is `dir.join("index.json").is_file()`;
  - the fast path runs `touch` + `cleanup` on **every** call, so never cache "already installed": the touch at each
    engine start is what keeps other binaries' cleanup off this version;
  - `specs_dir()?` runs first;
  - without `embed-specs`, the error is exactly
    `no specs in {dir} (built without the `embed-specs` feature)`, with `dir = specs/<version>`.
- **The engine ensures twice** (`Engine::new`'s `ensure_extracted()` and `SpecIndex::load()`, `engine.rs:316-317` at
  `0a0a74e`, and then `specs.rs:143`): `SpecIndex::load` ensures again
  before `user_dir()?`. `and_then` short-circuits: if the first ensure fails, load is never tried and the error is
  logged once as `engine: specs: {e:#}` (error level), followed by `SpecIndex::default()`.
- **Engine startup order** (`spawn`'s `Js::new` and `Engine::new`: `engine.rs:65` and `315-330` at `0a0a74e`).
  `Js::new` runs first on the engine thread. If it fails,
  nothing is extracted. The index is built before the history.
- **`extract_to`** (50-71):
  - it creates the root first;
  - the temp dir is `{ver}.tmp-{pid}`, an on-disk contract shared with `cleanup`'s `split_once(".tmp-")` and with
    other installed binaries;
  - an unpack error removes the temp dir and returns **without** touch or cleanup, and the root stays (test lines
    355-360);
  - a leftover dir without `index.json` is replaced before the rename;
  - the loser of a concurrent extraction relies on `rename` failing onto the winner's dir and then finding
    `index.json` there;
  - it does its own touch + cleanup (68-69), which `full_bundle` and all four extract tests depend on;
  - never route it through `AtomicFile`.
- **`touch`** ignores every error, `File::create` truncates, and the mtime is `SystemTime::now()`.
- **`cleanup`** (82-107):
  - the stem is the name before `.tmp-` (TMP_STALE, 600 s), else the whole name (VERSION_STALE, 7 days);
  - a dir is "ours" when every dot-separated part is non-empty and all ASCII digits, so `user/`, `dev`, `notes.tmp-1`
    and pre-release names are kept, while a lone `5` is ours;
  - it compares the **full** name `name != ver`, so a stale `<current>.tmp-N` is removed;
  - it requires `is_dir`;
  - the age is the mtime of `<dir>/.last-used`, falling back to the dir's own mtime only when that metadata call
    fails; a `modified()` error keeps the dir;
  - `elapsed().unwrap_or_default() > max`: a future mtime counts as age 0, and the comparison is strict;
  - `read_dir` errors and `remove_dir_all` errors are swallowed, and names use `to_string_lossy`.
- **`unpack`** (111-125):
  - it skips every entry whose file name starts with `._`, at any depth;
  - it unpacks regular files only;
  - it ignores `unpack_in`'s `Ok(false)`;
  - it keeps the `tar` defaults.
- **`load_from`** (146-174):
  - `completions` has no serde default, so an `index.json` without it is a `parsing` error;
    `diffVersionedCompletions` defaults to `[]`;
  - the order is index order (duplicates kept), then the top-level user `*.js` stems in `read_dir` order, deduped;
  - the extension must be exactly `js`; `is_file` follows symlinks; non-UTF-8 stems are skipped;
  - user `read_dir` errors are swallowed;
  - `completions` stays a `Vec`, because its order feeds `first_token_items` (`&self.index.completions`,
    `engine.rs:1031` at `0a0a74e`).
- **`resolve`** (179-185):
  - it rebuilds the roots from the **current** settings on every call: dev folder, then `<specs>/user`, then
    `<specs>/<version>`;
  - `specs_dir().ok()?` means that with no data directory the answer is `None`, even when a dev folder is set;
  - `ensure_extracted` and `user_dir` instead return the home error through `?`.
- **`dev_folder`** (188-201):
  - an empty or non-string folder gives `None`; developer mode off gives `None`;
  - `strip_prefix("~")` strips a whole path component only;
  - when the user home fails, the **literal** path is used.

  The parser's Local `~` (`parser.rs:330-331`) gives `None` instead. Do not share a tilde helper.
- **`resolve_in`** (205-221):
  - an empty name, or any component that is not `Component::Normal`, gives `None`. This is platform-dependent
    `std::path` semantics: do not reimplement it with string splitting;
  - a diff-versioned name skips `<name>.js` in **every** root;
  - the lookup is root-major (`<name>.js`, then `<name>/index.js`, in one root before the next), with `is_file`,
    and the first match wins;
  - the Local lookup (`parser.rs:334-341`) applies neither the validation nor the diff skip.
- **`spec_location`** (`parser.rs:165-189`):
  - the first token is Global only when it has no `/`; `bin/console` or `*/bin/console` gives `php/bin-console`;
  - with isCommand (`Some(false)`) it is Global unless the path starts with `/`, `./` or `~/`;
  - with isScript it is always Local, and the Local dir keeps its trailing slash.

  The **single-token** rule (`parser.rs:983-998`, exact `bin/console` → `php/bin-console`, otherwise `dotslash`) is
  a different rule. It stays in the parser and must never be merged into `spec_location`.
- **`loc_key`** (`parser.rs:208-213`) produces `global,,{name}` or `local,{dir.display()},{name}`. It is the key of
  the spec cache and of `by_json` (245), and part of the prefix-cache key (1031-1038). `PathBuf` `Eq`/`Hash` treat
  `/p/` and `/p` as equal, so never key a cache on `SpecLocation`. The tests do rely on its derived component-wise
  `PartialEq`.
- **`apply_versions`** (393-404):
  - it applies **every** diff whose key is ≤ the target, cumulatively, in semver order; `version_pick` needs 8.10.0
    to carry 8.2.0's subcommands;
  - a `None` or unparsable target applies all of them;
  - unparsable keys sort as 0.0.0;
  - the sort is stable over serde_json's `BTreeMap` order (no `preserve_order`).

  The plan's "newest ≤ target" is wrong.
- **`semver`** (513-531):
  - it trims and strips leading `=`/`v` runs;
  - `+` starts build metadata and `-` starts the prerelease;
  - main parts reject leading zeros and `u64` overflow.

  **`semver_cmp`** (543-560): a release sorts after its prereleases, numeric identifiers sort before alphanumeric
  ones, and `1.0.0-01` equals `1.0.0-1`. Never derive `Ord`.
- **Error texts reach three sinks as an anyhow `{:#}` chain:**
  - the engine log `engine: specs: {e:#}`;
  - setup's `error: could not extract the completion specs: …`;
  - doctor's `Failed to fix: {e:#}` (`doctor.rs:156` at `0a0a74e`).

  The contexts are:
  - `creating {root}` (`specs.rs:53`);
  - `extracting specs into {tmp}`, with the **temp** path (57);
  - `renaming specs into {dir}` (65);
  - `reading {path}` (156);
  - `parsing {path}` (158);
  - the home error from `SharedError`;
  - the no-embed message.

  An `io::Error` from `tar` carries a nested cause (`failed to unpack …: <io error>`), so the rendering must walk
  `source()`. The golden capture covers none of this (it excludes `.tabsmith/specs`, and runs neither setup nor a
  doctor fix). Phase 4's `integrations-diff.sh` runs setup and doctor's spec fix on the success path only (its diff
  against the baseline binary is empty at `0a0a74e`; 5.2b runs it, and 5.5 again on the final commit). Three things
  guard the failing path:
  - the new chain tests;
  - Phase 4's bridge test `should_keep_the_whole_chain_of_a_failed_extraction`;
  - from 5.2b on, one run per binary (`target/hexagonal-refactor/baseline-tree/target/debug/tabsmith` and
    `target/debug/tabsmith`) in a fresh sandbox: `env -i`, `HOME=$SB/home`, `TABSMITH_HOME=$SB/data`,
    `PATH=/usr/bin:/bin`, a regular file at `$SB/data/specs`, stdin `/dev/null`. On both binaries,
    `setup --no-confirm` must print
    `error: could not extract the completion specs: creating $SB/data/specs: File exists (os error 17)`, and then
    `doctor` must print `Failed to fix: creating $SB/data/specs: File exists (os error 17)`. Both did at `0a0a74e`.
- **`#[cfg(feature = "embed-specs")]`** guards `ARCHIVE`, the ensure branch, `testutil` and `embedded_index_shape`.
  Neither the gate nor CI builds `--no-default-features`. `cargo check --locked --no-default-features --all-targets`
  passes at `a3953cf`; run it after 5.2b.
- **Test support.** `bundle_subset` matches `strip_prefix(".")` paths exactly and asserts `unpack_in() == true`.
  `full_bundle` extracts with the production `extract_to` into version `x` and returns the **root**. CI runs
  `cargo test --locked --release -- --ignored all_bundle_modules` (`ci.yml:23`): that name must survive, and
  `full_bundle` must stay reachable from wherever the test lands. `embedded_index_shape` hard-codes 1484 completions,
  6 diff-versioned and `heroku`. `TempDir` creates its dir eagerly and the extract tests assert exact listings of it
  (`specs.rs:301-305, 337, 352, 359`), so no helper may create anything inside it.
- **Include paths are relative to the file.** From `src/specs/infrastructure/adapters/` they are
  `../../../../assets/…`. Keep the `.trim()` (the file holds `2.692.3\n`). Never leave a second
  `include_bytes!` of the archive in a shim: debug builds would embed it twice.

### Runtime and process execution (5.3)
- **One id space.** `Host::id` (`js.rs:186-189`) numbers tasks, JS exec ops and sleep ops. `finish` routes by
  `live` membership, and anything else goes to `__ts.settle` (581-602). Never split the counter.
- **Exec results arrive asynchronously, through the channel.** That covers bad exec input and a failed thread
  spawn, which are never finished inline (`js.rs:203-208, 276-281`): JS registers its resolver only after
  `__ts_exec` returns (PRELUDE 154). `__ts_exec`'s order is:
  1. borrow;
  2. allocate the id;
  3. look up the ctx by the callback's task id, `unwrap_or_default()`;
  4. only then `serde_json::from_str`.
- **`ExecInput`'s serde error texts.**
  - `__ts_exec` uses `from_str`, so its error carries ` at line L column C`. The engine uses `from_value`
    (`script_input`), which does not. Keep each caller's own parse entry point.
  - serde builds `expected struct ExecInput` and `struct ExecInput with 5 elements` from the **Rust identifier**
    (serde_derive 1.0.229 `de.rs:167`, `de/struct_.rs:46, 439`). `#[serde(expecting)]` cannot reproduce the
    sequence form, and JS code can reach it with `__ts_exec(1, "[]")`. So the record keeps the identifier
    `ExecInput` and the exact attributes: `command` required; `#[serde(default)]` on args, cwd, env and timeout;
    env `null` removes the variable; `timeout: Option<u64>`; unknown fields ignored.
- **Abort semantics** (`js.rs:466-509`):
  - `wait` checks only the **deferred** queue for a `Complete`, and does so **before** pumping (472-478). A Complete
    still unread in the channel does not abort a wait whose tasks finish in the first pump.
  - It receives **one** message per loop turn: a runtime event is handled, any other message is pushed to the
    **back** of `deferred` and the loop restarts at the abort check; Timeout does nothing; Disconnected returns
    `collect`.
  - The receive timeout is `next.saturating_duration_since(now)`, where `next = min(earliest timer, deadline)` and
    `now` is read right after the pump.
  - Once a Complete is deferred, every wait aborts at once until the engine's `next_msg` pops it.
- **`next_msg`/`try_next_msg`** (329-354) return deferred messages first, FIFO, then `recv`/`try_recv`. An
  `ExecDone` read there is handed to the runtime **immediately** (`internal` → `finish`, a table insert or
  `__ts.settle`, which runs JS). It is never queued for a later wait.
- **The inbox never disconnects.** The runtime holds a sender clone for its whole life (`Host.tx`), so the engine
  loop ends only on `Shutdown` (`Engine::handle`). The test doubles must also keep a sender, or `wait` would return
  `collect` early on Disconnected.
- **`EngineMsg::Timer` is never constructed.** It appears only in the `EngineMsg` variant (types.rs), in `internal`'s
  arm (`js.rs:569`, plus the comments at 330, 563 and 570) and in `Engine::handle`'s no-op arm (engine.rs).
- **Partial delivery** (`js.rs:481-489, 635-649`). A task counts as finished when it is done or not live. `collect`
  delivers a result once (it removes live + ctxs), returns `None` for a pending task and
  `Some(Err("unknown task {t}"))` for an unknown or forgotten one. `Aborted` returns before `collect`, so nothing is
  lost.
- **`call` vs `cli_version`.** `call` (511-532) forgets the task on Timeout and on Aborted, and maps
  `Some(Err(e))` to `Other(e)` without forgetting. `cli_version` (`parser.rs:371-376`) uses start/wait and does
  **not** forget on Aborted (the inflight entry survives).
- **CPU limits and timers** (`js.rs:611-633`):
  - `stop = deadline.max(now + MIN_PUMP)`, and each job runs with an interrupt at `max(stop, now + SLICE)`;
  - start and settle arm `now + SLICE`, and load arms `LOAD_TIMEOUT`; `arm_off` runs after every JS entry; the
    interrupt check is strict (`Instant::now() > d`);
  - due timers are drained in **insertion** order and fired before jobs run, and `clearTimeout` only drops the JS id;
  - `Instant::now() + Duration::from_millis(ms.max(0.0) as u64)` (292) panics for `setTimeout(fn, Infinity)`.
    Move it as is.
- **PRELUDE runs exactly once per Context** (313): its top-level `let`/`const` would throw on a second eval. Both
  the prelude and modules use the default `EvalOptions`. `wrap_module`'s output calls `__ts.json`. PRELUDE calls the
  four host functions registered in `Js::new`, so the protocol spans the two files: keep every name.
- **`__ts.fns` only grows.** A `SpecFunctionRef` is an index into it, so there is one runtime per engine thread for
  its whole life. Only `all_bundle_modules` recreates it, on purpose, every 100 modules.
- **`load_module`** (363-407):
  - a cache hit requires a successful stat with a matching `(mtime, len)`;
  - errors are never cached, and an empty file caches an empty map;
  - error texts are `{path}: {e}` and `{path}: no trailing export clause`;
  - after any failure, Timeout is chosen when `Instant::now() >= deadline`.
- **`start_call`** (410-451) passes ctx JSON `{"id", "exec": [arg indexes]}`, plus `cwd` only when it is non-empty.
  `Null` sits at each Exec position. A synchronous start failure is stored as the task's result, not returned, so
  `start` stays infallible.
- **`__ts_done`** (298-312) drops results for tasks that are not live. A payload that fails to parse becomes
  `Err(serde message)`.
- **Exec results reach JS and the engine as `to_value(ExecOutput)`** (567): exactly `{status, stderr, stdout}`,
  with sorted keys. A hand-built `json!` from `ProcessOutput` produces an identical `Value`.
- **Log texts, kept as they are:**
  - `js console.{level}: {msg}` (265);
  - `js settle {id}: {e}` (605);
  - `engine: js host: {e:#}` (engine.rs `spawn`);
  - the constructor errors `js prelude: {e}` and the rquickjs Display. `rquickjs::Error` has no `source()`
    (`rquickjs-core-0.14.0` `result.rs:361`), so a typed error with that Display prints the same line under `{:#}`.
- **`SpecError` Display:** `aborted by a newer request`, `timed out`, or the raw text with no prefix. It derives
  `Clone`, `Debug` and `PartialEq`, because the tests `assert_eq!` on `Err(Aborted)`. `ParseError::NoSuggestions(
  e.to_string())` is logged as `engine: no suggestions: {e}` (`Engine::compute`), and
  `git_alias_directive_with_fake_git` matches `Failed parsing alias`.
- **There are two script timeouts (D1).** The runtime reads `Settings::i64` (`js.rs:212-214`) and the engine reads
  f64 (engine.rs `script_timeout`). `set_settings` updates only the runtime floor, and running exec workers keep the floor
  they captured at spawn. The exec timeout is `default.max(input.timeout.unwrap_or(0))` (783).
- **There are two generatorContext builders (D1).** `engine::gen_context_json` (production, env as is, `{}` when the
  env is empty) and `GenCtx::context_json` (test-only, falls back to the process env when the env is empty). Same
  six keys, sorted.
- **Startup.** QuickJS is `!Send`: build the inbox, the sink and the runtime inside the `engine` thread closure,
  before `Engine::new`. Keep the field order `ctx` then `_rt`, so the Context drops first.
- **`exec`** (692-788):
  - cwd is the first non-empty `is_dir` of `[input.cwd, ctx.cwd]` (a relative `input.cwd` is checked against the
    process cwd), else `current_dir`, else `C:\` or `/`;
  - native `ls` instead joins `input.cwd` under `ctx.cwd`;
  - the short-circuits run in this order: `ls` with trimmed args `["-1ApL"]` goes to `native_ls` (env ignored), then
    `fig` or `tabsmith` gives status 1;
  - child env:
    1. `env_clear` + the session env, only when it is non-empty;
    2. remove the color variables;
    3. set `PROCESS_LAUNCHED_BY_TABSMITH=1`, `HISTFILE=''`, `HISTCONTROL=ignoreboth`, `TERM=xterm-256color`,
       `NO_COLOR=1`;
    4. apply `input.env` (`None` removes);
    5. set `NoDefaultCurrentDirectoryInExePath=1` when the program's stem is `cmd`, on unix too.
  - variable lookups use the `input.env` entry first, even when its value is `None`. The env maps stay `HashMap`s.
- **`find_exe`** (834-869) returns absolute names without checking they exist and resolves multi-component names
  under the cwd. On Windows it tries the bare name only when it has an extension, then PATHEXT. On unix it requires
  `is_file` and an exec bit. Its error texts are `failed running executable ({cmd}): not found` and `…: {e}`. It is
  **not** deduped with the shared `which` (D1).
- **`wait_output`** (906-971):
  - it returns only once the status is known **and** both pipes have hit EOF;
  - a signal exit becomes status 1;
  - it polls every 5 ms;
  - on timeout it kills the pgid, kills the job, then `child.kill` + `wait`, and reports
    `timed out after {timeout_ms} ms running executable ({label})`;
  - `_live` is declared after `job`, so it leaves `LIVE` before `CloseHandle`.
- **`kill_live_children`** (1008-1020) is a stateless free function over the static `LIVE`. The `extern "C"` atexit
  hook (986-993) calls it, and so do pty.rs's `fallback` and `Session::cleanup` (its two
  `crate::js::kill_live_children()` calls). It must stay a free function.
- **Everything else in the executor is copied unchanged.** `clean` (1079-1086) is lossy, replaces `\r\n` first,
  removes `ESC[?25h` and trims `\n`. `native_ls` (1088-1113) keeps `read_dir` order, follows symlinks for the `/`
  suffix, skips unreadable entries, joins with `\n` and has no trailing newline. Its error is `ls: {dir}: {e}` with
  status 2. The worker thread is named `tabsmith-exec`.
- **`exec_children_die_with_the_wrapper` re-executes the test binary with `--exact NAME`** (1638). Its new NAME is
  `specs::infrastructure::adapters::process_executor::tests::exec_children_die_with_the_wrapper` (D9). Otherwise the
  child runs 0 tests. The same holds for every `cargo test NAME -- --exact` run, such as 5.3d's guard loops: a bare fn
  name matches nothing and prints `running 0 tests … ok`.
- **Windows code is never compiled locally.** Hand-check every renamed accessor and every path in
  `build_command`'s `cfg(windows)` block, `git_bash`, `mod job`, `wait_output`, `kill_live_children`, `exec` and
  `is_executable`, and in `timeout_kills_grandchildren`. The reverse holds too: a `cfg(unix)`-only item breaks the
  Windows CI job when it is imported without the same cfg. `fake_bin` becomes `cfg(all(test, unix))` in 5.3b, so
  the runtime tests import it as `#[cfg(unix)] use …::process_executor::fake_bin;`.
- **RefCell scoping.** Today `start_call` drops its borrow before `ctx.with` (412-418 vs 437), and so do `finish`
  (581-588 vs 594) and `pump` (613-618 vs 624). The new borrows must follow the same rule:
  - the module cache is never borrowed across eval;
  - the inbox's deferred queue is never borrowed across `handle_event`;
  - there is no outer `RefCell` around the runtime. The engine passes the runtime down through compute → parse, so a
    borrow held across the parse would panic.

### Repository and parser seam (5.4)
- **`disableForCommands`** (`parser.rs:236-243`) is checked on **every** location load: first, before dev mode and
  before the cache. It returns `Ok(None)`, caches nothing, and compares the location name (the base name for a
  Local). It reaches:
  - the single-token load;
  - every `parse_prefix` location (a `None` moves on to the next location);
  - nested `loadSpec`/`isCommand`/`isScript`/`isModule`;
  - `Parser::load_spec`.

  It stays inside `find` (D1), stated once in the domain as `SpecLookup::disables` (P13).
- **Dev mode** skips cache **reads** (246, 287) but still **writes** both the specs map and the CLI-version memo
  (272, 291-293). `by_json` is consulted in dev mode too, which is what keeps the `Rc` across dev reloads.
- **Only "not found" outcomes are cached, as `None`:**
  - no file (249-252);
  - a default export that is neither `$fn` nor an object (259);
  - a versioned `default()` result with no string `versionedSpecPath` that is not an object (300-302).

  Errors are never cached (253, 256, 265). The `$fn` arm (255) comes before the object arm (258).
- **`by_json`** is keyed by the location key and compares raw `Value`s (262-268). It is never cleared or bounded.
  A cache hit returns the stored `Rc` clone.
- **`clear_caches`** (126-132) clears only `loader.specs` and the `ParseCaches` prefix/generated maps. It keeps
  `first`, `versions`, `inflight` and `by_json`. It runs on **every** `EngineMsg::Settings` (the `EngineMsg::Settings` arm of `Engine::handle`;
  `engine.rs:427-433` at `0a0a74e`).
- **`versioned`** (276-321):
  - the memo key is `cliVersion-{name}`;
  - an unknown version is passed as JSON `null`;
  - `default()` is called with exactly `[Json(arg)]`, `Some(generator)` and `TIMEOUT`;
  - the diff file is the sibling `<last segment of vsp>.js`;
  - the diff target is the **result's** `version` field;
  - the error texts are `default is not a function` and `{file}: invalid versioned spec file`.
- **`cli_version`** (347-388):
  - `@usermn/sdc` gives `Ok(None)`;
  - `inflight` is keyed by the plain name, and `or_insert_with` reuses a running task;
  - the probe is getVersionCommand(exec) with `Some(generator)`, else an exec of `<location name> --version` in the
    generator's cwd and env;
  - it waits with `now + TIMEOUT`;
  - on Aborted it returns **without** remove/forget; on any other outcome it removes the entry and forgets the task;
  - a getVersionCommand string is used raw, and from an exec only `stdout` goes through `semver_clean`;
  - failures are not memoised.
- **`spec_file`** (326-344) expands a Local `~` only here, through `user_home_from_env().ok()?`. A missing home
  makes the spec missing. It tries `<dir>/.fig/autocomplete/build/<name>.js`, then `<name>/index.js`, with
  `is_file`. The Global branch calls `resolve` with the context's settings and index on every call.
- **`TIMEOUT`** (5 s, `parser.rs:27`) is shared by versioned `default()`, the probe wait and `JsHost::call`. It is
  separate from `LOAD_TIMEOUT` (5 s) and from the engine's `SYNC_CALL` (1 s).
- **The Loader maps are unbounded.** `MAX_CACHED` applies only to the `ParseCaches` prefix and generated maps.
- **`parse_prefix`** (1017-1152) tries locations in order. `host.load(loc)?` propagates an error **immediately**;
  `None` moves on to the next location. If nothing loads, the error is `failed loading spec for {t}`.
- **The parser test Mock** (`parser/tests.rs:77-118`):
  - it records **every** load before the lookup and never caches;
  - it returns the exact `Rc` it was given and never errors;
  - it keys globals by name and locals by `"{dir.display()}|{name}"`;
  - it ignores `disableForCommands`.

  The tests assert exact load sequences and counts.
- **Per-parse generator context.** `gen_ctx` (198-205) builds it once per parse: cwd (lossy), env, shell name,
  `search_term` `""` and `is_dangerous` false. The loads and the parser callbacks share it.
- **Typed decoding must equal the legacy readings:**
  - alias: `as_str`, then the parser trims, and a non-string gives `alias did not return a string`;
  - function loadSpec: `load_spec_result`, with JS errors propagated;
  - cacheKey: Aborted propagates, while any other error or a non-string gives no key;
  - generateSpec: a JS error and a `normalise` error both become `NoSuggestions(text)`, logged at warn level as
    `generateSpec ({t}): {e:?}`.

### Cross-cutting
- **Module root.** `src/specs.rs` exists, so creating `src/specs/mod.rs` makes `mod specs;` ambiguous (E0761).
  Until the end of 5.2b, `src/specs.rs` stays the module file and declares the layer children right after its two
  `//!` lines; putting the declarations before them does not compile. The last commit of 5.2b, when only the three
  declarations are left, runs `git mv src/specs.rs src/specs/mod.rs` and keeps only those three lines, as
  `settings/mod.rs` and `history/mod.rs` do.
- **Local fitness check, every step from 5.1a to 5.4b.** The fitness test reports `src/specs.rs` whenever it exists
  (`misplaced_files`), so `specs` can be registered only once that file is gone, and until 5.5 nothing would check
  the new layers. So, like Phase 4's T56: before each commit (once `src/specs/` exists: `rust_files` panics on a
  missing folder), add `"specs"` to `REGISTERED_MODULES` locally, run `cargo test --locked architecture`, and revert
  that line (`git diff src/architecture_tests.rs` empty). Accepted lines, verbatim at `0a0a74e`:
  - until 5.2b's move,
    `src/specs.rs: outside a layer folder, where the layer rules cannot read its layer: a module is src/<module>/mod.rs plus its layers`;
  - in 5.1b only, each `serde_json` line of `domain/value_objects/generator_script.rs` that 5.1c removes, e.g.
    `src/specs/domain/value_objects/generator_script.rs:<n>: domain uses application, infrastructure, bootstrap, a legacy module, an external crate, an output macro or the logger: use serde_json::Value;`.

  Any other line is a defect of that step. Rule 6 ("Fitness rules 6-8" below) cannot fire on the legacy body before
  the move: `registered_module_violations` reads only the files under `src/specs/`, and `src/specs.rs` reaches the
  report only through `misplaced_files`, so the checker never reads its `use` lines and items.
  - From 5.1a on, every `src/specs/**/mod.rs` holds only `pub(crate) mod x;` lines (P7's temporary
    `#![warn(dead_code)]` is a one-line attribute and passes).
  - The `git mv` runs only once `src/specs.rs` holds nothing but its `//!` lines and the three declarations, and it
    keeps only the three lines, so legacy code never sits in a `mod.rs`. A `//!` line would pass too: the checker
    drops `//` comments before it reads a line (`without_line_comment`). From the move on, the run reports nothing.
- **The fitness test** exempts test-only items and sibling `tests.rs` from rules 1-3 (`#[cfg(test)]` and
  `#[cfg(all(test, ..))]` both count as test-only). Rule 2 forbids `anyhow`, serde, serde_json, `tar` and `zstd` in
  application. Rules 1-2 (with Phase 3's `LEGACY_MODULES`) forbid a registered module's domain and application to
  name `crate::types`, `crate::js`, `crate::engine`, `crate::parser`, … once `specs` is registered (5.5), so every type
  the specs domain/application names must come from `specs`, `settings` or `shared`. Infrastructure may still use a
  legacy module (rule 3 never flags it), and `LEGACY_MODULES` cannot list `specs`, so 5.5 greps by hand.
- **Fitness rules 6-8 (Phase 4) hold in test code too.**
  - Rule 6: every `mod.rs` at any depth of a registered module holds only `mod x;`/`pub(crate) mod x;` lines and
    one-line attributes. `pub mod`, `use`, `type` or any other item is flagged; 5.1d's temporary
    `#![warn(dead_code)]` passes.
  - Rule 7: `src/main.rs` holds no `use` item and no crate-root item besides attributes, `mod` declarations and
    `fn main`. Phase 5 only deletes `mod spec;` (5.1b) and `mod js;` (5.3c). Code calls by full path
    (`crate::shared::infrastructure::logger::warn`), never a root alias such as `crate::logger::warn`, which the
    `af1b304` source maps still write.
  - Rule 8: no `#[path]`, `include!`, `#[macro_export]` or `#[macro_use]` in a registered module or `main.rs`. It
    matches the macro name `include` exactly, so `embedded_spec_bundle.rs`'s `include_bytes!`/`include_str!` pass.
- **Test lock per phase.** The gate's MISSING check compares with `ef807c1` only, so a deleted test that Phases 1-4
  added would pass it. Step 5.1a tags the phase start (`git tag phase5-start HEAD`, local, never pushed). Every step
  runs `python3 scripts/hexagonal-refactor/testlock.py . phase5-start --diff`: MISSING must be 0 there too, and the
  CHANGED list is exactly this phase's restructurings. testlock also prints `DUPLICATE (n)`: the gate shows that line
  but does not fail on it. It is 0 at `0a0a74e` and must stay 0, because testlock keys tests by fn name, so a second
  test with the same name (a new name that already exists, or a moved test left in both files) hides one of the two.
- **`all_bundle_modules`** (plan section 8, per phase) runs at the end of 5.2b, of every 5.3 and 5.4 step, and of
  5.5: `cargo test --locked --release -- --ignored all_bundle_modules` must print `1 passed`. It evaluates every
  bundle module through the runtime and `full_bundle`, which those steps move.
- **A flaky gate test.** `integrations_zsh_powerlevel10k_prompt_wrapped` (shell_integration's
  `embedded_shell_scripts` tests) is pty-timing flaky under the gate's parallel load. It failed twice during Phase 4
  (`target/hexagonal-refactor/test-failures/`). When it is the gate's only failure, rerun it alone with
  `cargo test --locked shell_integration::infrastructure::adapters::embedded_shell_scripts::tests::integrations_zsh_powerlevel10k_prompt_wrapped -- --exact`
  (it must print `1 passed`), then run the full gate again. Never edit, ignore or skip it.
- **History is append-only.** Never amend, rebase, reset, `filter-branch` or force-push a commit (one Phase 4 agent
  used `filter-branch`). Fix forward with a new commit. Subjects are at most 72 characters. A longer one is reported,
  not rewritten, as Phase 4 did for its 16 (plan section 9, "History").

## Decisions

### P1 Module wiring and deletion order
- 5.1a adds `pub(crate) mod domain;` to `src/specs.rs`, after the `//!` lines; 5.1b adds
  `pub(crate) mod infrastructure;` and 5.2b `pub(crate) mod application;`.
- The last commit of 5.2b runs `git mv src/specs.rs src/specs/mod.rs`, once only the three declarations are left.
- Each legacy file goes as soon as its last importer has switched. Shims inside the phase are allowed, but none is
  needed:
  - `spec.rs` and `mod spec;` at the end of 5.1b;
  - `specs.rs`'s code at the end of 5.2b, where the file becomes `specs/mod.rs`;
  - `js.rs` and `mod js;` in 5.3c (`git mv` to `quickjs_spec_runtime.rs`).

  The plan deletes them in 5.5. Doing it earlier is an import-path change only.

### P2 Glossary renames
| Old name | New name |
|---|---|
| `Opt` | `SpecOption` |
| `Directives` | `ParserDirectives` |
| `FnId` | `SpecFunctionRef` |
| `LoadSpec` | `SpecLoadDirective` |
| `Repeat` | `Repeatability` |
| `StrOrFn` | `TextOrFunction` (variants `Str`/`Fn` kept) |
| `spec::Suggestion` | `SpecSuggestion` |
| `Trigger` | `GeneratorTrigger` |
| `Script` | `GeneratorScript` |
| `CacheCfg` | `GeneratorCachePolicy` |
| `Strategy` | `FilterStrategy` |
| `ItemType` | `SuggestionKind` |
| `JsError` | `SpecError` |
| `GenCtx` | `GeneratorContext` |
| `ExecInput` | `ExecRequest` (domain); the serde record keeps the name `ExecInput` |
| `ExecOutput` | `ProcessOutput` |
| `Js` | `QuickJsSpecRuntime` |
| `Host` | `TaskRegistry` (private) |
| `Loader` | private inside `QuickJsSpecRepository` |

`Subcommand`, `Arg`, `Generator` and `SpecLocation` keep their names, and enum variant names do not change.

Rename compiler-guided: change the `use` line, then fix each error, plus targeted path renames. Respect the
exclusions in "Model and Fig mapper". Legacy files take the new names; the consumers researcher's
import-alias idea was rejected, because the glossary governs every type name. Testlock's loose form ignores paths,
so `ItemType::File` → `SuggestionKind::File` shows as unchanged. Only bare type mentions show as CHANGED (a
justified rename).

### P3 `SuggestionKind` in the specs domain
- `types.rs:11-23` moves to `src/specs/domain/value_objects/suggestion_kind.rs` with derives `Clone, Copy, Debug,
  PartialEq, Eq, Hash`. The serde derives are dropped: they are unused.
- `from_fig` (`types.rs:25-40`) and `fig_type` become private functions of the mapper. Fig strings never reach the
  domain: 5.1a parks `from_fig` as a private free fn in `spec.rs` (its only caller, `spec.rs:259`), and 5.1b moves it
  into the mapper with the rest of `spec.rs`.
- Every user switches in 5.1a: types.rs, engine, suggest, inline, menu, icons, render and the pty tests. There is no
  `types.rs` alias, following the Phase 3 precedent (`types.rs` may carry shims across phases, but the glossary
  governs every type name, and the consumers researcher's alias idea would keep `ItemType` alive until Phase 11).
- Phases 8 and 9: `render::describe` (`render.rs:597-613`) is the only production use that heads for
  terminal_session, which has no edge to specs. Put it in a dropdown domain service (dropdown → specs is allowed).

### P4 `SpecFunctionRef` stays a type alias
`pub(crate) type SpecFunctionRef = u32;` and `fn_id`'s `as_u64` + `as u32` rule stays as it is. The same number
still reaches `__ts.start`, and the literal ids in the spec and parser tests stay. A newtype would wrap about 15 test
literals for no behaviour.

### P5 `SpecValue` is an exact std-only tree. This overrides the judge's opaque text.
```rust
// src/specs/domain/value_objects/spec_value.rs
/// A JSON value written by spec code, held exactly as read until an adapter interprets it.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum SpecValue {
    Null, Bool(bool), Unsigned(u64), Negative(i64), Float(f64),
    Text(String), List(Vec<SpecValue>), Object(BTreeMap<String, SpecValue>),
}
// fig_spec_mapper.rs (named like setting_value_json.rs's setting_value_from_json / setting_value_to_json)
pub(crate) fn spec_value_from_json(value: &Value) -> SpecValue;   // numbers: as_u64, then as_i64, then as_f64 (`setting_number`, setting_value_json.rs:42-50)
pub(crate) fn spec_value_to_json(value: &SpecValue) -> Value;    // Value::from(u64 / i64), Number::from_f64 (always Some: serde_json holds no NaN or inf)
```
- `Value` → `SpecValue` → `Value` is the identity: integers stay PosInt or NegInt, floats keep their bits, and
  objects are a `BTreeMap` both ways.
- The judge's `SpecValue(String)` would re-parse floats inexactly (measured above).
- `GeneratorScript::Object(SpecValue)`. `engine.rs:795` (at `0a0a74e`, `Engine::begin`) passes
  `fig_spec_mapper::spec_value_to_json(o)` to `script_input`, and the rest of the engine's script path does not change.
- **Deviation** from the plan's "`Object(Value)` becomes a typed `ExecRequest`" (D1). Typing it at load time would
  move the `bad script input` failure from run time to load time, drop unknown keys from the cache-key text, and
  change the cwd override order.
- It gets its own number variants, not the settings `SettingNumber` (a script object is not a setting). See Open
  questions.

### P6 `SpecError` (one enum for the module, built up over the phase)
```rust
// src/specs/domain/spec_error.rs
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum SpecError {
    Aborted,                    // "aborted by a newer request"                                              (5.3a, ← JsError::Aborted)
    Timeout,                    // "timed out"                                                               (5.3a)
    Invalid(String),            // the text as is: "spec is not an object", load errors, JS failures, …     (5.1b; ← JsError::Other in 5.3a)
    BundleUnavailable(String),  // the text as is: the bundle failure with its causes, as anyhow's {:#} printed it (5.2b)
}
// hand-written Display (no prefix) + impl std::error::Error {} (no source)
```
- `normalise` returns `Err(SpecError::Invalid("spec is not an object".into()))`.
- There is no `NotFound`: `find` returns `Option` (P13).
- `BundleUnavailable` carries the chain **rendered** by the adapter as `context: cause: cause…`, through Phase 4's
  shared `crate::shared::infrastructure::error_chain::display_chain`, which settings' `load_or_defaults` and
  shell_integration's `os_shell_environment` also call (never a third hand-written walk). `{e}`, `{e:#}`,
  `anyhow::Error::from(e)` with `{:#}` and
  Phase 4's `SpecBundle(format!("{e:#}"))` all print the old text. This is the same shape as Phase 4's own
  `ShellIntegrationError::SpecBundle(message)`. It keeps `Clone + PartialEq` (a boxed source would lose both), and it
  needs no `EmbeddedSpecBundleError` enum.
- `From<SpecError> for ParseError`, with explicit arms (D8): `Aborted` → `Aborted`, and `Timeout | Invalid(_) |
  BundleUnavailable(_)` → `NoSuggestions(e.to_string())`.
- The engine's catch-alls get explicit arms with the same mapping (D8): the `_` after getQueryTerm (in `queries`), the
  `Err(_)` after a trigger (in `update_slots`) and the `Err(e)` in `pump`.
- Variant naming is a deviation: the plan has `Aborted, Timeout, NotFound, Invalid`. `Invalid` follows the plan and
  the judge. It renames `JsError::Other`, so about 3 js tests show as CHANGED. See Open questions.

### P7 Spec model fields are private, with accessors and plain builders (5.1d-5.1e)
5.1d covers the leaf structs (`SpecSuggestion`, `Generator`, `GeneratorCachePolicy`, `ParserDirectives`); 5.1e covers
`Arg`, `SpecOption` and `Subcommand`, plus `merged_with_generated`. The enums have no fields to hide.
- **Accessors** are named after the fields. They return `&FieldType` for non-`Copy` fields (`&Vec<Rc<Arg>>`,
  `&Option<String>`, `&HashMap<..>`) and the value for `Copy` fields. So every call site changes mechanically from
  `x.f` to `x.f()`.
  - The one exception is `Arg::default_value()`, which is `#[cfg(test)]`: only spec test 692 reads it.
- **Builders** are `with_<field>(mut self, v) -> Self` on top of `Default`. They are plain setters and never
  reindex.
  - `Subcommand::with_directives(d)` also sets `has_directives = true`, because the only writer always sets both.
  - `reindex` stays `pub(crate)`, and the mapper calls it where `spec.rs:337` does.
- **`Subcommand::merged_with_generated(&self, generated: &Subcommand) -> Subcommand`** takes `parser.rs:1276-1293`
  as is: the shallow clone, `merge<T>` as a private fn, args only if empty, and the reindex. The parser then runs:
  ```rust
  let args_were_empty = st.completion.args().is_empty();
  let c = st.completion.merged_with_generated(&generator);
  if args_were_empty { st.sub_args = ArgState::new(c.args()); }
  ```
  The `st.persistent` and `set_completion` lines that follow stay as they are.
- **Literal sites switch to builders with the same field values:**
  - production: `SpecSuggestion::from_script_output`, `suggest.rs:1303` (`list_dir`: evaluate
    `insert_value: Some(n.clone())` before `names: vec![n]`) and `suggest.rs:1338` (`help_items`), lines at `0a0a74e`;
  - tests: `suggest.rs` 1495-1514, 1757, 1761, 1798, 1811-1837, 1855-1863, 2382-2394, 2541-2544, and spec test
    657-662.
- **Test-only accessors.** Find them with a temporary `#![warn(dead_code)]` in `src/specs/domain/mod.rs`, mark them
  `#[cfg(test)]` (D7), and remove the attribute.
- A script can generate the accessors and builders from the struct bodies. This is the brief's "domain fields
  private". The cost is about 180 read sites in Phase 6 files. See Open questions.

### P8 `fig_spec_mapper.rs` is a module of free functions
This follows the `setting_value_json.rs` precedent; there is no state.

| Functions | Origin | Step |
|---|---|---|
| `normalise`, `suggestion_from_json`, `load_spec_result`, `fn_id`, `template_json` | `spec.rs`, `suggest.rs` | 5.1b |
| private `from_fig`, `fig_type` and every `spec.rs` reader | `types.rs` (via `spec.rs`, 5.1a), `suggest.rs`, `spec.rs` | 5.1b |
| `spec_value_from_json`, `spec_value_to_json` | new | 5.1c |
| `truthy`, `script_exec_input`, `json_items`, `post_process_items` | `engine.rs` | 5.1c |
| `generator_context_json(&GeneratorContext)` | `engine.rs` `gen_context_json` | 5.3a |
| `function_result(kind, &Value, &GeneratorContext) -> SpecFunctionResult` | new | 5.3f |

The `ExecInput` serde record and `process_output_json` live in their own adapter file, `exec_input.rs` (5.3a), as the
judge placed them.

- Every function is `pub(crate)` unless listed as private.
- `script_string` becomes the domain constructor `SpecSuggestion::from_script_output(text: &str, is_dangerous:
  bool)`, with the same field values. `post_process_items` and `engine.rs:903` (at `0a0a74e`, `exec_output`'s splitOn
  path) call it.
- The tests stay inline: about 400 lines.
- Moving the engine's JSON halves in 5.1 touches Phase 6 files early. That is allowed, because legacy files may
  import infrastructure.

### P9 Location and versions (5.2a)
- **`SpecLocation`** stays an enum with derives `Clone, Debug, PartialEq`. It gains `cache_key(&self) -> String`,
  taken from `loc_key` unchanged and written test-first: `global,,git` and `local,/cwd/dir/,test`. `loc_key` is
  deleted. Never derive `Hash`.
- **`SpecIndex`** is a VO with private fields, `new(completions, diff_versioned)`, `completions() -> &[String]` and
  `diff_versioned() -> &HashSet<String>`. It keeps derives `Clone, Debug, Default`.
- **`spec_locator.rs`** (domain service, free functions):
  - `spec_location(name, cwd, is_script)`, moved unchanged;
  - `find_global_spec(name, roots: &[PathBuf], index: &SpecIndex, is_file: impl Fn(&Path) -> bool) ->
    Option<PathBuf>`, which is `resolve_in`'s body with only the `is_file` probe injected. It keeps the
    short-circuit, so a diff-versioned name never asks about `<name>.js`. Written test-first with a set predicate.
  - The adapter keeps `resolve_in(name, roots, index)` as a one-liner over `find_global_spec(.., Path::is_file)`,
    so `resolve_order` and the `cfg(windows)` drive test move whole, as the fs guard.
  - **Deviation:** the plan's "split `spec_location_table` and `resolve_order`" is not done. `spec_location_table`
    is already pure, and splitting `resolve_order` would duplicate assertions.
  - The single-token rule and the Local lookup stay with the parser and the repository.
- **`semantic_version.rs`**:
  - `SemanticVersion` has private fields, `parse(&str) -> Option<Self>` (← `semver`) and `compare(&self, &Self) ->
    Ordering`, with the exact precedence and **no** derived `Ord`/`PartialOrd`;
  - the free functions `semver_clean` and `semver_cmp` (parse or 0.0.0) keep their names, so `semver_helpers` moves
    with only path changes.
- **`spec_version_selector.rs`**, test-first: `versions_to_apply<'a>(keys: impl IntoIterator<Item = &'a str>, target:
  Option<&str>) -> Vec<&'a str>`. It collects the keys, sorts them with a stable `sort_by(semver_cmp)`, and keeps the
  keys ≤ the target, or all of them when the target is `None` or unparsable. This is **cumulative**: the plan's
  "newest ≤ target" is a deviation, and the plan's section 4 wording gets fixed. `apply_versions` folds
  `merge_subcommand` over it, with `version_pick` as the guard.
- **`bundle_retention_policy.rs`**, test-first: `is_stale(name, current_version, age: Option<Duration>) -> bool`,
  holding `VERSION_STALE` and `TMP_STALE`. The adapter computes
  `age = metadata(.last-used).or_else(|_| metadata(dir)).and_then(|m| m.modified()).ok().map(|t|
  t.elapsed().unwrap_or_default())`, and keeps `read_dir`, `to_string_lossy`, `is_dir`, `remove_dir_all` and every
  swallowed error. There is no Clock port: today's code reads `SystemTime`.

### P10 Bundle adapter, port and use case (5.2b)
```rust
// application/ports/spec_bundle.rs
pub(crate) trait SpecBundle { fn install(&self) -> Result<(), SpecError>; }
#[cfg(test)] pub(crate) struct RecordingSpecBundle { /* installs: Cell<usize>, result: Result<(), SpecError> */ }
// application/install_spec_bundle_use_case.rs
pub(crate) struct InstallSpecBundleUseCase { bundle: Rc<dyn SpecBundle> }   // execute(&self) -> Result<(), SpecError>
// infrastructure/specs_module.rs
pub(crate) struct SpecsModule { pub(crate) install_spec_bundle: InstallSpecBundleUseCase }
pub(crate) fn build_specs_module(deps: &SharedDependencies) -> SpecsModule;
// infrastructure/adapters/embedded_spec_bundle.rs
pub(crate) struct EmbeddedSpecBundle { data_directory: DataDirectory }   // new(DataDirectory) never fails (D3)
impl SpecBundle for EmbeddedSpecBundle { fn install(&self) -> Result<(), SpecError> { self.ensure().map(|_| ()) } }
impl EmbeddedSpecBundle {
    fn ensure(&self) -> Result<PathBuf, SpecError>;                        // ensure_extracted unchanged, cfg split inside
    pub(crate) fn load_index(&self) -> Result<SpecIndex, SpecError>;      // SpecIndex::load unchanged, with its 2nd ensure
    pub(crate) fn resolve(&self, name: &str, settings: &Settings, index: &SpecIndex) -> Option<PathBuf>;
}
pub(crate) fn version() -> &'static str;
pub(crate) fn extract_to(archive: &[u8], root: &Path, ver: &str) -> Result<PathBuf, SpecError>;
pub(crate) fn load_from(bundle: &Path, user: &Path) -> Result<SpecIndex, SpecError>;
pub(crate) fn resolve_in(name: &str, roots: &[PathBuf], index: &SpecIndex) -> Option<PathBuf>;
// private: ARCHIVE, LAST_USED, touch, cleanup, unpack, dev_folder, and the chain renderer
#[cfg(all(test, feature = "embed-specs"))] pub(crate) mod testing { bundle_subset, full_bundle }
```
- **The chain renderer.** A private `fn unavailable(context: impl Display, cause: &(dyn Error + 'static)) ->
  SpecError` is `SpecError::BundleUnavailable(format!("{context}: {}", display_chain(cause)))`, over Phase 4's
  shared `display_chain` (precedent: `profile_from` in shell_integration's `os_shell_environment.rs`). It never walks
  `source()` itself (`io::Error`, `serde_json::Error` and `SharedError` are `'static`). The home error is
  `BundleUnavailable(display_chain(&error))`, with no context, and the no-embed text has no cause. Write it
  test-first, comparing it with `format!("{:#}", anyhow::Error::new(cause).context(text))` in the test (D2 allows
  anyhow in tests).
- **No `LoadSpecIndexUseCase`** (YAGNI). The legacy `Engine::new` keeps today's call shape, with a `ponytail:`
  comment saying that the second ensure inside `load_index` is today's behaviour (touch and cleanup twice):
  ```rust
  let dependencies = SharedDependencies::from_env();      // Phase 3's line, hoisted above the index build (5.4b: spawn)
  let bundle = EmbeddedSpecBundle::new(dependencies.data_directory.clone());
  let index = build_specs_module(&dependencies)
      .install_spec_bundle
      .execute()
      .and_then(|()| bundle.load_index())
      .unwrap_or_else(|e| {
          logger::error(&format!("engine: specs: {e:#}"));
          SpecIndex::default()
      });
  ```
  Hoisting is not observable: today `ensure_extracted` reads the same env at the same moment, and nothing in-process
  changes `HOME`/`TABSMITH_HOME`. The install goes through the module's use case, so it has one entry point and one
  wiring site (`build_specs_module`); the calls and their order are today's (install, then `load_index` with its
  own ensure). Phase 6's `engine_server` (bootstrap) does the same.
- **Doctor** (legacy until Phase 10):
  - it imports `embedded_spec_bundle::version`, and the check path keeps its own `ctx.data_root().join("specs")` join
    (`ctx` is Phase 4's `IntegrationPaths`; `specs_index` in `setup_checks`);
  - the fix runs `build_specs_module(&SharedDependencies::from_env()).install_spec_bundle.execute()` at fix time (the
    env is read then, as today) and converts with `?` into anyhow.
- **The Phase 4 bridge** (Phase 4's hand-off, plan section 9).
  - `SpecBundleBridge` holds the `InstallSpecBundleUseCase` and gets a `new(install_spec_bundle)`.
  - `bootstrap/cli.rs`'s `shell_integration_module()` is the only place that builds the bridge. It binds
    `let dependencies = SharedDependencies::from_env();` once, builds the bridge from
    `build_specs_module(&dependencies).install_spec_bundle`, and passes `&dependencies` to
    `build_shell_integration_module`.
  - `install` is `self.install_spec_bundle.execute().map_err(|e| spec_bundle_failure(anyhow::Error::from(e)))`.
    `spec_bundle_failure` and its test `should_keep_the_whole_chain_of_a_failed_extraction` stay unchanged.
    Update the two comments this makes stale. `SpecBundleBridge`'s `(Phase 5 swaps in its use case)` becomes
    `(specs' InstallSpecBundleUseCase)`. In the test's doc comment, replace `` `ensure_extracted` fails with a chain ``
    with `The bundle's extraction (EmbeddedSpecBundle) fails with a chain that BundleUnavailable holds already
    rendered`. testlock drops comments, so the test still counts as unchanged.
  - `format!("{error:#}")` of a `BundleUnavailable`, which has no source, prints the adapter's `display_chain` text.
    So `error: could not extract the completion specs: creating <dir>: <io>` stays byte for byte. Check it with the
    sandbox run under "Error texts reach three sinks".
  - Building `SpecsModule` only constructs (D3). The data-directory env is now read at dispatch, not inside
    `install`, as the rest of setup has done since Phase 4 ("Env reads at dispatch", plan section 9). Nothing in that
    process sets an env var, and the paths still resolve when `install` runs.
- **The parser.** `spec_file`'s Global arm calls `EmbeddedSpecBundle::new(data_directory_from_env()).resolve(..)`
  per call: the same environment read at the same moment as today. `HOME` and `TABSMITH_HOME` never change
  in-process. Only `ZDOTDIR` and `TABSMITH_DEBUG_SHELL` are set, by `cli.rs`'s `debug_shell` (582-585 at `0a0a74e`).
  This is the 5.2b interim: 5.4b injects the bundle into `QuickJsSpecRepository` (P13).
- **Deviations:**
  - the branching stays in the adapter, because the fast path and `extract_to` share touch + cleanup and the
    `cfg(feature)` split must not reach application;
  - `SpecBundle` has only `install`.

### P11 Runtime: the judged design
The winning design takes the sequencing and the inbox from design C, the typed vocabulary in `specs/domain` from
design A, and two details from design B: the `ExecInput` identifier and separate decoders. The full design follows.
Where it differs from the judge's text, the difference is marked and explained.

Step cut (the judge's commits 3a-3j, regrouped into seven steps, each alone and gate-green):
- 5.3a: `GeneratorContext`, `ExecRequest`, `ProcessOutput`, `TaskId`, `SpecError::{Aborted, Timeout}`, the
  `ExecInput` record, `generator_context_json`, D8 arms (judge 3a).
- 5.3b: the engine-free pieces of `js.rs` move out: `process_executor.rs`, `native_ls.rs`,
  `quickjs_module_wrapper.rs` (judge 3c, 3d).
- 5.3c: `git mv` to `quickjs_spec_runtime.rs`, then `&self` and the `*_json` renames (judge 3e, 3f).
- 5.3d: `RuntimeEventSource` + `EngineInbox`, the riskiest commit (judge 3g-1, 3g-2).
- 5.3e: the typed vocabulary, the `SpecRuntime` port and `InMemorySpecRuntime` (judge 3h).
- 5.3f: `QuickJsSpecRuntime` implements `SpecRuntime`: `call_arguments`, `function_result` (judge 3i).
- 5.3g: the parser's `Host::call` becomes typed (judge 3j).

The judge's 3b (the engine's JSON readers into the mapper) is done earlier, in 5.1c, with the model.

Domain (`src/specs/domain/`, std only):
```rust
// value_objects/generator_context.rs   ← js::GenCtx (shell stays a String: Default must give currentProcess "")
#[derive(Clone, Debug, Default)]
pub(crate) struct GeneratorContext { cwd: String, env: HashMap<String, String>, shell: String, search_term: String, is_dangerous: bool }
impl GeneratorContext { pub(crate) fn new(cwd, env, shell, search_term, is_dangerous) -> Self; /* cwd() -> &str, env() -> &HashMap, shell() -> &str, search_term() -> &str, is_dangerous() -> bool */ }
// value_objects/exec_request.rs        ← js::ExecInput (no serde)
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct ExecRequest { command: String, args: Vec<String>, cwd: Option<String>, env: HashMap<String, Option<String>> /* None removes */, timeout: Option<u64> }
// value_objects/process_output.rs      ← types::ExecOutput
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct ProcessOutput { stdout: String, stderr: String, status: i32 }
// value_objects/task_id.rs             pub(crate) type TaskId = u64;   ONE id space: tasks, JS exec ops, sleep ops (5.3a)
// value_objects/task_outcome.rs        pub(crate) type TaskOutcome = Result<SpecValue, String>;   (5.3e, and below)
// value_objects/spec_function_call.rs  ← (FnId, Vec<JsArg>) built at each legacy call site
#[derive(Clone, Debug, PartialEq)]        // needs SpecSuggestion: PartialEq (an added derive)
pub(crate) enum SpecFunctionCall {
    QueryTerm { function: SpecFunctionRef, search_term: String },                       // engine.rs:586
    Trigger { function: SpecFunctionRef, new_token: String, old_token: String },        // engine.rs:626
    Alias { function: SpecFunctionRef, token: String },                                 // parser.rs:1166
    LoadSpec { function: SpecFunctionRef, token: String },                              // parser.rs:1193
    GenerateSpecCacheKey { function: SpecFunctionRef, tokens: Vec<String> },            // parser.rs:1239, NO exec
    GenerateSpec { function: SpecFunctionRef, tokens: Vec<String> },                    // parser.rs:1256
    Script { function: SpecFunctionRef, tokens: Vec<String> },                          // engine.rs:776
    Custom { function: SpecFunctionRef, tokens: Vec<String> },                          // engine.rs:795
    PostProcess { function: SpecFunctionRef, stdout: String, tokens: Vec<String> },     // engine.rs:897
    FilterTemplateSuggestions { function: SpecFunctionRef, suggestions: TemplateSuggestions }, // engine.rs:758, 914
}
impl SpecFunctionCall { pub(crate) fn function(&self) -> SpecFunctionRef; pub(crate) fn kind(&self) -> SpecFunctionKind }
// value_objects/spec_function_kind.rs  #[derive(Clone, Copy, Debug, PartialEq, Eq)] enum SpecFunctionKind { the ten kinds }
// value_objects/template_suggestions.rs
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum TemplateSuggestions { Listed(Vec<SpecSuggestion>) /* adapter emits template_json */, Returned(SpecValue) /* a templated custom result, passed back unchanged */ }
// value_objects/spec_function_result.rs  (decoded by the adapter with the legacy readers, moved unchanged)
#[derive(Clone, Debug)]
pub(crate) enum SpecFunctionResult {
    QueryTerm(Option<String>),                 // Value::String only (engine.rs:591)
    Trigger(bool),                             // engine truthy, NaN is falsy (engine.rs:128)
    Alias(Option<String>),                     // as_str, not trimmed (the parser trims, parser.rs:1173)
    LoadSpec(Option<SpecLoadDirective>),       // load_spec_result
    GenerateSpecCacheKey(Option<String>),      // as_str
    GenerateSpec(Result<Box<Subcommand>, String>), // normalise, boxed (clippy large_enum_variant); Err "spec is not an object"
    Script(ScriptResolution),                  // engine.rs:855-866 without the settings and generator floors
    Custom(CustomGeneratorResult),             // engine.rs:907-917
    PostProcess(Vec<SpecSuggestion>),          // post_process_items(v, context.is_dangerous())
    FilterTemplateSuggestions(Vec<SpecSuggestion>), // json_items
}
// value_objects/script_resolution.rs
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum ScriptResolution {
    Skip,                       // falsy, empty array or string, number, bool → Done([])
    Invalid(String),            // serde text; the caller prefixes "bad script input: "
    Ready { request: ExecRequest /* timeout = Some(object.timeout.as_u64().unwrap_or(0)); the caller still takes the max with script_timeout(settings) and g.script_timeout */, cache_key_source: String /* input.to_string() BEFORE the timeout is set */ },
}
// value_objects/custom_generator_result.rs
#[derive(Clone, Debug)]
pub(crate) struct CustomGeneratorResult { templated: bool /* first element has a "context" key, even null */, suggestions: Vec<SpecSuggestion> }  // new + accessors
// spec_error.rs: see P6.  repositories/spec_repository.rs and value_objects/spec_lookup.rs: see P13.
```
`SpecValue` is the tree from P5. The judge's `SpecValue(String)`, with its `Eq` derive, is replaced.
`GenerateSpec` boxes its `Subcommand`: unboxed it is about 448 bytes against `Script`'s 160, and clippy's default
`large_enum_variant` would add a fourth warning (the gate allows 3, the baseline has 3). The shape is forced by
clippy and changes no behaviour: the parser unboxes at the decode, and each uncached call still makes one new `Rc`.

Application (`src/specs/application/`):
```rust
// ports/runtime_event.rs (port type: the payload of RuntimeEventSource; it holds a VO, so it is not a DTO)  ← EngineMsg::ExecDone
#[derive(Debug)]
pub(crate) struct RuntimeEvent { pub(crate) id: TaskId, pub(crate) output: Result<ProcessOutput, String> }
// ports/runtime_event_sink.rs
/// Posts into the engine inbox from any thread. It ONLY enqueues: a result is never settled inline, because JS
/// registers its resolver after __ts_exec returns (js.rs:152).
pub(crate) type RuntimeEventSink = Arc<dyn Fn(RuntimeEvent) + Send + Sync>;
// ports/runtime_event_poll.rs
pub(crate) enum RuntimeEventPoll { Event(RuntimeEvent), MessageKept, TimedOut, Disconnected }
// ports/runtime_event_source.rs (the trait + its #[cfg(test)] double only, like settings_editor.rs)
pub(crate) trait RuntimeEventSource {
    fn sink(&self) -> RuntimeEventSink;
    /// A newer completion request was received and is kept for the engine. wait checks it BEFORE every pump.
    fn has_newer_request(&self) -> bool;
    /// Receives AT MOST ONE message within `timeout`. Any other message is kept, in arrival order, for the engine.
    fn poll(&self, timeout: Duration) -> RuntimeEventPoll;
}
#[cfg(test)] pub(crate) struct ChannelRuntimeEventSource { sender: Sender<Option<RuntimeEvent>> /* None = newer request; kept so it never disconnects */, receiver: Receiver<Option<RuntimeEvent>>, kept: Cell<usize> }
// new(), send_newer_request(&self), take_newer_request(&self) -> bool (replaces `next_msg() == Complete`)
// ports/interrupted.rs
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Interrupted;   // a newer request is waiting; no task was dropped
// ports/spec_runtime.rs (the trait + InMemorySpecRuntime)
pub(crate) trait SpecRuntime {
    fn start(&self, call: SpecFunctionCall, context: Option<&GeneratorContext>) -> TaskId;
    fn start_exec(&self, request: ExecRequest, context: &GeneratorContext) -> TaskId;   // context NOT registered for nested exec (js.rs:454)
    fn start_sleep(&self, ms: u64) -> TaskId;
    /// None = still running. Some is delivered once. An unknown or forgotten task gives Some(Err("unknown task {t}")).
    fn wait(&self, tasks: &[TaskId], deadline: Instant) -> Result<Vec<Option<TaskOutcome>>, Interrupted>;
    fn forget(&self, tasks: &[TaskId]);
    /// start + wait + decode by call.kind(). Forgets on Timeout and on Aborted, not on a failed call (js.rs:524-533).
    fn call(&self, call: SpecFunctionCall, context: Option<&GeneratorContext>, timeout: Duration) -> Result<SpecFunctionResult, SpecError>;
    fn apply_settings(&self, settings: &Settings);   // js scriptTimeout read as i64 (D1)
    // Added in Phase 6.4 with their first caller (ProgressGeneratorsUseCase):
    // fn read(&self, value: &SpecValue, kind: SpecFunctionKind, context: &GeneratorContext) -> SpecFunctionResult;
    // fn read_exec_stdout(&self, value: &SpecValue) -> String;     // v["stdout"] as str else "" (engine.rs:882), whatever kind wrote v
    // fn resolve_script(&self, script: ScriptValue<'_>, cwd: &str) -> ScriptResolution;  // ScriptValue { Argv(&[String]), Shell(&str), Value(&SpecValue) }
}
#[cfg(test)] type Answer = Box<dyn Fn(&SpecFunctionCall) -> Result<SpecFunctionResult, SpecError>>;   // inline, the field trips clippy type_complexity (parser/tests.rs `Callback` precedent)
#[cfg(test)] pub(crate) struct InMemorySpecRuntime { answers: HashMap<SpecFunctionRef, Answer>, calls: RefCell<Vec<SpecFunctionCall>>, running: RefCell<HashSet<TaskId>>, next_task: Cell<TaskId> }
// answering(self, f, answer) -> Self; calls() -> Vec<SpecFunctionCall>
// call: record the call, then answer it, or Err(Invalid("unknown fn")) as the parser test Mock does
// start*: allocate a running task; wait: None while running, else "unknown task {t}"; forget: drop it; apply_settings: no-op
// ponytail: scripted task completion is added by Phase 6.4 when the generator tests need it
```
One file per type (plan section 1): `RuntimeEvent`, `RuntimeEventSink`, `RuntimeEventPoll` and `Interrupted` get their
own files under `application/ports/`, and each trait file holds only its trait and its `#[cfg(test)]` double, as
`settings_editor.rs` does. `RuntimeEvent` is a port type, not a DTO: it holds the `ProcessOutput` VO.

The port's `wait` maps `Err(SpecError::Aborted)` to `Interrupted` and each JSON result through
`spec_value_from_json`. Any other error from `wait_json` can never happen: `js.rs:472-478` is its only `Err`. Give it
the same per-task mapping as `engine.rs:965`, `Ok(vec![Some(Err(e.to_string())); n])`, with explicit arms (D8). This
is a clarification of the judge's text.

Infrastructure (`src/specs/infrastructure/adapters/`):
```rust
// exec_input.rs
/// The ident MUST stay `ExecInput`: serde_derive writes "expected struct ExecInput" and
/// "struct ExecInput with 5 elements" from it, and spec code can reach them through __ts_exec.
#[derive(Deserialize)] pub(crate) struct ExecInput { command: String, #[serde(default)] args: Vec<String>, #[serde(default)] cwd: Option<String>, #[serde(default)] env: HashMap<String, Option<String>>, #[serde(default)] timeout: Option<u64> }  // same field order
impl From<ExecInput> for ExecRequest
pub(crate) fn process_output_json(output: &ProcessOutput) -> Value   // {"status","stderr","stdout"} == to_value(ExecOutput)
// quickjs_spec_runtime.rs (tests in quickjs_spec_runtime/tests.rs when over ~500 lines)
pub(crate) enum JsArg { Json(Value), Exec }            // wire argument
pub(crate) type TaskResult = Result<Value, String>;    // JSON level, for the legacy engine until 6.4
pub(crate) struct QuickJsSpecRuntime { ctx: Context, _rt: Runtime /* drop order kept */, tasks: Rc<RefCell<TaskRegistry>>, deadline: Rc<Cell<Option<Instant>>>, events: Rc<dyn RuntimeEventSource>, modules: RefCell<ModuleCache> }
type ModuleCache = HashMap<PathBuf, (Stamp, Map<String, Value>)>;   // private alias: inline, the RefCell'd field trips clippy type_complexity
struct TaskRegistry { next_id, live, done: HashMap<TaskId, TaskResult>, ctxs: HashMap<u64, GeneratorContext>, timers, sink: RuntimeEventSink, timeout_ms }  // ← js::Host
impl QuickJsSpecRuntime {   // JSON-level API: js.rs bodies unchanged, &self
    pub(crate) fn new(events: Rc<dyn RuntimeEventSource>, settings: &Settings) -> Result<Self, SpecError>;
    pub(crate) fn load_module(&self, path: &Path) -> Result<Map<String, Value>, SpecError>;
    pub(crate) fn start_call(&self, f: SpecFunctionRef, args: Vec<JsArg>, ctx: Option<&GeneratorContext>) -> TaskId;
    pub(crate) fn wait_json(&self, tasks: &[TaskId], deadline: Instant) -> Result<Vec<Option<TaskResult>>, SpecError>;
    pub(crate) fn call_json(&self, f: SpecFunctionRef, args: Vec<JsArg>, ctx: Option<&GeneratorContext>, timeout: Duration) -> Result<Value, SpecError>;
    pub(crate) fn handle_event(&self, event: RuntimeEvent);   // ← internal(ExecDone): finish(id, output.map(|o| process_output_json(&o)))
}
impl SpecRuntime for QuickJsSpecRuntime { /* start/wait/call wrap the JSON API; start_exec/start_sleep/forget/apply_settings hold the moved bodies (no inherent twin) */ }   // 5.3f; until then those four are inherent methods
pub(crate) fn call_arguments(call: &SpecFunctionCall, context: &GeneratorContext) -> (SpecFunctionRef, Vec<JsArg>);   // 5.3f
// fig_spec_mapper.rs: + generator_context_json (5.3a), + function_result(kind, &Value, &GeneratorContext) (5.3f).
//   Script: script_exec_input, then key = input.to_string(), then requested = input["timeout"].as_u64() and
//   input["timeout"] = json!(requested.unwrap_or(0)), then from_value::<ExecInput>.
// process_executor.rs: exec, key_eq, build_command, find_exe, is_executable, git_bash, wait_output, LIVE, live, Live,
//   mod job, clean (unchanged), kill_live_children (stateless free fn), #[cfg(all(test, unix))] pub(crate) fn fake_bin
//   (its runtime-test users are unix-only: they import it as `#[cfg(unix)] use …::process_executor::fake_bin;`,
//   or the Windows CI job fails with E0432)
// native_ls.rs: native_ls(&Path) -> ProcessOutput
// quickjs_module_wrapper.rs: wrap_module, PRELUDE; its doc comment states the __ts_log/__ts_exec/__ts_sleep/__ts_done
//   contract with the registry and that PRELUDE is evaluated once per Context
```
`LOAD_TIMEOUT`, `SLICE` (1000 ms), `MIN_PUMP`, `MEMORY_LIMIT` and the i64 `script_timeout` stay private in the
runtime file. The log texts and the thread name `tabsmith-exec` do not change.

Legacy glue (unregistered files):
```rust
// engine.rs (moves to bootstrap/engine_server.rs in Phase 6.6)
struct EngineInbox { receiver: Receiver<EngineMsg>, sender: Sender<EngineMsg>, deferred: RefCell<VecDeque<EngineMsg>> }
impl EngineInbox {
    fn new(receiver: Receiver<EngineMsg>, sender: Sender<EngineMsg>) -> Self;
    fn next_msg(&self, runtime: &QuickJsSpecRuntime) -> Option<EngineMsg>;      // js.rs:331-342 unchanged; Runtime(e) → runtime.handle_event(e), loop
    fn try_next_msg(&self, runtime: &QuickJsSpecRuntime) -> Option<EngineMsg>;  // js.rs:344-354 unchanged
}
impl RuntimeEventSource for EngineInbox {
    // sink = Arc::new(move |e| { let _ = sender.send(EngineMsg::Runtime(e)); })
    // has_newer_request = deferred.iter().any(Complete)
    // poll = one recv_timeout: Runtime(e) → Event; m → push_back(m), MessageKept; Timeout → TimedOut; Disconnected → Disconnected
}
// types.rs: ExecOutput deleted; EngineMsg::ExecDone{..} → EngineMsg::Runtime(RuntimeEvent); EngineMsg::Timer deleted
// parser.rs: Host::call(&mut self, call: SpecFunctionCall) -> Result<SpecFunctionResult, ParseError> (5.3g)
```

**Call-site mapping** (`a3953cf` → Phase 5). The engine.rs sites cited here and in the blocks above keep their text at
`0a0a74e`, 3 lines lower above `struct Engine` and 12-13 lines lower after it (586 → 599, 965 → 978). Earlier Phase 5
steps move them again, so find each one by its text.

| Legacy site | Phase 5 | Step |
|---|---|---|
| `js.rs:32-49` `JsError` | `SpecError` (`Other` → `Invalid`), same Display | 5.3a |
| `js.rs:51-60` `GenCtx` | `GeneratorContext::new` + accessors | 5.3a |
| `js.rs:62-90` `context_json` | `#[cfg(test)] fn context_json(&GeneratorContext)` in the runtime tests (D7); only `custom_generator_executes_commands` uses it | 5.3a |
| `js.rs:101-113` `ExecInput` | infra `ExecInput` (identifier kept) + domain `ExecRequest`; `js.rs:274` and engine.rs `script_input` keep their own `from_str` / `from_value` | 5.3a |
| `types::ExecOutput` | `ProcessOutput`; `exec`, `native_ls` and `wait_output` return it | 5.3a |
| engine.rs `gen_context_json` | `fig_spec_mapper::generator_context_json` | 5.3a |
| engine.rs: `compute`'s `GenCtx { .. }` literal; every `ctx.`/`run.ctx.` field read (`update_slots`, `begin`, `script_input`, `exec_output`, `advance`) | `GeneratorContext::new(..)`; accessors | 5.3a |
| `parser.rs:88-95` `From<JsError>` | `From<SpecError>`, D8 arms | 5.3a |
| `parser.rs:198-205` `gen_ctx` | `GeneratorContext::new(cwd lossy, env.clone(), shell.name().into(), String::new(), false)` | 5.3a |
| `js.rs:692-1113` exec … `native_ls` | `process_executor.rs`, `native_ls.rs` | 5.3b |
| pty.rs `fallback` and `Session::cleanup`: `crate::js::kill_live_children()` | `crate::specs::infrastructure::adapters::process_executor::kill_live_children()` | 5.3b |
| `js.rs:127` PRELUDE, `655` `wrap_module` | `quickjs_module_wrapper.rs` | 5.3b |
| `js.rs:216` `Js`, `171` `Host`; `main.rs` `mod js;` | `QuickJsSpecRuntime`, `TaskRegistry`; `mod js;` removed; engine.rs, parser.rs and parser/tests.rs switch paths | 5.3c |
| `&mut self` throughout; `wait`, `call`, `set_settings` | `&self`; `modules` in a `RefCell` (bind a cache hit to a local before eval, insert after); `wait_json`, `call_json`, `apply_settings(&Settings)` | 5.3c |
| engine `js: &mut Js` parameters (Phase 4's test helper `engine_with_wrapper_env` returns the runtime; `report_session_context` keeps a `&mut` parameter, see "Test moves"); `fetch`'s `start: impl FnOnce(&mut Js) -> TaskId` | `js: &QuickJsSpecRuntime`; `impl FnOnce(&QuickJsSpecRuntime) -> TaskId` | 5.3c |
| engine.rs `js.call(` in `queries` and `update_slots`; `js.wait(` in `pump`; `js.set_settings(s.clone())` in `Engine::handle` | `call_json`, `wait_json`, `apply_settings(&s)` | 5.3c |
| `js.rs:178, 191-208` `Host.tx` + spawn_exec, `277-282` bad exec input | `(sink)(RuntimeEvent { id, output })`, still asynchronous | 5.3d |
| `js.rs:466-509` wait: `deferred.any(Complete)`, `rx.recv_timeout(d)` | `events.has_newer_request()`, `events.poll(d)` (`Event` → `handle_event`; `MessageKept`/`TimedOut` → loop; `Disconnected` → `collect`); nothing else changes | 5.3d |
| `js.rs:329-354` `next_msg`/`try_next_msg` + `deferred`, `rx`; `564-578` `internal` | `EngineInbox` (engine.rs); `handle_event(RuntimeEvent)`, with the Timer arm dropped | 5.3d |
| engine.rs `spawn` | inside the closure: `inbox = Rc::new(EngineInbox::new(erx, inner))` → `QuickJsSpecRuntime::new(inbox.clone(), &settings)`, on `Err` the same `engine: js host: {e:#}` → `Engine::new(..).run(&runtime, &inbox)`; the runtime is still built before the bundle install | 5.3d |
| `Engine::run`'s `js.next_msg()` / `js.try_next_msg()` | `inbox.next_msg(js)` / `inbox.try_next_msg(js)` | 5.3d |
| `Engine::handle`'s `ExecDone { .. } \| Timer { .. } => {}` | `EngineMsg::Runtime(_) => {}` (unreachable, as today) | 5.3d |
| engine.rs `js.start_sleep` (`update_slots`), `js.start_call` (`begin`, `exec_output`, `custom_result`), `js.start_exec` (`script_input`'s fetch closure), `js.forget` (`pump`) | text unchanged (`start_sleep` / `start_call` / `start_exec(ExecRequest::from(exec), ..)` / `forget`); `SpecRuntime` imported | 5.3a, 5.3f |
| `parser.rs:566-571` `Host::call(f, Vec<JsArg>) -> Value` | `call(SpecFunctionCall) -> Result<SpecFunctionResult, ParseError>` | 5.3g |
| `parser.rs:585-587` `JsHost::call` | `Ok(SpecRuntime::call(self.js, call, Some(&self.generator), TIMEOUT)?)` | 5.3g |
| `parser.rs:1166-1169` alias | `let SpecFunctionResult::Alias(Some(value)) = host.call(Alias { .. })? else { return Err(no("alias did not return a string")) };` the `trim_end` stays | 5.3g |
| `parser.rs:1193-1195` function loadSpec | `match host.call(LoadSpec { .. })? { SpecFunctionResult::LoadSpec(d) => d, _ => None }` | 5.3g |
| `parser.rs:1239-1245` cache key | `Err(Aborted)` → return; `Ok(GenerateSpecCacheKey(k))` → `k`; `Ok(_) \| Err(NoSuggestions(_))` → `None` | 5.3g |
| `parser.rs:1255-1257` generateSpec | `host.call(GenerateSpec { .. }).and_then(\|r\| match r { GenerateSpec(s) => s.map(\|spec\| *spec).map_err(no), _ => Err(no("spec is not an object")) })`; unboxing there keeps `let g = Rc::new(g);` (one new `Rc` per uncached call); the warn text and its Debug form do not change | 5.3g |
| `parser/tests.rs:77-125` Mock | `Callback` and `fns: HashMap<u32, _>` unchanged; `call` = `call_arguments` → callback → `function_result`; test bodies unchanged | 5.3g |

The `let-else` and the `_ => None` arms in the parser above read a variant that the adapter guarantees
(`kind → variant`, pinned by a test, P18). Where a match lists all ten variants, write them out (D8).

### P12 Runtime sharing
- Use `&self` methods with method-scoped interior borrows and share one `Rc<QuickJsSpecRuntime>`. Never put an
  outer `RefCell` around it.
- Drop the test bindings' `mut` that becomes unused (`unused_mut` would break the clippy budget of 3), and pass `&js`
  where an `&mut js` argument now meets a `&` parameter (clippy's default `unnecessary_mut_passed`). Those tests show
  as CHANGED, which is justified. A `&mut` parameter that reborrows into a `&` one is not flagged, so Phase 4's test
  helper `report_session_context` keeps its `&mut` and its five callers keep their bodies.
- There is no Rc cycle: the inbox does not hold the runtime.
- The repository → runtime → inbox chain never re-enters itself.

### P13 Repository: the judged design
```rust
// domain/value_objects/spec_lookup.rs   ← the loader half of ParseCtx (per find, so settings are read live)
pub(crate) struct SpecLookup<'a> { settings: &'a Settings, index: &'a SpecIndex, generator_context: &'a GeneratorContext }
impl<'a> SpecLookup<'a> {
    pub(crate) fn new(settings, index, generator_context) -> Self;   /* settings(), index(), generator_context() */
    /// autocomplete.disableForCommands lists the location's name (the base name for a Local): parser.rs:236-243.
    pub(crate) fn disables(&self, location: &SpecLocation) -> bool;
}
// domain/repositories/spec_repository.rs
pub(crate) trait SpecRepository {
    /// Ok(None): no spec file, a default export that is not usable, or a name in autocomplete.disableForCommands
    /// (checked on EVERY find, before the cache, never cached). The caller then tries the next location; any Err
    /// stops the parse. An unchanged spec keeps its Rc.
    fn find(&self, location: &SpecLocation, lookup: &SpecLookup<'_>) -> Result<Option<Rc<Subcommand>>, SpecError>;
    /// Settings saved: drop loaded specs only (loader.specs.clear()). Spec identities by JSON, CLI versions and
    /// running probes survive.
    fn forget_loaded_specs(&self);
}
#[cfg(test)] pub(crate) struct InMemorySpecRepository { specs: HashMap<String /* "name" | "dir|name", the Mock's keys */, Rc<Subcommand>>, finds: RefCell<Vec<SpecLocation>> }
// with(&[(&str, &Rc<Subcommand>)]), finds() -> Vec<SpecLocation>; records every find, then answers None when lookup.disables(location), else the same Rc; never caches; forget_loaded_specs is a no-op
// infrastructure/adapters/quickjs_spec_repository.rs
pub(crate) struct QuickJsSpecRepository { runtime: Rc<QuickJsSpecRuntime> /* concrete: needs module maps + raw default() */, bundle: EmbeddedSpecBundle /* injected: no find reads the env */, loader: RefCell<Loader> }
impl QuickJsSpecRepository { pub(crate) fn new(runtime: Rc<QuickJsSpecRuntime>, bundle: EmbeddedSpecBundle) -> Self }   // no I/O, no JS
impl SpecRepository for QuickJsSpecRepository { /* find = loader.borrow_mut().load(loc, lookup, &runtime, &bundle), whose first check is `if lookup.disables(loc) { return Ok(None) }`; forget_loaded_specs = loader.borrow_mut().specs.clear() */ }
#[derive(Default)] struct Loader { specs, versions, inflight: HashMap<String, TaskId>, by_json }   // parser.rs:216-323, unchanged
fn spec_file(loc: &SpecLocation, lookup: &SpecLookup, bundle: &EmbeddedSpecBundle) -> Option<PathBuf>;   // parser.rs:326-344; Global = bundle.resolve(..)
fn cli_version(exports: &Map<String, Value>, name: &str, runtime: &QuickJsSpecRuntime, generator: &GeneratorContext, inflight: &mut HashMap<String, TaskId>) -> Result<Option<String>, SpecError>;   // parser.rs:347-388
const TIMEOUT: Duration = Duration::from_secs(5);
// infrastructure/adapters/fig_version_diff.rs ← parser.rs:393-508 (apply_versions folding over versions_to_apply, merge_*, shallow_merge, make_array, names, truthy)
```
- `SpecLookup::disables` states the `disableForCommands` rule once, in the domain: it is business logic, and the
  `parser.rs:236-243` expression moves into it unchanged. The adapter's `find` and `InMemorySpecRepository::find` both
  call it first, so the double honours the trait contract. No existing test sets the key, so the Mock's migration in
  Phase 6.3 changes no test.
- The bundle is injected. Spawn builds it from the same `SharedDependencies::from_env()` as the engine's index build
  (P10; 5.4b moves that line into spawn), and `spec_file`'s Global arm calls `bundle.resolve(..)`. Only the env read
  moves, from every Global find to spawn: nothing in-process changes `HOME`/`TABSMITH_HOME` (P10), `DataDirectory`
  captures the env when it is built, and `resolve` keeps `specs_dir().ok()?`, so a missing home still finds nothing.
  Tests that build a repository pass `EmbeddedSpecBundle::new(data_directory_from_env())`: the env their finds read
  today.
- Holding one `loader.borrow_mut()` for the whole `find` is safe: nothing `find` calls re-enters the repository.
  JS host functions touch only the runtime's registry. Say so in a comment.
- The parser wiring (5.4b):
  - `Parser { repository: Rc<dyn SpecRepository>, runtime: Rc<dyn SpecRuntime>, caches }`, built with
    `Parser::new(repository, runtime)`;
  - `parse(cmd, ctx)`;
  - `clear_caches` = `forget_loaded_specs()` + the `ParseCaches` reset that keeps `first`;
  - `#[cfg(test)] load_spec(&self, loc, ctx)` (D7);
  - `JsHost { repository: &dyn SpecRepository, runtime: &dyn SpecRuntime, ctx, generator }`, where `load` =
    `find(loc, &SpecLookup::new(ctx.settings, ctx.index, &self.generator)).map_err(ParseError::from)`;
  - `Engine::with(.., parser: Parser)`;
  - spawn builds, in order, inside the thread closure, which still returns `()`:
    1. the inbox;
    2. `let runtime = match QuickJsSpecRuntime::new(inbox.clone(), &settings) { Ok(r) => Rc::new(r), Err(e) => {
       logger::error(&format!("engine: js host: {e:#}")); return; } };`, so a failed build is logged with the same
       text and level before anything else is built, and the bundle is never installed (today's order);
    3. `let dependencies = SharedDependencies::from_env();` (moved from `Engine::new`, which now takes it);
    4. `QuickJsSpecRepository::new(runtime.clone(), EmbeddedSpecBundle::new(dependencies.data_directory.clone()))`;
    5. `Parser::new(repository, runtime.clone())`;
    6. `Engine::new(tx, settings, parser, &dependencies)`: the install, the index and the history module, in today's
       order;
    7. `run(&runtime, &inbox)`.

### P14 Timeouts
There are two 5 s `TIMEOUT` consts: one in the repository adapter and one in the parser (later
`CommandLineParser`). Neither is merged with `LOAD_TIMEOUT` or `SYNC_CALL`.

### P15 `EngineMsg::Timer` is deleted in 5.3d
It is never constructed (grep proof in the commit body), and plan section 4 already says it is deleted. The plan's
Phase 11 entry for it becomes void.

### P16 Test homes the plan does not name
- `wait_aborts_and_defers_on_complete` moves to `engine.rs` tests, over the real `EngineInbox` and the real
  `EngineMsg`. It moves again to `bootstrap/engine_server.rs` in Phase 6.6.
- `cli_version_detection_survives_an_aborted_parse` switches to `ChannelRuntimeEventSource` in 5.3d:
  `send_newer_request` replaces `tx.send(Complete(req))`, and `take_newer_request()` replaces
  `matches!(js.next_msg(), Some(Complete(_)))`. Then it moves with `cli_version` into the repository's tests in
  5.4b. It is the same scenario: a newer request is queued before the probe, with the same five assertions.
- `spec_identity_survives_dev_reload_and_cache_clear` stays in `parser/tests.rs`, with only its construction
  changed. Phase 6.3 splits it (D6): the repository half keeps the name, and the `first` half becomes
  `…_first_token`.

### P17 Fitness and the last step
- 5.5 registers `specs`. `spec` and `js` stay in `LEGACY_MODULES`: Phase 3's `ponytail:` comment says a name stays
  listed after its file is deleted (harmless). 5.5 only rewrites that comment's sentence about the legacy `specs.rs`
  (the plan's Phase 3 note asks for it), because `specs` is now the registered module.
- It greps that no production code under `src/specs` names a legacy module:
  `grep -rn 'crate::\(types\|engine\|parser\|suggest\|pty\|inline\|menu\|render\|icons\|doctor\|cli\|tracker\|keys\|shell_parser\|js\|spec\)\b' src/specs`
  must be empty outside `#[cfg(test)]` code. That holds in infrastructure too: `RuntimeEvent` replaces the
  `EngineMsg` sender.
- That grep misses grouped imports (`use crate::{engine::X, types};`), so 5.5 also reviews every hit of
  `grep -rn 'use crate::{' src/specs` by hand.
- Rule 3 only blocks other modules' use cases and infrastructure, so plan section 2's edge set (specs → shared,
  settings) is checked by hand too:
  `grep -rnE 'crate::(history|shell_integration|completion|ghost_text|dropdown|terminal_session|diagnostics|bootstrap)\b' src/specs`
  must be empty outside `#[cfg(test)]` code.
- There is no `anyhow` in `src/specs` production code (plan section 1).

### P18 Kind → variant is pinned by a test
`function_result` must return the variant of the given kind for every input. One test covers all ten kinds. That
is what makes the `let-else` reads in the parser safe.

### Deviations from the plan's letter (record them in plan section 9 at 5.5)
1. `find(&SpecLocation, &SpecLookup) -> Result<Option<Rc<Subcommand>>, SpecError>` + `forget_loaded_specs()`, with
   no `NotFound`. `Ok(None)` means "try the next location", and an `Err` stops the parse at once
   (`parser.rs:1046`).
2. The `SpecError` variants are `Aborted, Timeout, Invalid(text), BundleUnavailable(text)`.
3. `disableForCommands` stays inside `find` (D1), stated once in the domain as `SpecLookup::disables`, which the
   adapter and the InMemory double both call first. Phase 6.3 must not move it into `CompletionRequestPolicy`.
4. Both generatorContext builders stay (D1). Both `script_timeout`s stay (D1).
5. `GeneratorScript::Object(SpecValue)`, not a typed `ExecRequest` (D1). `SpecValue` is an exact tree.
6. `EngineMsg::Timer` is deleted in 5.3, not Phase 11.
7. The types that completion's domain must name (`TaskId`, `TaskOutcome`, `SpecValue`, `SpecFunctionCall`/`Kind`/
   `Result`, `ScriptResolution`, `CustomGeneratorResult`, `TemplateSuggestions`) live in `specs/domain`, because
   fitness rule 1 forbids a domain file from naming an `application` path. The glossary rows move from "Port types"
   to VO.
8. The legacy engine's generator state machine stays on the JSON API (`call_json`, `wait_json`) in Phase 5. The
   typed port's task half has no production caller until 6.4.
9. Phase 6.4 adds `read`, `read_exec_stdout` and `resolve_script` to `SpecRuntime`.
10. The serde record keeps the identifier `ExecInput`.
11. `Parser` takes `Rc<dyn SpecRepository>` + `Rc<dyn SpecRuntime>` in its constructor. `Parser::load_spec` becomes
    `#[cfg(test)]` instead of being deleted in Phase 11.
12. Test homes per P16.
13. Two 5 s `TIMEOUT` consts.
14. `kill_live_children` stays a free function: the plan's `ProcessExecutor::kill_live_children` is read as a module
    path.
15. `let-else` on a decoded variant (P11, P18).
16. The InMemory doubles have minimal surface and no Phase 5 consumer.
17. `spec.rs` is deleted in 5.1b, `js.rs` in 5.3c, and `specs.rs` becomes `specs/mod.rs` in 5.2b.
18. `SpecFunctionRef` is a type alias.
19. `SpecVersionSelector` is cumulative. Fix section 4's "newest ≤ target".
20. `spec_location_table` and `resolve_order` are not split (P9).
21. There is no `LoadSpecIndexUseCase`, and `SpecBundle` has only `install` (P10).
22. The engine's JSON halves and `template_json` move in 5.1, touching Phase 6 files early.
23. `SuggestionKind` drops its serde derives, and every user switches in 5.1 (no `types.rs` alias).
24. The spec model's fields are private behind accessors and plain `with_*` builders. `Arg::default_value()`.
25. The production generatorContext JSON builder, `generator_context_json`, lives in `fig_spec_mapper` with the other
    JSON readers and writers, not in `quickjs_spec_runtime`; the test-only one stays in the runtime tests (deviation 4).
26. `InstallSpecBundleUseCase` only delegates to `SpecBundle::install`: the `ensure_extracted` branching (fast path,
    extract, touch + cleanup, the `cfg(feature)` split) stays in `EmbeddedSpecBundle` (P10).

### Hand-off notes for later phases
- **Phase 6, completion.**
  - `CommandLineParser` is the Phase 5 `Parser` renamed, and the `Host` trait is deleted. `host.load` becomes
    `specs.find(loc, &SpecLookup)`, where `None` means "try the next location". `host.call` becomes
    `runtime.call(typed, Some(&generator), TIMEOUT)`.
  - The Mock becomes `InMemorySpecRepository` + `InMemorySpecRuntime`. A test helper may wrap the JSON callbacks
    through `call_arguments`/`function_result` (D2), so the test bodies stay the same.
  - `disableForCommands` stays in the repository (deviation 3): `SpecLookup::disables`, called first by every `find`.
  - Completion may not import `fig_spec_mapper` (rule 3). It reads results through the typed port only.
  - `GeneratorRun` returns `StartCall(SpecFunctionCall, GeneratorContext)`, `StartExec(ExecRequest,
    GeneratorContext)`, `StartSleep(u64)` or `Settle`.
    - The use case calls `read`, `read_exec_stdout` and `resolve_script`.
    - A script's timeout is the `let timeout = …` in `Engine::script_input` exactly (`engine.rs:872-874` at
      `0a0a74e`): `script_timeout(settings)` (the f64 floor)
      `.max(generator.script_timeout().unwrap_or(0)).max(request.timeout().unwrap_or(0))`, applied with
      `ExecRequest::with_timeout` after `ScriptResolution::Ready` and before `start_exec`. `cache_key_source` is
      still the text taken before any timeout is set. `with_timeout` arrives in 6.4.
    - `GenCache` becomes `GeneratorCache<SpecValue>` and still stores raw pre-step values. A step of another kind
      may read them (`Engine::fetch`, and the `self.cache.store` in `Engine::advance`; `engine.rs:832-863, 946-948` at
      `0a0a74e`). A malformed result must still map to `[]`, never to a task
      error.
  - `ApplyCompletionSettingsUseCase` runs today's sequence (the `EngineMsg::Settings` arm of `Engine::handle`), in
    today's order, on **every** Settings message: `apply_settings(&s)` (the runtime's i64 floor);
    replace the session's settings; `forget_loaded_specs()`; reset the `ParsePrefixCache`, keeping `first`; reset the
    generator slots (slots, slot arg, previous search term); clear the first-token cache.
  - The f64 `script_timeout` moves to `GeneratorTriggerPolicy`. The single-token dotslash/`bin/console` rule
    (`parser.rs:983-998`) moves with `ParseCaches::parse` and never reuses `spec_location`.
  - `EngineInbox` and its two tests move unchanged to `bootstrap/engine_server.rs`
    (`EngineMessage::Runtime(RuntimeEvent)`). Everything is still built inside the engine thread closure.
  - The index build (`build_specs_module(&dependencies).install_spec_bundle.execute()`, then the adapter's
    `load_index`, double ensure) moves to `engine_server` with the same log line and fallback, and the same
    `SharedDependencies` builds the repository's `EmbeddedSpecBundle`.
  - Split `spec_identity_survives_dev_reload_and_cache_clear` (D6).
  - `SpecIndex` exposes its completions in order, and `SpecLocation::cache_key()` builds the prefix-cache key.
- **Phases 8-9.** `render::describe` goes to a dropdown domain service (P3). `GeneratorProcessReaper` is bridged to
  `process_executor::kill_live_children`.
- **Phase 10.** Diagnostics gets the bundle version and the "installed" fact via `SystemFacts` or a port, and its
  `InstallSpecBundle` fix goes through the `CheckFixer` bridge to `InstallSpecBundleUseCase`. Doctor's check path
  (`ctx.data_root()/specs/<version>/index.json` in `doctor::setup_checks`, where `ctx` is Phase 4's
  `IntegrationPaths`) and the fix's env-derived root stay as they are.
- **Phase 11.** `EngineMsg::Timer` is already gone. `Parser::load_spec` is `#[cfg(test)]` and dies with the parser
  in Phase 6.

## Glossary additions (proposed for `docs/ubiquitous-language.md`, specs section; 5.5 writes them)

| Term | Kind | Meaning | Replaces |
|---|---|---|---|
| `SpecValue` | VO | A JSON value written by spec code (a script object, a call argument, a task result), held exactly as read. Unsigned, negative and float numbers are kept apart; object keys are sorted. | `serde_json::Value` in the spec model and runtime |
| `GeneratorScript` (edit) | VO | Argv, legacy shell string, function, or an `executeCommand` object kept as a `SpecValue`. It becomes an `ExecRequest` only when the generator runs, so a malformed object still fails then, with `bad script input: …`. | `Script` (`Object(Value)` → `Object(SpecValue)`) |
| `SpecFunctionRef` (edit) | VO (alias of u32) | Index of a function in the runtime's function table, written `{"$fn": n}`. | `FnId` |
| `SpecLocation` (edit) | VO | Global `{name}` or Local `{dir, name}`. `cache_key()` (`global,,{name}` / `local,{dir},{name}`, trailing slash kept) names it in every cache; PathBuf equality never does. | `SpecLocation`, `loc_key` |
| `SuggestionKind` (note) | VO | Fig `type` strings map to it in the Fig spec mapper: `mixin` → Special, unknown → Arg. | `types::ItemType` (+ `from_fig`, `fig_type`) |
| `SpecIndex` (edit) | VO | Root spec names in `index.json` order, then the user's own spec names, plus the diff-versioned set. | `SpecIndex` |
| `SemanticVersion` (edit) | VO | Parsed CLI version, with node-semver precedence; an unparsable version compares as 0.0.0. | `SemVer` + `semver*` |
| `SpecLocator` | Domain service | Where a command's spec lives (Global or Local, from the first token, isCommand or isScript; `bin/console` is `php/bin-console`). Which file a Global name loads from, over the roots the caller gives in order: root by root, `<name>.js`, else `<name>/index.js`; diff-versioned names only `<name>/index.js`; plain relative names only. | `parser::spec_location`, `specs::resolve_in` |
| `SpecVersionSelector` | Domain service | Which diffs of a diff-versioned spec apply for a detected CLI version: every diff whose version is ≤ it, oldest first, or all of them when the version is unknown. | key selection of `apply_versions` |
| `BundleRetentionPolicy` | Domain service | Which dirs under `~/.tabsmith/specs` are stale bundles to remove: tabsmith's own all-numeric version dirs other than the current one, unused for over 7 days, and `<version>.tmp-<pid>` dirs older than 10 minutes. | `specs::cleanup` rules |
| `SpecBundle` | Port | The embedded Fig spec bundle under the data directory: install it (extract if missing, mark in use, remove stale versions). | `specs::ensure_extracted` |
| `SpecFunctionCall` / `SpecFunctionResult` (edit) | VO | A typed call into a spec function, and its typed result, for ten kinds: query term, trigger, alias, load spec, generate-spec cache key, generate spec, script, custom, post-process, filter template suggestions. | `JsArg` + `serde_json::Value` |
| `SpecFunctionKind` | VO | Which of the ten kinds a call is; it decides how a result is read. | |
| `TaskId` / `TaskOutcome` | VO | Handle of a started spec function, process or sleep (one id space), and its raw result: a `SpecValue` or the error text. | `js::{TaskId, TaskResult}` |
| `ScriptResolution` | VO | What a generator's script turned into: skip, invalid (serde text), or an `ExecRequest` + the cache-key source text. | inline in `engine::script_input` |
| `CustomGeneratorResult` | VO | A custom generator's suggestions + whether they are templated (first item has a `context` key). | inline in `engine::custom_result` |
| `TemplateSuggestions` | VO | What `filterTemplateSuggestions` receives: listed suggestions, or a templated custom result passed back unchanged. | `template_json` / raw custom value |
| `SpecLookup` | VO | What finding a spec needs from the current request: the settings, the `SpecIndex` and the `GeneratorContext` the CLI version probe and a versioned spec's `default(version)` run with. It also says whether `autocomplete.disableForCommands` disables a location's name. | `ParseCtx` + `gen_ctx` handed to `Loader::load` |
| `SpecError` | Domain error | Aborted (`aborted by a newer request`), Timeout (`timed out`), Invalid (text as is), BundleUnavailable (text as is). | `js::JsError` + the anyhow errors of `spec`/`specs` |
| `SpecRepository` | Repository | Finds the Spec at a `SpecLocation`. It answers none when the spec is missing, disabled by `autocomplete.disableForCommands`, or not a spec. `forget_loaded_specs` makes later finds look again; an unchanged spec keeps its `Rc`. | `parser::Loader`, `Host::load` |
| `SpecRuntime` | Port | Starts spec functions, processes and sleeps, waits for them (interrupted by a newer request), forgets them, and applies the script-timeout setting. | `js::Js` API, `parser::Host::call` |
| `RuntimeEventSource` | Port | The engine inbox as the runtime sees it: runtime events in arrival order, other messages kept for the engine, and whether a newer request is waiting. | `Js.rx` + `Js.deferred` |
| `RuntimeEvent` | Port type | A generator process finished: id + `ProcessOutput` or error text. | `EngineMsg::{ExecDone, Timer}` |
| `Interrupted` | Port type | A wait stopped because a newer request is waiting; no task was dropped. | `JsError::Aborted` from `wait` |

Rows to edit:
- `host / pump`: `QuickJsSpecRuntime` replaces the js Host, which becomes its task registry. `EngineInbox`
  (engine.rs, then bootstrap) implements `RuntimeEventSource`.
- `cache`: add "detected CLI version memo and in-flight version probes (inside `QuickJsSpecRepository`)".
- New word row `version`: the bundle version (`assets/SPECS_VERSION`, `specs/<version>`), the CLI version
  (`SemanticVersion`) and the diff version (a key of `versions`).

These names are infrastructure or plumbing and get no row: `RuntimeEventSink`, `RuntimeEventPoll`, `EngineInbox`,
`ExecInput` (record), `JsArg`, `TaskResult`, `TaskRegistry`, `ChannelRuntimeEventSource`, `RecordingSpecBundle`,
`EmbeddedSpecBundle`, `InstallSpecBundleUseCase`, `SpecsModule`, `ScriptValue` (6.4).

## Test moves

Below, "path-only" means testlock's loose form shows no change. "CHANGED" means a justified restructuring: the
same scenario and the same assertions.

| Test (at `a3953cf`) | Destination | Step | Restructuring |
|---|---|---|---|
| `normalise_sudo`, `normalise_cd_function_generator`, `normalise_templates_and_persistent`, `normalise_requires_separator`, `normalise_git_like`, `normalise_mock_spec`, `normalise_shapes_and_collisions`, `suggestions_and_load_spec_results` (`spec.rs:544-790`) | `fig_spec_mapper.rs` tests | 5.1b, 5.1d-e | glossary renames (path-only); accessors (CHANGED); `CacheCfg` literal → `GeneratorCachePolicy::default().with_ttl_ms(..)…` (CHANGED); `help.default` → `default_value()`; literal ids unchanged; `is_err()` unchanged |
| `legacy_string_script_runs_through_sh_c`, `js_truthiness`, `post_process_filtering` (`engine.rs:1297-1341` at `0a0a74e`) | `fig_spec_mapper.rs` tests | 5.1c (accessors in 5.1d) | none; `post_process_filtering` CHANGED for accessors |
| new `should_keep_a_script_object_exactly` | `fig_spec_mapper.rs` | 5.1c (RED first) | `u64::MAX`, -5, `1.0715660391465826e-75`, nested env with `null`, unknown key: `Value` and `to_string` equal after the round trip; an object script → `Object`, `{"$fn": n}` → `Fn` |
| `suggest.rs` tests | stay (Phase 6) | 5.1a (kind), 5.1b (paths), 5.1d-e (builders) | `ItemType::X` → `SuggestionKind::X` (path-only); the bare type in `first_token_sources` (2521 at `0a0a74e`, CHANGED); helpers `sub`/`opt`/`arg` (1495-1514) → builders; CHANGED: `cache_ttl_swr_max_age`, `assembly_options_rules`, `option_chain_rewriting`, `item_highlight_and_hint`, `help_template`, `list_dir_sorted`, `list_dir_skips_control_char_names`; input string at 2331 untouched |
| `menu.rs`, `icons.rs`, `inline.rs`, `render.rs`, `pty.rs` tests | stay | 5.1a | `ItemType::X` → `SuggestionKind::X` (path-only) |
| `parser/tests.rs` Mock-based tests | stay (Phase 6.3) | 5.1b, 5.1d-e, 5.3g | Mock `FnId` → `SpecFunctionRef`; helpers `sub`/`pos`/`oarg` → accessors (`oarg` keeps ignoring the bool); accessor CHANGED: `parse_arguments_table`, `result_fields`, `sudo_git_command_index`, `generate_spec_merge`, `persistent_options_inherit_without_mutation`, `inline_and_function_load_specs`, `git_alias_directive`; 5.3g changes only the Mock (`call` = encode → callback → decode), no test body |
| `generator_keys_differ_per_index` (`engine.rs:1469` at `0a0a74e`) | stays | 5.1b | path-only (`Arg::default()` still works: accessor is `default_value`) |
| new `should_key_…` (2) for `cache_key` | `spec_location.rs` | 5.2a | test-first |
| `spec_location_table` (`parser/tests.rs:655`) | `spec_locator.rs` tests | 5.2a | path-only |
| new `should_…` for `find_global_spec` (set predicate: validation, diff skip in every root, root-major, first wins) | `spec_locator.rs` | 5.2a | test-first |
| `semver_helpers` (`parser/tests.rs:759`) | `semantic_version.rs` tests | 5.2a | path-only (free fns keep names) |
| new `should_…` for `versions_to_apply` (cumulative, `None`/garbage → all, unparsable as 0.0.0, equal keys keep input order, prerelease target excludes its release) | `spec_version_selector.rs` | 5.2a | test-first |
| new `should_…` for `is_stale` (exactly 7 days kept, stale `<current>.tmp-N` removed, `user`/`dev`/pre-release kept, `None` age kept) | `bundle_retention_policy.rs` | 5.2a | test-first |
| `extract_renames_and_removes_other_versions`, `extract_keeps_versions_in_use`, `extract_removes_abandoned_temp_dirs`, `extract_error_cleans_temp`, `dev_folder_expands_tilde` | `embedded_spec_bundle.rs` tests | 5.2b | path-only |
| `index_merges_user_stems`, `embedded_index_shape` | `embedded_spec_bundle.rs` tests | 5.2a (accessors), 5.2b (move) | CHANGED: accessors; `SpecIndex::load_from(` → `load_from(`; `testutil::` → `testing::` |
| `resolve_order` | `embedded_spec_bundle.rs` tests (fs guard, whole) | 5.2a, 5.2b | CHANGED: the struct literal → `SpecIndex::new(vec![], ["az".to_string()].into())` |
| `resolve_rejects_drive_relative_names` (`cfg(windows)`) | `embedded_spec_bundle.rs` tests | 5.2b | path-only; hand-check its imports (never compiled locally) |
| new `should_render_…_like_anyhow` | `embedded_spec_bundle.rs` | 5.2b | test-first; the renderer is `format!("{context}: {}", display_chain(cause))` over Phase 4's shared `display_chain`, compared with anyhow `{:#}` |
| new `InstallSpecBundleUseCase` tests over `RecordingSpecBundle` | `install_spec_bundle_use_case.rs` | 5.2b | test-first |
| `should_keep_the_whole_chain_of_a_failed_extraction` (Phase 4, `bootstrap/bridges.rs`) | stays | 5.2b | none; a guard: `spec_bundle_failure(error: anyhow::Error)` keeps its signature and body, and the bridge feeds it `anyhow::Error::from(e)` |
| `wrap_module_rewrites_export_clause` | `quickjs_module_wrapper.rs` | 5.3b | none |
| `native_ls_and_short_circuits` (whole, no split: one module), `exec_reports_an_unknown_command_instead_of_stubbing_it`, `exec_status_output_and_timeout`, `cmd_gets_no_cwd_search_env`, `timeout_kills_grandchildren` (windows) | `process_executor.rs` tests | 5.3a (constructors), 5.3b (move) | CHANGED: `ExecInput`/`ExecOutput`/`GenCtx` literals → `ExecRequest::new`/`ProcessOutput::new`/`GeneratorContext::new` with the same values; field reads → accessors; hand-check the windows test |
| `exec_children_die_with_the_wrapper` | `process_executor.rs` tests (beside the private `live()`) | 5.3b | CHANGED: constructors + D9 NAME |
| `loads_hand_written_es_modules`, `serializes_functions_and_values`, `loads_bundled_specs`, `set_timeout_ordering_and_intl`, `interrupt_stops_infinite_loop`, `long_callback_survives_short_pump_slices`, `load_module_reloads_edited_file`, `all_bundle_modules` (name kept, `#[ignore]` kept) | `quickjs_spec_runtime.rs` tests | 5.3c | CHANGED where `mut` is dropped or `call`/`wait` → `call_json`/`wait_json`, `JsError::Other` → `SpecError::Invalid`; `testutil` → `testing` paths (5.2b) |
| `git_alias_directive_with_fake_git`, `custom_generator_executes_commands` | `quickjs_spec_runtime.rs` tests | 5.3a, 5.3c | CHANGED: `GenCtx` literal → constructor; `ctx.context_json()` → `context_json(&ctx)`; `mut` dropped; keep `contains("Failed parsing alias")` |
| `wait_aborts_and_defers_on_complete` | `engine.rs` tests | 5.3d | CHANGED: built over the real `EngineInbox` + `EngineMsg`; `js.next_msg()` → `inbox.next_msg(&js)`; same messages, order, deadlines, assertions (`{stdout:"",stderr:"",status:1}`) |
| new `should_report_a_newer_request_only_once_received`, `should_keep_reporting_it_until_taken` | `runtime_event_source.rs` | 5.3d | test-first |
| new `should_deliver_a_runtime_event_read_by_the_engine_loop_to_the_runtime` | `engine.rs` tests | 5.3d | guards against a Runtime message falling through to the engine's no-op arm |
| `cli_version_detection_survives_an_aborted_parse` | `parser/tests.rs` (5.3d), then `quickjs_spec_repository.rs` tests (5.4b) | 5.3c-d, 5.4b | CHANGED: over `ChannelRuntimeEventSource`, as in P16; `GenCtx::default()` → `GeneratorContext::default()`; 5.4b: `Err(SpecError::Aborted)` |
| new SpecRuntime/InMemory tests (answer, record, unknown fn, a forgotten task is unknown, a running task gives None); one encode test and one decode test per kind; kind → variant for all ten | `spec_runtime.rs`, `quickjs_spec_runtime` tests, `fig_spec_mapper.rs` | 5.3e (port, InMemory), 5.3f (encode, decode, kind → variant) | test-first; decode cases: alias 5 → `Alias(None)`; query term `[]` → `None`; trigger 0 or `""` → `false`; generate spec 5 → `Err("spec is not an object")`; script `[1]` → `Invalid("invalid type: integer `1`, expected a string")`; script `{"cwd":null,..}` keeps cwd `None`; `{"timeout":1.5}` → timeout `Some(0)`; custom `[{"context":null}]` is templated; an exec JSON read as Custom → `[]`, not templated |
| `spec_identity_survives_dev_reload_and_cache_clear` | stays in `parser/tests.rs` | 5.1e, 5.3c-d, 5.4b | CHANGED: `d.args[0].name` → accessors; runtime constructor; `Parser::new(Rc::new(QuickJsSpecRepository::new(runtime.clone(), EmbeddedSpecBundle::new(data_directory_from_env()))), runtime)`; `load_spec(&loc, &ctx)`; assertions unchanged |
| `session_update_refreshes_first_token_items` (`engine.rs:1344` at `0a0a74e`) | stays (Phase 6) | 5.3c-d, 5.4b | CHANGED: runtime constructor over `ChannelRuntimeEventSource`; `let mut js` → `let js` and `&mut js` → `&js` (clippy `unnecessary_mut_passed` once `handle` takes `&`); 5.4b builds the runtime before `Engine::with` and passes it a `Parser` over `runtime.clone()`; Phase 4's `EngineMsg::Session { env: HashMap::new(), alias: None }` stays as it is |
| Phase 4's five engine session tests `should_use_the_env_of_a_session_context_in_place_of_the_wrapper_env`, `should_fall_back_to_the_wrapper_env_after_a_session_context_with_an_empty_env`, `should_parse_the_aliases_of_a_session_context_for_the_shell`, `should_find_no_aliases_after_a_session_context_with_an_empty_alias_text`, `should_find_no_aliases_after_a_session_context_without_alias_text` (added by Phase 4, not at `a3953cf`) | stay (Phase 6 hands them to `ApplySessionContextUseCase`) | 5.3c-d, 5.4b | none, only their helpers change: `engine_with_wrapper_env` returns `(Engine, QuickJsSpecRuntime)` (5.3c), builds it over `ChannelRuntimeEventSource` (5.3d), and in 5.4b builds the runtime before its `Engine::with`, gives that a `Parser` and returns the shared runtime. `report_session_context` keeps a `js: &mut …` parameter (`&mut QuickJsSpecRuntime`, then the type the helper returns), so the bodies' `let (mut e, mut js)` and `&mut js` stay: it reborrows `js` into `handle`'s `&`, which clippy does not flag, while `&mut js` passed straight to a `&` parameter would trip `unnecessary_mut_passed`. The test-lock lens diffs both helpers |
| `first_token_spec_identity_is_stable` | stays | all | guard; unchanged |
| new `should_return_the_stored_spec_unchanged` (`ptr_eq`), `should_find_a_local_spec_by_dir_and_name`, `should_answer_none_for_an_unknown_location`, `should_record_every_find_in_order` | `spec_repository.rs` | 5.4a | test-first |
| new `should_disable_only_a_listed_command_name` (a listed Global name, a listed Local base name, an unlisted name); `should_answer_none_for_a_disabled_command` (InMemory, the find still recorded) | `spec_lookup.rs`; `spec_repository.rs` | 5.4a | test-first |
| `diff_merge`, `version_pick` | `fig_version_diff.rs` tests | 5.4a | none |
| new characterization tests in `quickjs_spec_repository.rs`: `should_find_nothing_for_a_disabled_command_and_cache_nothing`, `should_keep_a_missing_spec_missing_until_loaded_specs_are_forgotten`, `should_not_cache_a_spec_that_failed_to_load` | | 5.4b | must pass with no production change; they pin the traps deviation 3 relies on |

Expected totals: every moved test keeps its name, and MISSING stays 0. The pass count only grows.

## Step briefs

The full prompts are in `phase-5-specs.args.json` (ids below). Each implementer reads `agent-brief.md`, then this
file and the source maps named in its prompt. Every step ends with:
- the full gate green and everything committed (`refactor(specs): …`, one commit per sub-step or TDD phase);
- `python3 scripts/hexagonal-refactor/testlock.py . phase5-start --diff`: MISSING 0, DUPLICATE 0, every CHANGED test
  justified;
- for 5.2b, every 5.3 and 5.4 step and 5.5: `cargo test --locked --release -- --ignored all_bundle_modules`;
- for 5.1a-5.4b: the local fitness run ("Cross-cutting"), reverted before each commit;
- no rewritten history, subjects of at most 72 characters, and a lone `integrations_zsh_powerlevel10k_prompt_wrapped`
  failure rerun alone before the gate is run again ("Cross-cutting").

Each step compiles on its own: shims inside the phase are allowed (none is planned), and every legacy importer of
an item switches in the step that moves it. Steps 5.3a-5.3g are the plan's runtime step, split; nothing from another
plan step runs alongside them.

| Id | Title | Plan step | What it does |
|---|---|---|---|
| 5.1a | `SuggestionKind` and the specs module root | 5.1 | tags `phase5-start`; `src/specs.rs` declares `domain`; `types::ItemType` → `SuggestionKind` (serde dropped) and every user switches (~250 references); `from_fig` parked in `spec.rs` (P1, P3) |
| 5.1b | Spec model and the Fig spec mapper | 5.1 | `spec.rs` sliced into the domain files under glossary names (P2), `SpecError::Invalid`, `fig_spec_mapper.rs` (with `from_fig`, `fig_type`, `template_json`, the 8 tests); every importer switches; `spec.rs` deleted (P4, P6, P8) |
| 5.1c | `SpecValue` and the engine's JSON halves | 5.1 | RED/GREEN `should_keep_a_script_object_exactly`, `SpecValue` tree, `GeneratorScript::Object(SpecValue)`; `truthy`, `script_exec_input`, `json_items`, `post_process_items` + 3 tests → mapper; `SpecSuggestion::from_script_output` (P5, P8) |
| 5.1d | Private fields of the leaf spec VOs | 5.1 | `SpecSuggestion`, `Generator`, `GeneratorCachePolicy`, `ParserDirectives`: accessors, plain `with_*` builders, dead-code sweep (P7) |
| 5.1e | Private fields of `Arg`, `SpecOption`, `Subcommand` | 5.1 | same, `Arg::default_value()`, `with_directives`, `Subcommand::merged_with_generated` (P7) |
| 5.2a | Spec location and versions in the domain | 5.2 | `SpecLocation::cache_key` (TDD), `SpecIndex` VO, `spec_locator` (move + `find_global_spec` TDD), `SemanticVersion` (move), `SpecVersionSelector` (TDD), `BundleRetentionPolicy` (TDD); each switched into its legacy caller (P9) |
| 5.2b | Embedded spec bundle, `SpecBundle` and `InstallSpecBundleUseCase` | 5.2 | `embedded_spec_bundle.rs` (with the chain renderer TDD and the `testing` helpers), `SpecError::BundleUnavailable`, port + use case (TDD), `specs_module`; engine, parser, doctor, js tests and the Phase 4 bridge switch; `specs.rs` → `specs/mod.rs` (mod lines only); `--no-default-features` check (P1, P10) |
| 5.3a | Runtime value objects and the runtime error | 5.3 | `GeneratorContext`, `ExecRequest`, `ProcessOutput`, `TaskId`, `SpecError::{Aborted, Timeout}` (`JsError` gone), `exec_input.rs`, `generator_context_json`, D8 arms; `types::ExecOutput` deleted (P6, P11) |
| 5.3b | Process executor, native ls and module wrapper | 5.3 | `process_executor.rs`, `native_ls.rs`, `quickjs_module_wrapper.rs` + 7 tests; D9 name; pty switch; Windows hand-check (P11) |
| 5.3c | `QuickJsSpecRuntime` | 5.3 | `git mv src/js.rs` → `quickjs_spec_runtime.rs`, `TaskRegistry`, `SpecError` constructor errors; then `&self`, `wait_json`/`call_json`/`apply_settings`; `mod js` gone (P11, P12) |
| 5.3d | `RuntimeEventSource` and the engine inbox | 5.3 | the port + `ChannelRuntimeEventSource` (TDD), then alone: `EngineInbox` in engine.rs, `EngineMsg::Runtime`, `Timer` deleted, test moves, the delivery test, 50× guard runs by full test path (P11, P15, P16) |
| 5.3e | Typed spec function calls and the `SpecRuntime` port | 5.3 | the call/kind/result VOs, `TaskOutcome`, `Interrupted`, `SpecRuntime`, `InMemorySpecRuntime`, all TDD; no production caller yet (P11) |
| 5.3f | `QuickJsSpecRuntime` implements `SpecRuntime` | 5.3 | `call_arguments`, `function_result`, the trait impl; per-kind encode/decode tests and kind → variant (P11, P18) |
| 5.3g | The parser calls spec functions through `SpecRuntime` | 5.3 | typed `Host::call`, the four parser sites, the Mock over encode/decode; every parser test is a guard (P11) |
| 5.4a | `SpecRepository`, `SpecLookup` and the version-diff merge | 5.4 | trait + `InMemorySpecRepository` + `SpecLookup` with `disables` (TDD); `fig_version_diff.rs` with `diff_merge`, `version_pick` (P13) |
| 5.4b | `QuickJsSpecRepository` and the parser/engine wiring | 5.4 | `Loader`/`versioned`/`spec_file`/`cli_version` → the repository; `Parser::new(repository, runtime)`; spawn order; the test moves and three characterization tests (P13, P14, P16) |
| 5.5 | Register specs and record the phase | 5.5 | register `specs`, the greps, glossary rows, plan section 4 and 9 updates, status lines (P17) |

## Open questions (for the plan owner; the default applies if nobody answers)

1. **Private spec-model fields now.** The brief requires private domain fields. The cost is about 80 accessors,
   about 75 builders, about 180 mechanical read edits in Phase 6 files (engine, parser, suggest) and about 20
   CHANGED tests. The alternative is to keep them `pub(crate)` until Phase 6 moves the readers, as a recorded
   deviation. *Default: private now (5.1d-5.1e).*
2. **The typed `SpecRuntime` port and the typed parser `Host::call` in Phase 5.** The judge chose this, following
   the plan's letter, over deferring the port to Phase 6 (the runtime researcher's choice). The engine stays on the
   JSON API in both options. *Default: the judge's design (5.3e-5.3g).*
3. **The name of `SpecError`'s free-text variant.** `Invalid` follows the plan and the judge. `Other` would keep
   about 3 js test edits path-only and reads better for JS failures. *Default: `Invalid`.*
4. **Should `SpecValue` reuse settings' `SettingNumber`?** The specs → settings edge is allowed, but a script object
   is not a setting. *Default: its own `Unsigned`/`Negative`/`Float` variants.*
5. **No `LoadSpecIndexUseCase`.** The legacy engine, and later bootstrap's engine server, run
   `InstallSpecBundleUseCase` from `build_specs_module`, then the adapter's `load_index`. *Default: as stated.
   Phase 6 may add the use case if the engine-server dispatch wants one.*
6. **Where `render::describe` goes in Phases 8-9.** It is the only production `SuggestionKind` use that heads for
   terminal_session. *Default: a dropdown domain service, with no new `terminal_session → specs` edge.*
7. **Restructuring of `cli_version_detection_survives_an_aborted_parse`.** Building it over the event-source double
   instead of `EngineMsg::Complete` + `next_msg` keeps the scenario and the five assertions, but at the runtime's
   level of abstraction. *Default: accepted as restructuring. `wait_aborts_and_defers_on_complete` keeps the real
   inbox contract.*
8. **`SpecValue` as an exact tree instead of the judge's opaque canonical text** (P5). The judge's text form
   re-parses some floats differently (measured), which would change the cache-key text and the `bad script input`
   message of a script object holding such a float. *Default: the tree.*
9. **Seventeen steps.** The plan's five steps are split so that no agent has to hold `js.rs` (1835 lines), `parser.rs`
   + its tests (2237) and the ~250-reference `SuggestionKind` rename at once. Merging 5.1d with 5.1e, or 5.3e with
   5.3f, would cut two agents at the cost of larger diffs per review. *Default: seventeen.*
10. **Where the `disableForCommands` rule is stated.** `SpecLookup::disables` in the specs domain, called first by the
    adapter and by `InMemorySpecRepository`, or adapter-only with the rule dropped from the trait doc. *Default: the
    domain (P13): it is business logic, and the double then honours the trait contract.*

## Launch

From the repo root, with the gate green at the end of Phase 4:
1. Both files already sit at `docs/hexagonal-refactor/phase-5-specs.md` and
   `docs/hexagonal-refactor/phase-5-specs.args.json`, untracked at `0a0a74e`. Commit them
   (`docs: hand Phase 5 over with its research`, as `6f38da1` did for Phase 4): the step prompts point there, and
   5.1a's step 0 stops on a dirty `git status`. `startRev` is the HEAD after that commit.
2. Run the workflow:

```js
Workflow({
  scriptPath: "docs/hexagonal-refactor/phase-workflow.js",
  args: /* contents of phase-5-specs.args.json, with startRev = HEAD at launch */
})
```
