# Phase 6 (completion): preparation

Status: **prepared, not started; re-checked against `ff2db5a`, the end of Phase 5.** Phase 5 closed at `d7079b9`
(gate green: 851 passed, 0 failed, 1 ignored, clippy 3 warnings, golden identical, testlock MISSING 0 and DUPLICATE
0); `75b9334` (docs) and `ff2db5a` (comment-only, `parser.rs:1, 33`, `quickjs_spec_repository.rs:81`,
`architecture_tests.rs:367-378`, `settings_catalog.rs:388`) followed it and change no behaviour.

How it was researched:
- Six slice researchers read a frozen copy of `c2f0825` (taken before Phase 5 runs 4-5): **tokenizer** (`TOK`),
  **suggestions** (`SUG`), **parser** (`PAR`, traps `PAR-T`), **generators** (`GEN`, traps `GEN-GT`), **engine-server**
  (`ENG`, traps `ENG-E`) and **consumers** (`CON`, traps `CON-T`); a seventh (`GHO`) prepared Phase 7 on the same tree.
- A critic checked the seven reports for gaps and conflicts (items `C`, `U`, `TC`, `X`, and the unclaimed source-map
  traps `M1`-`M10`).
- Three designers wrote whole-phase proposals: behaviour-first (**BF**), plan-letter (**PL**) and seam-first (**SF**).
  A judge scored them (BF 47, PL 45, SF 44 out of 60) and took BF as the base, grafting named parts of PL and SF. The
  judge's decisions **Q1-Q33** are binding and are restated below with their content.
- A recheck read every claim of the decisions against `ff2db5a` (46 commits after `c2f0825`) with `git show`/`grep`
  and a read-only testlock run. Its corrections are folded in below and marked **(recheck)**.

The research files sit in `target/hexagonal-refactor/prep6/` (git-ignored, not committed); this file is the merged,
committed record. The 28 step prompts (ids `6.0a` … `6.7`, see "Step briefs") are in `phase-6-completion.args.json`,
next to this file.

Plan reference: [the plan](../hexagonal-refactor-plan.md).
- Section 5, "Phase 6: completion" (steps 1-7) and "Phase 7: ghost_text" (Phase 7 consumes what Phase 6 leaves).
- The `completion`, `bootstrap` and `terminal_session` layouts and the engine-server event table in section 4.
- Sections 1 (Rust conventions: one type per file, driver-owned aggregates passed `&mut`, DTOs, doubles in the port
  file), 2 (allowed dependencies, bridges), 7 (risks: `Rc` identity, `GeneratorKey::for_arg`), 8 (verification,
  smoke items) and 9 (D1-D9, and the "Phase 5 notes", plan lines 987-1154, whose Phase 6 hand-offs are at 1111-1146).

Detailed traps per legacy file are in [`source-map/shell_parser.md`](source-map/shell_parser.md),
[`suggest.md`](source-map/suggest.md), [`parser.md`](source-map/parser.md), [`engine.md`](source-map/engine.md),
[`types.md`](source-map/types.md), [`pty.md`](source-map/pty.md), [`inline.md`](source-map/inline.md) and
[`cli.md`](source-map/cli.md) (line numbers at `af1b304`). This file restates each trap Phase 6 can break, cited at
`ff2db5a`.

**Line numbers.** This file cites **`ff2db5a`** unless it says otherwise.
- Untouched since `c2f0825` (`git diff --quiet c2f0825 ff2db5a -- <file>`): `pty.rs`, `types.rs`, `suggest.rs`,
  `menu.rs`, `render.rs`, `icons.rs`, `inline.rs`, `cli.rs`, `shell_parser.rs`, `bootstrap/`, `main.rs`, `shared/`,
  `history/`, `scripts/`, `tests/`. Research numbers for these files hold as they are.
- `engine.rs` moved. From `c2f0825` to `ff2db5a`: lines 1-26 +0; 27-44 +1; 45-87 +2; `spawn`'s closure rewritten
  (now 82-111); 92-346 +12; 349-361 +11; 362-370 +12; 371-1269 +13 (the whole `impl Engine`: `run` 408 → 421,
  `compute` 515 → 528, `pump` 1010 → 1023, `reply` 1088 → 1101); tests from 1290 on: helpers `parser_over` (1393-1397)
  and `engine_with_wrapper_env` (1399) inserted, the later tests +22 (`runtime_with_inbox` 1470 → 1492).
- `parser.rs` shrank from 1211 to 939 lines (5.4b moved the loader into `QuickJsSpecRepository`). From `c2f0825`: the
  vocabulary 43-115 → 36-108; the argv state machine 514-879 → 222-587 (-292); `ParseCaches` 881 → 589;
  1090-1112 -287; 1116-1160 -281; 1168-1175 -278; 1178-1211 -272 (the generateSpec warn 1191 → 919).
- `parser/tests.rs` is 780 lines with 23 `#[test]`s (26 at `c2f0825`: `diff_merge` and `version_pick` went to
  `fig_version_diff.rs`, `cli_version_detection_survives_an_aborted_parse` to `quickjs_spec_repository.rs`).
- Each step moves code for the next one. Every brief locates sites **by symbol or content**, never by these numbers.

**What Phase 5 leaves for Phase 6** (plan section 9, "Phase 5 notes"; recheck R1-R14).
- `Parser { repository: Rc<dyn SpecRepository>, runtime: Rc<dyn SpecRuntime>, caches: ParseCaches }`
  (`parser.rs:120-124`): `Parser::new(repository, runtime)` (127), `parse(&mut self, cmd: &CommandInfo, ctx:
  &ParseCtx)` (146), `clear_caches(&mut self)` (137-143) = `self.repository.forget_loaded_specs();` then
  `ParseCaches { first: self.caches.first.clone(), ..ParseCaches::default() }`, and `#[cfg(test)] pub fn load_spec`
  (159-171), whose only caller is `spec_identity_survives_dev_reload_and_cache_clear` (`parser/tests.rs:758-770`).
- `trait Host` (`parser.rs:192-197`) is already typed: `fn call(&mut self, call: SpecFunctionCall) ->
  Result<SpecFunctionResult, ParseError>`. The parser test `Mock` implements it (`parser/tests.rs:105`) and already
  wraps its JSON callbacks (`parser/tests.rs:115-120`): `let context = GeneratorContext::default(); let (f, args) =
  call_arguments(&call, &context); … Ok(function_result(call.kind(), &v, &context))`. There is **no** `Functions` type
  at HEAD: `type Callback = Box<dyn Fn(&[JsArg]) -> Result<Value, ParseError>>` (77) and `fns: HashMap<SpecFunctionRef,
  Callback>` (90).
- `spawn` (`engine.rs:82-111`) builds, inside the `engine` thread: the inbox (88) → `QuickJsSpecRuntime::new(inbox
  .clone(), &settings)` as `Rc` (on `Err`: `engine: js host: {e:#}` and return, 90-96) → `SharedDependencies::from_env()`
  (97) → `Rc::new(QuickJsSpecRepository::new(runtime.clone(), EmbeddedSpecBundle::new(dependencies.data_directory
  .clone())))` (98-101) → `Parser::new(repository, runtime.clone())` (102) → `Engine::new(tx, settings, parser,
  &dependencies).run(&runtime, &inbox)` (103). `QuickJsSpecRepository::new` takes the **concrete**
  `Rc<QuickJsSpecRuntime>` (`quickjs_spec_repository.rs:41`); only the parser gets `Rc<dyn SpecRuntime>`.
- `Engine::new` (359-378): `build_specs_module(dependencies).install_spec_bundle.execute().and_then(|()|
  bundle.load_index())`, on `Err` `engine: specs: {e:#}` and `SpecIndex::default()` (the ponytail double ensure kept),
  then `Engine::with(tx, settings, parser, index, build_history_module(dependencies),
  dependencies.data_directory.state_file().ok())`. `Engine::with` (380-418) loads recency from the state path (391) and
  snapshots `std::env::vars()` (403).
- The engine's generator state machine is still on the runtime's JSON API: `call_json` (`engine.rs:648, 693`, with
  `SYNC_CALL` at 652, 697), `start_call` (829, 847, 873, 968, 985), `wait_json` (1033); `start_sleep` (765),
  `start_exec` (944) and `forget` (1054, 1065) already go through the port. The port methods `start_exec`, `start_sleep`, `forget`,
  `apply_settings` exist only in `impl SpecRuntime for QuickJsSpecRuntime` (`quickjs_spec_runtime.rs:470-544`), so
  `engine.rs` imports the trait (27). The JSON methods stay alive after 6.4d anyway: `QuickJsSpecRepository` calls
  `call_json` (152), `start_call` (214), `wait_json` (220) and `load_module` (106, 159).
- `SpecRuntime` (`spec_runtime.rs:19-46`) has `start`, `start_exec`, `start_sleep`, `wait`, `forget`, `call`,
  `apply_settings`; no `read`/`read_exec_stdout` yet. `InMemorySpecRuntime` (`spec_runtime.rs:54-138`) has
  `answering(function, answer)` and `calls()`; its tasks never finish, `apply_settings` is a no-op, and its ponytail note
  (53) says "Phase 6.4 adds scripted task completion". `ExecRequest` has no `with_timeout` yet.
- `InMemorySpecRepository` (`spec_repository.rs`): `with(specs: &[(&str, &Rc<Subcommand>)])` (37), `finds()` (48);
  `find` records the location first (60), then `if lookup.disables(location) { return Ok(None); }` (61-63), keys `name`
  / `format!("{}|{name}", dir.display())` (64-67); `forget_loaded_specs` is a no-op (71); `#[derive(Default)]` (29). No
  `forgets()` yet.
- `SpecLookup::new(settings, index, generator_context)` (`spec_lookup.rs:16-20`), built per find by `JsHost::load`
  (`parser.rs:207-211`).
- `fig_spec_mapper.rs`: `function_result(kind, v: &Value, context) -> SpecFunctionResult` (483-513, `pub(crate)`),
  `spec_value_to_json` (463), `spec_value_from_json` (438). `script_resolution` (525-539) is **private** and reached
  through `function_result(SpecFunctionKind::Script, ..)` (502).
- `disableForCommands` is already settled by 5.5: `SpecLookup::disables` inside every `find`; plan Phase 6 step 3
  (plan line 645) and section 4's policy line (361) already say so; `parser.rs` has no copy of it.
- `REGISTERED_MODULES = ["history", "settings", "shared", "shell_integration", "specs"]` (`architecture_tests.rs:9`).
  `"parser"`, `"engine"`, `"suggest"`, `"shell_parser"`, `"types"`, `"inline"` … are in `LEGACY_MODULES` (15-33).
  `src/terminal_session/` exists and stays unregistered until Phase 9.
- The specs glossary rows (`SpecLookup`, `SpecRepository`, `SpecRuntime`, `ubiquitous-language.md:95-98`) exist. The
  `host / pump` row (192) already says "…The pumps become `pty_reader`, `ProgressGeneratorsUseCase` and runtime
  internals"; the `cache` row (193) already says "first-token cache (in `CompletionSession`)".
- testlock at `ff2db5a` (read-only): against `ef807c1` 287 → 856 tests, MISSING 0, CHANGED 104, DUPLICATE 0; against
  `phase5-start` MISSING 0, CHANGED 46, DUPLICATE 0. Many tests Phase 6 changes are already CHANGED against `ef807c1`
  (`parse_arguments_table`, `generate_spec_merge`, `git_alias_directive`, `first_token_sources`, `list_dir_sorted`,
  `recency_round_trip_keeps_other_keys`, `enter_records_the_submitted_line`, `debug_shell_survives_closed_terminal`,
  `session_update_refreshes_first_token_items`, …), which is why Phase 6 judges its own edits against `phase6-start`.

## Where the code is today (`ff2db5a`)

| What | Where | Used by | Goes to (step) |
|---|---|---|---|
| `Token`, `CommandInfo` | `shell_parser.rs:8-29` | `engine.rs:541` (`current_command`), 611 (`cmd.original_text`); `parser.rs:5`; `parser/tests.rs:5` | `token.rs`, `command_line.rs` VOs (6.1b), private (6.1c) |
| `current_command`, `parse_aliases`, `substitute_token`, private `Kind`/`Node`/`P`/`descendant_at`/`reduce`; 16 tests | `shell_parser.rs:34-585`, tests `586-1049` | `engine.rs:5` (doc), 22, 513 (`parse_aliases`), 541; `parser.rs:218` (`substitute_token`) | `completion/domain/services/command_line_tokenizer.rs` by `git mv` (6.1a); `AliasTable` + `parse_aliases` (6.1b) |
| `Insertion`, `GenKey`, `Item` (+ `title`, `first_name`), `ArgInfo`, `Phase`, `Completion` (+ `error`), `Request` | `types.rs:11-144` | built by `suggest::{to_item, insertion}`, `engine::{reply, error}`; read by `menu`, `render`, `icons`, `inline`, `pty` | completion VOs/DTO under glossary names (6.2a), private (6.2b) |
| `Msg` | `types.rs:147-157` | pty loop, engine replies | stays (Phase 9) |
| `EngineMsg` | `types.rs:159-183` | `engine.rs`, `pty.rs` sends, `bootstrap/cli.rs` | `bootstrap/engine_message.rs` as `EngineMessage` (6.6b) |
| `now_ms`, `now_secs`, `Model` | `types.rs:186-196` | engine (`now_ms`), pty | stay (Phase 11); `now_ms` loses its engine callers |
| `Query` (+ `of`), `after_sep` | `suggest.rs:24-57` | engine `queries`/`reply`, suggest | `suggestion_query.rs` (6.2c) |
| `Sugg` (+ `new`, `first`, `from_sub`, `from_opt`, `from_suggestion`) | `suggest.rs:59-176` | engine, suggest | `suggestion_candidate.rs` (6.2c), private (6.2f) |
| `cmp_locale`, `cmp_first` | `suggest.rs:179-187` | filter, assembler, `list_dir` | `suggestion_filter.rs` / private in `suggestion_assembler.rs` (6.2d) |
| `GenView` | `suggest.rs:192-199` | engine `reply` | `generator_slot_view.rs` (6.2c, `key: GeneratorKey`) |
| `static_options`, `short_name`, `assemble`, `option_chain` | `suggest.rs:201-351` | engine `reply` | `suggestion_assembler.rs` (6.2d) |
| `history_rows`, `mix_history` | `suggest.rs:356-432` | engine `reply` | `history_suggestions.rs` (6.2d) |
| `update_priorities`, `same_row`, `dedupe`, `rank` | `suggest.rs:437-457, 696-735` | engine `reply` | `suggestion_ranker.rs` (6.2d) |
| `FilterOpts` | `suggest.rs:459-467` | engine `reply` | `filter_options.rs` (6.2c), private (6.2f) |
| `fuzzysort` | `suggest.rs:469-555` | filter, finalizer | `fuzzy_matcher.rs` (6.2d) |
| `prefix_score`, `partial`, `auto_exec`, `filter` | `suggest.rs:557-694` | `rank` | `suggestion_filter.rs` (6.2d) |
| `InsCtx` | `suggest.rs:740-748` | engine `reply` | `insertion_context.rs` (6.2c), private (6.2f) |
| `name_match`, `is_var_ref`, `escape_posix`, `escape_pwsh`, `At`, `unquote`, `escape_in_quote`, `insertion` | `suggest.rs:750-989` | `to_item` | `insertion_composer.rs` (6.2d) |
| `args_hint`, `highlight`, `to_item` | `suggest.rs:991-1064` | engine `reply` | `suggestion_finalizer.rs` (6.2d) |
| `triggered`, `dir_changed`, `cache_key` | `suggest.rs:1069-1092` | engine `update_slots`, `begin`, `fetch` | `generator_trigger_policy.rs` (6.4a) |
| `Lookup`, `CacheEntry`, `GenCache` | `suggest.rs:1094-1177` | engine `fetch`, `advance`, `pump` | `cache_lookup.rs`, `generator_cache.rs` generic (6.4a) |
| `shell_expand`, `inserted_directory`, `help_items` | `suggest.rs:1182-1275, 1326-1353` | engine `update_slots`, `begin` | `path_template.rs` (6.2d) |
| `list_dir` | `suggest.rs:1278-1324` | engine `begin` | `FsDirectoryLister` behind `DirectoryLister` (6.2e) |
| `is_root_spec`, `exe_name` (`cfg(unix)` 1367-1371, `cfg(not(unix))` 1373-1380), `executables`, `first_token_items` | `suggest.rs:1358-1431` | engine `first_token_items` | `first_token_suggestions.rs` + `PathExecutableCatalog` behind `ExecutableCatalog` (6.2e) |
| 23 tests | `suggest.rs:1434-2508` | | see "Test moves" |
| `AnnKind`, `Annotation`, `ParseResult`, `ParseError` + `From<SpecError>`, `no` | `parser.rs:36-108` | suggest (`ParseResult`/`AnnKind` at `suggest.rs:5, 202, 268, 1327`), engine | `annotation_kind.rs`, `token_annotation.rs`, `parsed_command_line.rs`, `completion_error.rs` (6.3a), private (6.3b); `no` stays with the parser |
| `ParseCtx` | `parser.rs:111-118` | engine `compute` | `parse_context.rs` (6.3e) |
| `Parser` (`new`, `clear_caches`, `parse`, `#[cfg(test)] load_spec`), `dev_mode`, `gen_ctx`, `TIMEOUT` (34) | `parser.rs:34, 120-190` | engine (field, `compute`, Settings arm), spawn | `CommandLineParser` (6.3d in `parser.rs`, 6.3e in `command_line_parser.rs`); `load_spec` deleted (6.3d) |
| `Host`, `JsHost` | `parser.rs:192-220` | `ParseCaches` | deleted (6.3d) |
| `ArgState`, `mandatory_or_var`, `State`, `ann`, `update`, `subcommand`, `option`, `option_arg`, `subcommand_arg`, `option_or_chain`, `result` | `parser.rs:222-587` | `ParseCaches` | `argv_parser.rs` with the `ArgvState` API (6.3c) |
| `ParseCaches`, `MAX_CACHED` (599), `MAX_DEPTH` (601); `parse`, `parse_prefix`, `substitute_alias`, `apply_load_spec`, `generate_spec` | `parser.rs:589-939` | `Parser` | `ParsePrefixCache` (6.3d) + the private `ParsePass` of `CommandLineParser` (6.3d) |
| 23 parser tests, `Mock` (`impl Host for Mock` 105), helpers | `parser/tests.rs` | | `command_line_parser/tests.rs` by `git mv` (6.3e) |
| `SLICE`, `SYNC_CALL`, `FIRST_TOKEN_TTL`, `RECENCY_KEY`, `Recency` | `engine.rs:71-78` | engine | runner / trigger policy / session / recency adapter (Q7, Q10) |
| `spawn` | `engine.rs:80-111` | `pty.rs:461` | `bootstrap/engine_server.rs` (6.6b) |
| `EngineInbox` + `impl RuntimeEventSource` | `engine.rs:113-193` | `spawn`, `run` | `bootstrap/engine_inbox.rs` (6.6b) |
| `load_recency`, `save_recency` | `engine.rs:195-228` | `Engine::with`, `record` | `StateFileRecencyRepository` (6.4b) |
| `hash2`, `gen_key` | `engine.rs:230-240` | `compute` (arg key, 565), `update_slots` | `generator_key.rs` (6.2a) |
| `is_path_template`, `script_timeout` (f64) | `engine.rs:242-246, 263-265` | `update_slots`, `begin`, `script_input` | `generator_trigger_policy.rs` (6.4a) |
| `env_var` (`cfg!(windows)`) | `engine.rs:248-261` | `compute` (588) | `completion_request_policy.rs` (6.5b) |
| `llm_history` | `engine.rs:267-270` | `llm` | the legacy ghost block (Q14) |
| `Slot`, `Step`, `Run`, `Outcome` | `engine.rs:273-314` | generators | `generator_slot.rs`, `generator_step.rs`, `generator_run.rs`, `generator_outcome.rs` (6.4e) |
| `Current` | `engine.rs:317-329` (built 607-618) | `reply`, `ghost`, `pump` | `served_request.rs` (6.4e) |
| `Engine` fields | `engine.rs:331-356` | | `CompletionSession` (6.4e) + `EngineServer` (6.5c, 6.6b) |
| `Engine::{new, with}` | `engine.rs:359-419` | `spawn` | spawn order (6.5c) |
| `run`, `handle` | `engine.rs:421-495` | `spawn` | `EngineServer::{run, handle}` (6.5c, 6.6b) |
| `reset_slots`, `env`, `alias_map` | `engine.rs:496-527` | engine | `CompletionSession` (6.4e) |
| `compute`, `error`, `queries` | `engine.rs:528-669` | `run` | `CompleteCommandLineUseCase` + `CompletionRequestPolicy` (6.5b) |
| `update_slots`, `is_current`, `settle`, `begin`, `fetch`, `script_input`, `exec_output`, `custom_result`, `advance`, `pump` | `engine.rs:670-1078` | `compute`, `run` | `GeneratorRunner` (6.4f), `GeneratorRun` transitions (6.4g), `ProgressGeneratorsUseCase` (6.5c) |
| `first_token_items` | `engine.rs:1079-1100` | `compute`, `reply` | `CompletionSession::first_token_suggestions` + use case (6.4e, 6.5b) |
| `reply` | `engine.rs:1101-1187` | `compute`, `pump` | `completion_result_builder.rs` (6.5b) minus the ghost line |
| `ghost` | `engine.rs:1188-1206` | `reply`, `error` | `EngineServer::local_ghost_text` (legacy ghost block) |
| `record`, `save_recency_now` | `engine.rs:1207-1246` | `handle`, `run` | `SuggestionRecency` + `RecordSuggestionInsertionUseCase` + adapter (6.4b, 6.5a) |
| `llm` | `engine.rs:1247-1277` | `handle` | `EngineServer::request_llm_ghost_text` (legacy ghost block) |
| 12 engine tests + helpers `parser_over`, `engine_with_wrapper_env`, `wrapper_env`, `report_session_context`, `runtime_with_inbox` | `engine.rs:1279-1578` | | see "Test moves" |
| the engine's sender and nine sends | `pty.rs:461` (spawn), sends 790-791, 871, 1136, 1182, 1240, 1284, 1299, 1308 | | `CompletionEngine` port calls (6.6a) |
| `cli::debug`, `debug_shell` | `cli.rs` (re-exec test `debug_shell_survives_closed_terminal` 830-883) | `bootstrap/cli.rs:139, 141` | gain the starter argument (6.6a) |

## Behaviour that must not change

Trap ids carry the phase prefix `P6-` plus the research id: `P6-PAR-T1` is the parser report's trap T1, `P6-GEN-GT24`
the generators report's GT24, `P6-ENG-E3`, `P6-CON-T16`, `P6-M3`. The tokenizer and suggestion reports had no
numbers; their traps are numbered here (`P6-TOK-n`, `P6-SUG-n`). The decisions below cite the same traps without the
prefix (`ENG-E24` = `P6-ENG-E24`). Never cite a bare `T`/`GT`/`E`.

### Tokenizer (6.1) — `shell_parser.rs` moves verbatim
- **P6-TOK-1 Absolute-offset literal** (`shell_parser.rs:569`): `if cmd.children.last().map_or(1, |c| c.end) <
  cmd.end && text.ends_with(' ')`. The `1` is compared with the absolute `cmd.end`, which is why `cmd(" ") == None`
  (628, 636) while `toks("  ") == [""]` (634) and `toks("FOO=bar ") == [""]` (651). Never "fix" it to `cmd.start`.
- **P6-TOK-2 Space only, not tab**, for the trailing empty token (569), while `words()` splits arguments on space
  **and** tab (381). `"git\t"` gets no trailing token.
- **P6-TOK-3 Two whitespace predicates.** `words()` treats `\n` as a terminator (366 `terms = b"&|;\n'\"`"`, 381);
  `next_word` skips any ASCII whitespace (243). Never unify them.
- **P6-TOK-4 Empty command** (`command()` 398-408): `if has { end } else { self.s.len() }`, `complete = has`. A
  command with no word runs to EOF: `"ls | "` yields command text `" "` (`spec_table`, 619).
- **P6-TOK-5 Command text runs to the operator, trailing spaces included** (`words()` 371-391):
  `descendant_at(&tree("git co && ls -l"), 3)` is `(0, 7)` (777); `token_offsets` (784-792) pins `c.text == "git  co "`
  and `&c.text[t.start..t.end] == t.raw`.
- **P6-TOK-6 `None` versus `Some`** (`current_command` 61, `(!cmd.tokens.is_empty()).then_some(cmd)`; 40 `?`).
  `engine.rs:541-543` turns `None` into `self.error(req, cursor, true)`, an error reply that may carry history ghost
  text (`error`, 622-631). The mid-word guard (530-538) replies before the tokenizer runs.
- **P6-TOK-7 `Token.raw` is the exact source slice** (`self.text(c)`, 563). The last token's raw becomes
  `ParseResult.raw_last_token` (`ParseCaches::parse`, `parser.rs:611-640` → `result` 538-587), and the engine strips
  it to compute the insertion prefix: `before.strip_suffix(pr.raw_last_token.as_str())` (`engine.rs:602-606`). A word
  starts **at** the escape char (384-385), so `cd \ fo` has raw `\ fo` (`escapes` 712-716).
- **P6-TOK-8 Byte offsets.** `P::text` is `&self.src[n.start..n.end]` (211); node ends are ASCII syntax chars or EOF
  (module doc, line 4); `multibyte_and_no_panics` (794-812) runs every char prefix of three strings × `Shell::ALL`.
  `Token.start`/`end` are byte offsets consumed by `substitute_token` (94-96) and the parser. Never char indices. A
  trailing escape skips past EOF: `let i = i.min(s.len());` (392).
- **P6-TOK-9 Unquoting** (`P::inner` 215-240): Word drops every escape char; Str drops it only before `$ ` " \\ \n`
  **or EOF** (232); RawStr, AnsiC, Expansion, CmdSubst and Arith stay verbatim; delimiters strip `(1, 1)` for
  Str/RawStr, `(2, 1)` for AnsiC, the closing one only when complete (219-225); the Str escape list hard-codes `\\` and
  `` ` `` whatever the shell; `literal()` honours the escape for every kind except RawStr (326) while `inner()` leaves
  AnsiC verbatim (`$'c\'d'` → `c\'d`, `escapes` 711); `String::from_utf8_lossy` (239).
- **P6-TOK-10 Expansions** (`simple_expansion` 278-293): `$` + one of `*@?-$0_` is a 2-byte SpecialExp (`$_foo` =
  `$_` + `foo`); the terminator set `b"\t \n$\\"` (283) hard-codes the backslash; a lone `$` before a terminator stays a
  word char, `$` at EOF is a 1-byte SimpleExp (750-751).
- **P6-TOK-11 pwsh** (200-207, 265, 270): the escape is a backtick, no backtick substitution, no `$'…'`, `\` plain, `''`
  not an escape, a leading `&` is an operator (ponytail 190-191); fish uses the bash rules (`P::new` checks only
  `Shell::Pwsh`). Test `pwsh` 939-951.
- **P6-TOK-12 Grammar.** `words()` checks the terminal (372) before `child_at` (375); `descendant_at` (166-175) has an
  inclusive end, first matching child wins, deepest Command returned, called with `buffer_before_cursor.len()` (40);
  `|`/`|&` bind tighter than `&&`/`||` by re-association (499-503); `reduce` flattens only same kinds (180); `;`, `&`,
  `&;` end a statement (489); `op()` is greedy for `&; |& && ||` (252); `}` counts only after a separator (506-523), so
  `{a; b}` is incomplete (772); an AssignmentList is always complete (454, 534-537); assignments only at statement
  start, `[A-Za-z0-9_\[\]]+\+?=` (412-420), an empty value is complete (434), the value parsed with terminal `b' '`
  (432); `cmd_subst`, compound and subshell are complete only when terminated and non-empty (308, 479). A very long
  `a && b && …` chain recurses once per operator (492) and can overflow the engine stack: pre-existing, report only.
- **P6-TOK-13 Alias loop** (`current_command` 42-60): only `tokens[0]` expands, only while `cmd.tokens.len() > 1` (44),
  each name once (`used`, 43, 49), an empty value breaks (46); a failed `substitute_token` keeps `cmd` (56-58); the
  lookup key is the **unquoted** `text` (45). Tests `alias_expansion_cases` 815-830, `aliases_expansion` 833-884.
- **P6-TOK-14 Two quote strips** (D1): `current_command` strips `'…'` only when both ends are there (52-55);
  `parse_aliases` strips each end independently (74-75). Keep both.
- **P6-TOK-15 `parse_aliases`** (64-80): `text.lines()` swallows `\r\n` (zsh input, 897); a missing `alias ` prefix
  falls back to the **original** line (`aliasfoo=bar` → key `aliasfoo`, 69-71); `split_once` at the **first**
  separator (73); separator `' '` for fish only, `'='` otherwise (66); empty key or value dropped (77); last duplicate
  wins (79). No test pins the fallback, the one-sided strip or last-wins: keep the body verbatim.
- **P6-TOK-16 Alias order.** The map is a `HashMap`; its one iterating consumer sorts the `(k, v)` pairs
  (`suggest.rs:1417-1418`), so a spec name beats a same-named alias through `seen` (1407). `AliasTable` must let that
  caller sort `(&String, &String)` pairs (Q28: it is the std map).
- **P6-TOK-17 `substitute_token`** (82-119): exactly one top-level plain Command (`let [node] = …`, 100-105), so `|`,
  `&&`, `;`, `FOO=1`, `{`, `(` give `None`; `get(..)?` never panics (91, 94-96); the final length check (118) rejects
  an empty replacement (934). Raw remap (108-117): alias tokens take the replaced token's raw through
  `command.tokens.get(k - diff)?.raw.clone()` (115); `in_alias` is strict `<` with `node.start` added (111);
  `original_text` is **copied** (107), so chained expansions keep `"ll "` (843-846).
- **P6-TOK-18 `original_text` versus `text`.** `text` is the expanded command source, `original_text` the command as
  typed (`"echo | g ch"` → `"g ch"`, 848-852). The engine sends `original_text` to the menu (`engine.rs:611` →
  `Completion.command_text` → `menu.rs:117, 128`). 6.1c writes `cmd.original_text().to_string()` there.
- **P6-TOK-19 Load-bearing derives.** `Clone` (`parser.rs:684`, `let mut cur = cmd.clone();`), `PartialEq + Debug`
  (`assert_eq!(cmd(..), None)` 628-630, 649-650; `assert_eq!(substitute_token(..), None)` 933-935), `Kind: Copy +
  PartialEq + Debug` in the tests.
- **P6-TOK-20 Keys that read `Token.text`**: the prefix-cache key `texts[..len - 1].join(" ")` (`parser.rs:658-666`),
  the generateSpec key `{tokens[0]}:{k}`, the generator cache-key fallback `run.tokens.join(" ")`
  (`engine.rs:863`). Any change to `inner()` changes them.
- **P6-TOK-21 No `Rc`, statics, threads or logging.** The tokenizer runs on the engine thread; the pty sends only the
  raw alias **text** (`EngineMsg::Session { alias }`), parsed lazily per request shell (`alias_map`,
  `engine.rs:510-522`). Never move the parse into the pty or into the session-context use case; add no log line.
- **P6-TOK-22 Test-file mechanics.** `parse_tree_fixtures` (955-1048) `include_str!`s
  `../tests/fixtures/shell_parser/{basic,multipleStatements,primaryExpressions,variables}/{input.sh,output.txt}`
  (1011-1027), relative to the file holding the macro; from
  `src/completion/domain/services/command_line_tokenizer/tests.rs` the prefix is `../../../../../tests/fixtures/…`
  (eight paths). It keeps the `count > 60` guard (1046). Its `serde_json` use passes the fitness test (D2;
  `architecture_tests.rs` treats `*/tests.rs` as test-only; rule 8 flags only the macro `include`). The tests read
  private items (`P::new`, `p.s`, `p.inner`, `Node`, `Kind`, `descendant_at`), so the file stays a child
  (`#[cfg(test)] mod tests;` + `use super::*;`); once the parent stops importing `HashMap` (6.1b), the test file needs
  its own `use std::collections::HashMap;` (895, 1032).

### Wire rows and the engine ↔ pty plumbing (6.2a-b, 6.6a)
- **P6-CON-T1 `CompletionResult::error` is phase `Parse`** (`types.rs:112-131`: `phase: Phase::Parse, error: true`,
  everything else empty/0/false). `pty.rs:886-892` arms the LLM debounce only when `c.phase == Phase::Parse &&
  c.ghost.is_none() && c.cursor == c.buffer.len()`. A dedicated error phase silently stops LLM ghost text for commands
  without a spec. Guard: `llm_ghost_requested_for_commands_without_spec` (`pty.rs:1995`). Keep the name `error`.
- **P6-CON-T2 The menu keeps the last reply with its items taken out** (`menu.rs:105-109`: `if c.error { return
  self.error_reset(); } let items = std::mem::take(&mut c.items);`; 158 `self.last = Some(c);`). Readers of that
  half-empty reply: `render.rs:406`, `pty.rs:1159, 1251`, `menu.rs:122-131, 367-371`. So `take_suggestions(&mut self)`
  (a `mem::take`), never a consuming `into_parts`.
- **P6-CON-T3 Generator identity is a plain value** (`gen_key`, `engine.rs:237-240`); the menu compares keys by value
  (`menu.rs:136-138`), overwrites `last_inserted_gen` with `item.generator` even when `None` (373); test literals `7`
  (618, 625). `triggered_generators` is filled for `Parse` only (`engine.rs:1175-1179`). The key crosses threads in
  `Msg::Completion`: it stays `Copy + Send`.
- **P6-CON-T4 Insertion counts are chars** (`pty.rs:146-152`: `repeat_n(0x7f, ins.delete)`, `\x1b[D` per
  `cursor_back`, `\r` if `execute`; 166-168 `s.pop()`); `inline.rs:146-150` compares `cursor_back` in bytes (safe
  because one `' '` is stripped). `Insertion` keeps `Clone, Debug, Default, PartialEq, Eq` (`menu::Effect` derives
  `PartialEq`, `menu.rs:24-35`; tests `assert_eq!` effects, 579-591).
- **P6-CON-T5 `execute` is set on a clone, never for an auto-execute row** (`menu.rs:361-364`). With private fields:
  `with_execute()` on the clone under the same condition.
- **P6-CON-T6 `Suggestion` has no `PartialEq`** (`types.rs:29`, `Clone, Debug`); `same_item` compares `names`, `kind`,
  `description` (`menu.rs:73-75`). A derived `PartialEq` would compare `insert.text` and break the sticky test
  (802-826).
- **P6-CON-T7 `title()` / `first_name()`** verbatim (`types.rs:57-66`); `render.rs:513` highlights char indexes into
  `title()`; `menu.rs:79` compares `"../"`; `menu.rs:367` skips recency for names containing `'↪'`.
- **P6-CON-T8 `cursor` is a byte offset; the reply echoes the clamped value** (`engine.rs:529`, reply 1165;
  `pty.rs:888`).
- **P6-CON-T9 `cwd: PathBuf`, `shell: Shell`** (`types.rs:141-142`): the parse reads `&req.cwd` as a `Path`
  (`engine.rs:549-554`); only the generator context converts lossily (582). A `String` loses non-UTF-8 cwds.
- **P6-CON-T10 One inbox for the pty loop** (`pty.rs:458-461`): engine replies (`Msg::Completion`,
  `engine.rs:442, 1072`; `Msg::Ghost` 1250, 1254, 1270, the last from the `llm` thread) share the channel with
  `Pty`/`Stdin`/`Resize`/`ChildExit`; `pty.rs:621-640` batches `Pty` messages. No second reply channel.
- **P6-CON-T11 Where the engine starts**: inside `Session::start` at `pty.rs:461`, after the renderer, keymap, menu,
  the `Msg` channel and `spawn_resize`, before `openpty` (463), inside `pty::run`'s `catch_unwind` (69-71). An
  injected starter is called at the same line (ENG-E30).
- **P6-CON-T12 Spawn failures and threads** (`engine.rs:85-106`): thread `engine`; QuickJS is `!Send`, so everything
  is built in the closure; `engine: js host: {e:#}` at error (93) and the thread returns; `engine: spawn: {e}` at error
  (106) and the sender is still returned, so the session runs without completions. Other threads: `recency`
  (1223; warn 1229/1243), `llm` (1264; warn 1273).
- **P6-CON-T13 The engine inbox never disconnects** (it keeps its own `sender`, `engine.rs:113-127`); only `Shutdown`
  (`pty.rs:1308`) ends the loop (`engine.rs:491`). `ChannelCompletionEngine` must not change that.
- **P6-CON-T14 Send errors are ignored, never logged** (`let _ = self.engine.send(..)` at `pty.rs:790-791, 871, 1136,
  1182, 1240, 1284, 1299, 1308`).
- **P6-CON-T15 Send order**: `CommandFinished`/`CommandEnded` before `menu.error_reset()` and `poll_files()` (789-797);
  in `poll_files` the session context before the settings (1284, 1299); `ToggleFuzzy` then `rerequest` (1135-1138);
  `RecordInsert` after the lock is set (1180-1183); cleanup = clear, `Shutdown`, `kill_live_children`,
  `modes.restore`, remove the session file (1304-1314). Each port call keeps its position.
- **P6-CON-T16 The recording double records every call, in order, and hands each over once.** `contexts_sent`
  (`pty.rs:2404-2411`) drains with `try_iter()` and panics on any non-`Session` message ("only session contexts are
  sent"); four Phase 4 tests call it twice and expect the second call empty (2441-2445, 2455-2465, 2473-2482,
  2490-2502); `no_history_command_still_ends` (2299-2316) filters out `Complete` and expects exactly `[CommandEnded]`.
- **P6-CON-T17 `Send` across threads**: `CompletionResult`, `Suggestion`, `Insertion`, `CompletionRequest`,
  `HistoryEntry`, `Settings`, `RuntimeEvent` hold no `Rc`/`RefCell`.
- **P6-CON-T18 Dispatch order** (`handle`, `engine.rs:456-495`): see `P6-ENG-E10`…`E13` and the table in Q26/§3.3 of
  the decisions.
- **P6-CON-T19 Local ghost text on every reply** (`reply` 1159 for Parse and Generators, `error` 626-629): see
  `P6-ENG-E16`/`E17` and Q15.
- **P6-CON-T20 The only automated run of the real spawn**: `cli.rs:830-883` `#[cfg(unix)]
  debug_shell_survives_closed_terminal` re-runs itself on a pty (`cb.args([NAME, "--exact", "--nocapture"])`, NAME =
  `"cli::tests::debug_shell_survives_closed_terminal"`) → `debug_shell` → `pty::run` → `Session::start` →
  `engine::spawn`. It is the one automated guard for 6.6a/6.6b. Run it alone by full path where Q33 says.

### Suggestion pipeline (6.2c-6.2f) — `suggest.rs` moves verbatim (all lines below are `suggest.rs` unless named)
- **P6-SUG-1 `Query::of`** (42): `search_term[search_term.rfind(g.as_str()).map_or(0, |i| i + g.len())..]` — byte
  index, empty `g` gives `""`. `derive(PartialEq)` is load-bearing (`if !g.loading || g.query == Query::Term`, 286).
- **P6-SUG-2 `after_sep`** (54-57): `rfind(|c| c == '/' || (backslash && c == '\\')).map_or(0, |i| i + 1)`; four
  callers rely on the byte index (46, 907, 1085, 1262).
- **P6-SUG-3 Two forms of the pwsh flag**: `Query::Path { backslash: shell == Shell::Pwsh }` (`engine.rs:641-643`)
  versus `ctx.shell() == Shell::Pwsh.name()` (`engine.rs:689, 822`). They agree because the context's shell is
  `req.shell.name()` (584). Keep both; never derive one from the other.
- **P6-SUG-4 `Sugg` defaults and builders** (84-175): priority `0.0` = unset, query `Term`; `first()` gives `""` for
  no names (105-107; relied on by `mix_history` 407, 415 and `update_priorities` 440); `from_sub`'s
  `should_add_space` = `mandatory || requires_subcommand().unwrap_or(!subcommands().is_empty())` (110, 122-123);
  `from_opt`'s separator ladder (129-137, an option with no args and `requires_equals` still gets `"="`); 
  `from_suggestion` kind `unwrap_or(Arg)`, `is_dangerous || dangerous`, `priority.unwrap_or(0.0)` (155-175).
- **P6-SUG-5 `args: Vec<Rc<Arg>>` is identity** (cloned `Rc`s, 120, 147; `same_row` uses `Rc::ptr_eq`, 702). `Sugg` is
  `!Send` and lives on the engine thread; the final `Suggestion` crosses threads and must hold no `Rc`.
- **P6-SUG-6 Assembly order** (272-312): sorted subcommands; static arg rows **unsorted**; generator rows per slot in
  slot order **unsorted**; sorted additional suggestions; sorted options; the option-chain rewrite. Dangerous flags:
  static `arg.is_dangerous()` (282), generator `false` (290), additional `comp.is_dangerous()` (301). Stale generator
  rows only when `!g.loading || g.query == Query::Term` (286); path templates get `Query::Path`, so their stale rows hide
  while loading. Additional suggestions are added whatever the `suggest_*` flags say (296), inside a chain only typed
  rows (300). Options are computed even when `!suggest_options` (305), because the chain needs them.
- **P6-SUG-7 `static_options`** (220-254): iteration `comp.options()` → `pr.persistent` → `comp.persistent()`;
  `seen.extend` (230) runs **before** `exclusive_on` (231) and the repeat limit (234-247), so an excluded or used-up
  local option still hides a same-named inherited one; `No | Times(0) => Some(1)` (235); unmet `depends_on` → priority
  75.0 (249-251); separators from the directives (248); `cmp_first` sort (254).
- **P6-SUG-8 `short_name`** (262) and **`option_chain`** (316-351): `last = format!("-{c}")`; keeps only `Option | Arg`
  rows; pushes the matching short option only when `!options_flag`; `Arg` rows get `Query::Value(String::new())`; the
  matching option is renamed **in place** keeping its `insert_value`; others become `names = [chain + x]`. `cmp_first`
  = `cmp_locale(a.first(), b.first())` (185-187); every sort is a stable `sort_by`.
- **P6-SUG-9 `history_rows`** (357-399): newest first (`iter().rev()`, 361); byte-exact `strip_prefix` (362), then
  `trim_end` (365); empty names and any control char skipped (366-373); `first_word` splits on one `' '` (358);
  frequency counted **before** dedupe (376 vs 383); priority `75.0 + f.min(10) as f64 / 10.0` when `f > 1`, else
  `50.0` (389-393); description `"past command"`, `insert_value = name`, kind `History`.
- **P6-SUG-10 `mix_history`** (401-432): `!show` → history rows only; `by_word` last insert wins; `existing` = every name
  of every spec row, compared with the history row's **full** first name (415); spec rows boosted to the max over all
  their names; spec rows then extra history rows.
- **P6-SUG-11 `update_priorities`** (438-457): `0.0` → 50.0; `"../"` never gets recency; recency 0 ignored; clamp to
  0..=100 except `AutoExecute`; `rec.is_some() && (50.0..=75.0).contains(&p)` → `75.0 + boost`, else `p + boost`,
  `boost = ms as f64 / 1e13`.
- **P6-SUG-12 `rank`** (717-735): `update_priorities`, stable sort by priority desc with `partial_cmp(..)
  .unwrap_or(Equal)`, then `filter`, then `dedupe` only when `!out.is_empty() && out.len() <= 50` (730). `dedupe`
  (706-714) keeps the first of rows equal in names, insert value, display name and args by `Rc::ptr_eq`. Today the
  caller holds the recency lock during `rank` (`engine.rs:1139-1146`); after Q10 the ranking reads
  `session.recency().of_command(..)` without a lock: `rank` must stay free of anything that blocks.
- **P6-SUG-13 `filter`** (603-694): empty search term → hidden rows dropped, every row `(s, false)`, `↪ Immediately
  execute` inserted at 0 only when `exec_after_space && !out.is_empty() && !auto_exec_hidden` (604-615; pinned by
  `empty_search_term_is_no_prefix_match`); exact match ignores `hidden` (624), the hidden check is only on the partial
  branch (646); `qx` lowercased (+ `/` for a folder, 623); auto-exec only for the first exact match, only when `plain`,
  not dangerous unless `run_dangerous`, first arg optional or absent (619-644), the folder row named without `/` and
  described `"folder"`, `insert_value = "\n"`, pushed to bucket A **before** the exact row; the display-name partial
  runs for every row (651-655), leaving duplicates only `dedupe` removes; bucket C sorted by score desc then priority
  desc, NaN = Equal, stable (657-663); A ++ B ++ C; the dot / folder / current-token row at 0 only when
  `!out.is_empty() && !added && !auto_exec_hidden` (669-691), with the exact texts `"."`, `"↪"`, `"Enter the current
  directory"`, `"Enter the current argument"`.
- **P6-SUG-14 `partial`/`prefix_score`/`cmp_locale`**: non-fuzzy → best prefix score `(p as f64, true)`; fuzzy →
  `Some(0)` for an empty query, a prefix hit wins as `(p as f64 / 10.0, true)` (574-591); ladder 0, −1, −2, −3
  (558-571); `cmp_locale` = `a.to_lowercase().cmp(&b.to_lowercase()).then_with(|| a.cmp(b))` (179-183), also the
  `list_dir` sort key (1300).
- **P6-SUG-15 `fuzzysort`** (469-555): `None` for an empty query or target; first lowercase char only (474); word
  beginnings on the original case (492-500); no strict match → `score *= 1000`, then `-= (tn - qn)`; char indexes.
- **P6-SUG-16 `insertion`** (893-989): the delete branch order (898-923: AutoExecute → 0; `q == search_term` →
  `raw.len()` chars; `Query::Str(g)` → chars after the last `g` in **`c.raw`**; `Query::Path` → after `after_sep` in
  `c.raw`; else the getQueryTerm tail match over `unquote`, else the `2 per whitespace, 1 otherwise` approximation);
  `quote` and `inside` (924-929); text = `insert_value` unless File/Folder, else `name_match` (934-937); escaping by
  `(inside, shell)` (939-948), the closing quote appended for a File only; separator + `{cursor}` only on the name
  branch (950-953); trailing `\n`s collapse to one, the space added after `{cursor}` when `should_add_space &&
  insert_space` and no trailing `\n` (957-964); don't-retype drains `q.len()` **bytes** (967-970); `execute` detected
  (971) **before** `retain(!is_control)` (977); only the first `{cursor}` is consumed, `cursor_back` in chars
  (979-982). **P6-M3:** a quoted spec name holding `{cursor}` still becomes the marker; the 6.2b/6.2d rewrites of the
  insertion tests keep those inputs.
- **P6-SUG-17 Insertion helpers**: `name_match` (751-781, longest name with strict `>` for `Option | Subcommand`, all
  names only when `display_name` is `Some`); `is_var_ref` (784-790); `escape_posix` (795-812: Unicode
  `is_alphanumeric` + `-_./+,:@%=~`, tilde rule for paths, `$VAR` passthrough for non-paths, backslash-space, folder
  `/` outside the quotes, `shell_quoting::quote`); `escape_pwsh` (816-831: U+2018-201E special, folders
  backtick-escaped even inside an opened quote); `unquote` arm order (846-858: escape arm **before** the closing-quote
  arm); `escape_in_quote` (866-890: bash/zsh `!` → `"'!'"`, fish `"` specials `\ " $`, pwsh slices `quote()` as
  `q[1..q.len() - 1]`). They are **not** duplicates of `ShellQuoting::quote`: never merge them (Q28, SUG14).
- **P6-SUG-18 `to_item`/`highlight`/`args_hint`** (991-1064): `query = s.query.of(c.search_term)` feeds `highlight`
  and the row's `query` (read by `menu.rs:410, 427-445`, `render.rs:405`); highlight offsets add `+ 2` per `", "`
  (1024), matching `title()`'s join; fuzzy keeps the **last** segment among equal scores (`max_by_key`, 1031);
  `args_hint` skips nameless args, `...` for variadic, `[n]`/`<n>`.
- **P6-SUG-19 `shell_expand`** (1183-1252): home = `HOME`, then `USERPROFILE`, else `"~"`; `~` only alone or before `/`;
  pass 1 `$NAME` before pass 2 `${..}` (a value holding `${X}` expands twice); `${X:-}` stays literal; a set-but-empty
  var does **not** fall back to the default; an unset `${X}` stays literal.
- **P6-SUG-20 `inserted_directory`** (1255-1275): expands first, cuts at `after_sep`, base gets `/` unless `cwd` ends in
  `/` (or `\` with backslash) — always `/`, even in pwsh; `Path::is_absolute` is OS-dependent.
- **P6-SUG-21 `help_items`** (1327-1353): trigger = the **last** `Subcommand | Option` annotation; only a `Subcommand`
  trigger with `path.len() >= 2` uses the parent level and excludes the trigger's names; every name of every
  subcommand; rows kind `Special`, template type `"help"`.
- **P6-SUG-22 `first_token_items` order** (1398-1431): root specs (`is_root_spec`, 1359-1365) priority 40, then aliases
  collected and `sort()`ed as `(&String, &String)` (1417-1418) as `Shortcut` priority 100, then PATH executables in
  `split_paths` order, each directory sorted (byte order, 1393) — one `seen` set: a spec name beats an alias or an
  executable. An `ExecutableCatalog` that sorted or deduped globally would reorder rows: it returns the per-directory
  lists concatenated in PATH order.
- **P6-SUG-23 `executables`/`exe_name`** (1367-1395): unreadable dir → `[]`; `metadata` follows symlinks, `is_file()`
  only; lossy names; `cfg(unix)` any x bit keeps the full name; `cfg(not(unix))` PATHEXT default `.COM;.EXE;.BAT;.CMD`,
  `rsplit_once('.')?`, case-insensitive extension, returns the stem. Both twins move verbatim into one file; the
  second compiles only on CI's windows job.
- **P6-SUG-24 `list_dir`** (1279-1324): unreadable dir → `[]` with **no** `../`; `metadata(e.path())` follows symlinks
  (a broken symlink lists as a file); `.DS_Store` skipped case-insensitively; control-char names dropped; dot-names
  last, then `cmp_locale`; `"../"` appended **after** the sort, even for an empty dir; folder-only keeps `../`;
  `insert_value = name` read before `with_names(vec![n])` moves it; template type `"folders"`/`"filepaths"`.
- **P6-SUG-25 Timing of the listing**: `list_dir` runs synchronously inside `Engine::begin` and its rows are
  `Outcome::Done` in the same call (`engine.rs:814-839`), so template rows appear in the **Parse** reply; help rows are
  appended after them (826). `DirectoryLister::list` is synchronous and called at the same point.
- **P6-SUG-26 Timing of the PATH scan**: only on a first-token cache miss (`engine.rs:1079-1098`, 10 s TTL); Settings
  and Session clear the cache (481, 487), CommandFinished does not (`P6-ENG-E22`).
- **P6-SUG-27 Cross-cutting**: no `HashMap`/`HashSet` iteration reaches the output; no logging, errors or exit codes
  in the pipeline; no golden covers it (the 23 tests, the engine/menu/render/pty tests, 6.0a and the plan §8 smoke do);
  rename hazards (`Sugg` prefixes `Suggestion…`; `Item` appears in `TestOnlyItem` and fixture fns; `Query` in
  `getQueryTerm` strings; `"…-ItemType File…"` at `suggest.rs:2283` is test input) — compiler-guided renames only,
  never `sed`.

### Parser (6.3)
- **P6-PAR-T1 The warn line prints the error's derived `Debug`** (`parser.rs:919`): `logger::warn(&format!(
  "generateSpec ({}): {e:?}", tokens[0])); // spec name only: logs never hold typed text`, over `#[derive(Debug)] enum
  ParseError { Aborted, NoSuggestions(String) }` (88-93). Today: `generateSpec (cmd): NoSuggestions("spec is not an
  object")`. Renaming the enum keeps it; renaming a variant, hand-writing `Debug`, a newtype payload or `{e}` changes
  it. `tokens[0]` is the command token, not the spec name.
- **P6-PAR-T2 When the warn fires**: in the generate call's `Err` arm (918-921), before `return Ok(st)`, even when the
  parse later fails or aborts, once per failing call (generateSpec is retried each loop iteration, 694-698; failures
  are never cached, 909-914). Returning texts after the parse would move the line after the adapter's `js
  console.{level}` lines: Q11 keeps the moment with a callback.
- **P6-PAR-T3 `From<SpecError>`** (95-104): `Aborted → Aborted`; `Timeout | Invalid(_) | BundleUnavailable(_) →
  NoSuggestions(e.to_string())`. Explicit arms (D8).
- **P6-PAR-T4 Error texts, verbatim** (all reach `engine: no suggestions: {e}` at debug, `engine.rs:559-562`): `no
  tokens` (611), `no {name} spec` (625), `loadSpec recursion too deep` (656), `failed loading spec for {texts[0]}`
  (680), `cannot consume {token:?}` (711, Debug-quoted), `alias did not return a string` (800), `invalid alias for
  {token:?}` (807), `unknown fn` (the test Mock, `parser/tests.rs:117`). Repository texts arrive as
  `SpecError::Invalid` and pass unchanged.
- **P6-PAR-T5 Aborted propagates, never swallowed**: the single-token load (624), every `parse_prefix` load (674), an
  alias error with `matches!(e, ParseError::Aborted)` even after a substitution (726), a function loadSpec
  (`apply_load_spec`, 812-862), the cache-key call and the generate call (`generate_spec`, 868-939). `compute` then
  returns `None` (`engine.rs:558`) and the `Complete` stays deferred in the inbox. Nothing is cached on an abort (the
  prefix insert is the last statement, 772-778).
- **P6-PAR-T6 The single-token rule is not `spec_location`** (613-630): one token without `/` → the shared `first`
  `Rc`, `spec_name ""`, `first_token = true`, **no load**; one token with `/` → `if last.text == "bin/console" {
  "php/bin-console" } else { "dotslash" }`, a Global load, `no {name} spec` when missing. `spec_location`
  (`spec_locator.rs:10`) also matches `…/bin/console`, so a lone `vendor/bin/console` loads `dotslash` while
  `vendor/bin/console x` loads `php/bin-console`. The rule moves with the parse pass and never calls `spec_location`.
  The load goes through `find`, so `disableForCommands` applies to `dotslash` and `php/bin-console` too.
- **P6-PAR-T7 The final token comes from the original command line** (`last` from `cmd`, 611; passed to
  `update`/`result`, 635-640), never from the alias-substituted `cur`; an unconsumable final token gets an
  `AnnKind::None` annotation, never an error.
- **P6-PAR-T8 Location order** (`parse_prefix` 672-680): the first `Some` wins, `None` tries the next, **any** `Err`
  returns at once (Aborted included); nothing → `failed loading spec for …`.
- **P6-PAR-T9 Prefix-cache key** (659-666): `format!("{},{},{}", texts[..texts.len() - 1].join(" "),
  loc.cache_key(), cwd.display())`, `texts` = the original line at the root, the substituted `cur` for a nested call.
  Keep the ambiguous space join; never key on `SpecLocation` (PathBuf `Eq` ignores the trailing slash).
- **P6-PAR-T10 Cache reads and writes** (667-671, 772-778): dev off → every location's key looked up in order
  **before** any load, a hit returns a clone; dev on → no read, no write; the write only at the end of a prefix parse
  that did not hand over to a nested one; `if self.prefix.len() >= MAX_CACHED { self.prefix.clear(); }` before the
  insert, even when the key is stored. `ParsePrefixCache` keeps this wholesale clear (with its `ponytail:`, 598).
- **P6-PAR-T11 Depth limit** `depth > MAX_DEPTH` (`MAX_DEPTH = 64`, 601, 655): 65 levels.
- **P6-PAR-T12 The cached state is a clone**: `apply_load_spec` adds `index` and prepends this call's prefix tokens on
  the returned clone, never on the stored entry (the doc on `State.tokens`).
- **P6-PAR-T13 `disableForCommands` is a repository rule** (Phase 5 deviation 3, D1): every `find` checks
  `SpecLookup::disables` first, returns `Ok(None)` and caches nothing — the single-token load, every `parse_prefix`
  location (a disabled one falls through), nested `loadSpec`/`isCommand`/`isScript`/`isModule` loads, a Local base
  name. A request-level check cannot reproduce that: `CompletionRequestPolicy` has no such rule (Q23).
- **P6-PAR-T14 generateSpec runs first in every iteration** (694-698), including the one that then breaks for the final
  token, over `cur.tokens[start..]` with the partial final token (`generate_spec_merge` expects
  `["cmd","generateSpec",""]`).
- **P6-PAR-T15 `update` order and laziness** (365-380): subcommand → option_or_chain (if `can_options`) → option_arg
  (only if `prefer_opt_arg`) → subcommand_arg.
- **P6-PAR-T16 Alias** (711-731, `substitute_alias` 783-807): only when the token was consumed as the predicted kind
  and not yet in `substituted` (717); success re-parses the same position; failure returns at once when nothing was
  substituted yet or on Aborted (726), else it is deferred and a nested loadSpec may still win; `substituted.clear()`
  after each consumed token (769); the value is `trim_end()`-ed (806); substitution re-tokenizes with the **request's**
  shell (`JsHost::substitute` → `substitute_token(cmd, index, value, self.ctx.shell)`, 217-219).
- **P6-PAR-T17 loadSpec precedence**: a consumed subcommand's own `load_spec`; for an arg consumed as predicted:
  `arg.load_spec`, else `isCommand`/`isScript` → `spec_location(token, cwd, Some(is_script))`, else `isModule` →
  `Global { prefix + token }`; a function loadSpec at the root is skipped; an empty Locations list errors.
- **P6-PAR-T18 Inline loadSpec** (`apply_load_spec`, 812-862) **replaces** `persistent`, replaces the last path entry,
  resets `opt_args`/`passed`/`sub_args`/`entered_sub_args`, keeps `end_of_options`, replaces `directives` only when
  `has_directives` (the `ponytail:` at 850 stays). Its `_ => Ok(None)` (862) becomes explicit arms in 6.3d (D8).
- **P6-PAR-T19 generateSpec cache key** (878-890): a string key as is; a function key called with ONE argument
  `{"tokens": tokens}` and no exec; Aborted returns; any other error or a non-string gives no key and generation still
  runs; `.filter(|_| !dev)`; the key is `"{tokens[0]}:{k}"`.
- **P6-PAR-T20 generateSpec result** (890-937): a hit reuses the stored `Rc`; a miss makes one new `Rc` per call (908),
  stored only with a key, same wholesale clear at `>= MAX_CACHED` (910); the merge reads `args_were_empty` **before**
  `merged_with_generated`; `persistent = generated ++ inherited`; `set_completion(Rc::new(c))` (936).
- **P6-PAR-T21…T26 The state machine moves verbatim** (`parser.rs:222-587`): `subcommand` (382-406, `fin` only
  annotates, `persistent` appended parent first, `directives` replaced whole), `find_option` (352-361, the parent wins
  a clash) and `option` (408-427, `Times(n)` counts any shared name), `option_or_chain` (450-536, every POSIX-chain and
  separator rule), `can_options` (333-350, asymmetric) and `can_subcommands` (329-331, ignores `end_of_options`),
  `result` (538-587; its `match a.parts.last()` at 547 becomes explicit arms in 6.3b, D8), `ArgState::advance`
  (242-254). `apply_inline_spec`'s `ponytail:` stays (PAR3 API, 6.3c).
- **P6-PAR-T27 `Rc` identity is behaviour.** The arg key hashes `Rc::as_ptr(current_arg)` and
  `Rc::as_ptr(completion)` (`engine.rs:565-568`); what keeps addresses stable: one `first` `Rc` per cache (595) kept by
  `clear_caches` (140), a prefix hit returns a clone holding the same `Rc`s, every `State` clone copies `Rc`s, the
  repository returns the stored `Rc`, a cached generated spec reuses its `Rc`. So accessors return `&Rc<_>` /
  `&Option<Rc<_>>`; nothing deep-clones or re-wraps. Guards: `parse_arguments_table`, `single_token_paths`,
  `load_spec_and_is_module`, `recursive_load_spec`, `generate_spec_merge`, `first_token_spec_identity_is_stable`, the
  `spec_identity_…` split, `generator_keys_differ_per_index`.
- **P6-PAR-T28 Per-parse generator context** (`gen_ctx`, 178-190): `GeneratorContext::new(cwd lossy, env.clone(),
  shell.name(), String::new(), false)`, built once per parse, shared by every find and every parser spec call; not the
  engine's per-request context (D1: two builders).
- **P6-PAR-T29 `dev` is read per parse** (`dev_mode(ctx.settings)`, 147, 174) and separately per find by the
  repository (its own copy, `quickjs_spec_repository.rs:68`). Never cache it; never merge the two `dev_mode`s.
- **P6-PAR-T30 Timeouts**: every parser spec call uses the parser's own 5 s `TIMEOUT` (34; the repository has its
  own, `quickjs_spec_repository.rs:29`), never `SYNC_CALL` or `LOAD_TIMEOUT`.
- **P6-PAR-T31 `clear_caches`** (137-143) = `forget_loaded_specs()` then the reset keeping `first`; it runs on **every**
  Settings message at its position: `apply_settings`, settings, `clear_caches`, `reset_slots`, `first_token_cache =
  None` (`engine.rs:476-482`). It keeps the repository's `by_json`, versions and in-flight probes.
- **P6-PAR-T32 Iteration order**: only `Vec`s are iterated for output; `substituted` is membership only; the caches are
  lookups plus wholesale `clear()`.
- **P6-PAR-T33 Threads and borrows**: everything is `Rc` on the engine thread; `QuickJsSpecRepository::find` holds one
  `loader.borrow_mut()` for the whole find, so the parser never calls `find` from a runtime callback or nests finds;
  `ParsePrefixCache` is an exclusive `&mut`, never a `RefCell`.
- **P6-PAR-T34 No `cfg(windows)`/`cfg(unix)` code** in `parser.rs` or `parser/tests.rs` (only `#[cfg(test)]` at 30).
- **P6-PAR-T35 The Mock's contract**, which the InMemory doubles keep (`parser/tests.rs:86-125`): it records **every**
  load before the lookup, never caches, returns the exact `Rc`, never errors, keys globals by name and locals by
  `"{dir.display()}|{name}"`. Tests assert exact load sequences (`single_token_paths`,
  `relative_path_first_token_loads_local_spec`, `load_spec_and_is_module`, `recursive_load_spec`,
  `is_command_recursion`, `prefix_cache`, `cached_nested_state_keeps_this_commands_prefix`, `exceptions_and_first_token`).
  `InMemorySpecRepository` does all of that and honours `disables`, which no parser test sets.
- **P6-PAR-T36 The Mock's substitution is a whitespace splice** (`parser/tests.rs:121-125`, never `None`). The real
  `substitute_token` gives the same token **texts** for `git_alias_directive`'s buffers (`git co ` + `checkout` →
  `[git, checkout, ""]`; `git cm msg` + `commit -m` → `[git, commit, -m, msg]`, `diff = 1`); the test asserts texts,
  `search_term`, `suggest_options` and the completion `Rc`, never a substituted raw. 6.3d's harness uses the real
  `substitute_token`, so `git_alias_directive` is a guard of that step.

### Generators, cache, recency and session state (6.4) — `engine.rs` unless named
- **P6-GEN-GT1 Decide, then mutate** (`update_slots`, 682-710): `fire: Vec<bool>` is built for every generator before
  anything changes; a function trigger's `Err(SpecError::Aborted)` does `return None` (700) before `prev_search`
  (711), the slots or `next_gen` move.
- **P6-GEN-GT2 Fire rules, in order** (686-708): `!same || i >= slots.len()` fires; else a path template fires on
  `dir_changed(new, old, ctx.shell() == Shell::Pwsh.name())` (689); else `trigger()`: `None => arg.debounce()
  .is_some()`; `Fn(f)` → `call_json(f, [Json(new), Json(old)], None, SYNC_CALL)` (693-697): `Ok(v) => truthy(&v)`,
  `Aborted => return None`, every other error `=> true`; `Some(t) => triggered(t, new, old).unwrap_or(true)` (707). JS
  triggers run in generator order with ctx `None`; a new arg never asks a function trigger.
- **P6-GEN-GT3 `same` is `Rc::ptr_eq`** of `pr.current_arg` and `slot_arg` (678-681); `None` on either side is not same.
- **P6-GEN-GT4 `prev_search` is set after the loop for every non-aborted request**, also with no arg; then no arg
  clears the slots and `slot_arg` and returns `Some(vec![])` (711-716).
- **P6-GEN-GT5 Slots are rebuilt only for a new arg** (717-728): `key: gen_key(arg, i)`, empty, not loading,
  `gen_id: 0`; results are not carried over (ponytail 718).
- **P6-GEN-GT6 Help rows once per update** (729-733): `help_items(pr)` when ANY generator has a `"help"` template,
  cloned into every fired run and appended to every template generator's items (826).
- **P6-GEN-GT7 Two script timeouts (D1).** The engine floor is `settings.f64(AUTOCOMPLETE_SCRIPT_TIMEOUT).max(0.0) as
  u64` (`script_timeout`, 263-265; read at 734 and 930); the runtime's is `settings.i64(..).max(0) as u64`
  (`quickjs_spec_runtime.rs:105-107`). `7500.5` gives 7500 here and 5000 there. Never merge them.
- **P6-GEN-GT8 Per fired generator, in order** (740-770): `next_gen += 1`; `slot.gen_id = next_gen`; `loading =
  true`; push the key; debounce; budget; the run with `Instant::now()` read **per fired generator**; start; `settle`.
- **P6-GEN-GT9 Debounce and budget are plain `u64`** (745-748): `arg.debounce().map(|ms| if ms > 0 { ms } else { 200
  })`; `debounce.unwrap_or(0) + 2 * floor.max(g.script_timeout().unwrap_or(0)) + 1000`; `Instant::now() +
  Duration::from_millis(budget)` (761). A huge `scriptTimeout` overflows (debug panic, release wrap): keep it, no
  `saturating_*` (the clippy budget is 3 and the behaviour is the legacy one). One deadline per run.
- **P6-GEN-GT10 Debounced runs** start a sleep and stay in `Step::Debounce` (`run.task = js.start_sleep(ms);
  Outcome::Pending`, 763-766); undebounced runs call `begin` synchronously (768).
- **P6-GEN-GT11 Template generators never run script or custom** (`begin`, 814-839): the directory is listed only when
  `is_path_template(g) && !run.ctx.cwd().is_empty()` (817); `files` = any `"filepaths"` template (816);
  `inserted_directory(cwd, search_term, env, shell == "pwsh")` then `list_dir(&dir, files, is_dangerous)` (synchronous);
  `items.extend(run.help)`; `filterTemplateSuggestions` gets `template_json(&items)` and waits in `Step::Filter`; else
  `Done(items)`.
- **P6-GEN-GT12 A non-template generator with an empty cwd gives `Done([])`** (840-842), before script and custom.
- **P6-GEN-GT13 Script wins over custom** (843-858): `Fn(f)` starts `[Json(tokens)]` with `Some(&run.ctx)` in
  `Step::Script`; `Argv`/`Shell`/`Object` go to `script_input` with `json!(a)`, `json!(s)`, `spec_value_to_json(o)`;
  neither → `Done([])`.
- **P6-GEN-GT14 Custom** (859-877): key = `g.cache().map(|c| cache_key(c, cwd, c.cache_key().clone()
  .unwrap_or_else(|| run.tokens.join(" "))))`; args `[Json(tokens), Exec, Json(generator_context_json(&run.ctx))]`
  (the production builder; never the runtime test helper, D1); started with `Some(&ctx)` in `Step::Custom`.
- **P6-GEN-GT15 `fetch`** (881-922): no policy or key → start, step, `Err(Pending)`, no store; `Fresh(v) => Ok(v)`;
  `Stale { value, refresh }` with `refresh` → `bg = run.clone()` taken **before** `run.step` changes, `background =
  true`, its own task, `cache.start`, `bg.store = Some(key)`, `self.runs.push(bg)` immediately (never through
  `settle`), then `Ok(value)`; `Shared(t)` → `run.task = t`, step, `Err(Pending)`, a sharer never stores; `Miss` →
  start, step, `cache.start`, store. `start` is called at most once.
- **P6-GEN-GT16 `script_input`** (925-949): `script_exec_input(v, cwd)` (argv `{command, args, cwd}`, a non-empty
  string → `sh -c`, an object gets `cwd` first then its own keys win; `[]`, `""`, `null`, numbers, bools → `Done([])`);
  `key_src = input.to_string()` **before** the timeout is set; `timeout = floor.max(g.script_timeout().unwrap_or(0))
  .max(input.get("timeout").and_then(Value::as_u64).unwrap_or(0))` (a float or negative object timeout counts as 0);
  `from_value::<ExecInput>` failure → `Fail(format!("bad script input: {e}"))` (no ` at line L column C`); key =
  `cacheKey` override else `key_src`; fetch in `Step::Exec`.
- **P6-GEN-GT17 `exec_output`** (952-975): `stdout` read from whatever step stored the value; `splitOn` wins over
  `postProcess`; the WHOLE stdout trimmed, empty → `[]`; pieces NOT trimmed, empty pieces stay rows (`"a\n\nb"` → 3);
  `postProcess` gets `[Json(stdout), Json(tokens)]`; exit status ignored.
- **P6-GEN-GT18 `custom_result`** (978-991): templated = the FIRST element has a `"context"` key, even `null`;
  `filterTemplateSuggestions` gets the RAW value; else `json_items(&v)`.
- **P6-GEN-GT19 `advance`** (994-1020): `Err(e)` → `cache.fail(k)` when `store.take()` is `Some`, then `Fail(e)`;
  `Ok(v)` → `cache.store(&k, v.clone(), now_ms())` (1005) **before** the superseded check, so superseded runs still fill
  the cache; superseded → `Done([])`; then by step (`Debounce → begin`, `Script → script_input`, `Exec → exec_output`,
  `Custom → custom_result`, `PostProcess → post_process_items(&v, is_dangerous)`, `Filter → json_items`).
- **P6-GEN-GT20 Two JSON→row rules, never merged** (`fig_spec_mapper.rs`): `json_items` keeps string rows with kind
  `None`, `""` included; `post_process_items` makes `from_script_output` rows, drops `""`, needs a truthy `name`.
- **P6-GEN-GT21 A malformed result is `[]`, never a task error** (only GT16's serde text is a `Fail`).
- **P6-GEN-GT22 The cached value is the raw pre-step value** (an exec's `{status, stderr, stdout}` or a custom's
  return); a custom and a script generator with equal `cacheKey` read each other's raw values through their own reader.
- **P6-GEN-GT23 `settle`** (785-808): `Pending` → push, `false`; `Fail(e)` → `logger::debug("engine: generator failed:
  {e}")` (793) for EVERY failure (superseded and background included), then a background run returns `false`;
  `Done`/foreground `Fail` (`[]`, never cached) updates the slot only when `gen_id` matches, `loading = false`; returns
  whether a current slot changed.
- **P6-GEN-GT24 `pump` order** (1023-1074): tasks `sort_unstable` + `dedup` (ascending ids); deadline
  `.map_or(Instant::now() + SLICE, |d| d.min(Instant::now() + SLICE))` (1032, two eager clock reads); `Err(Aborted) =>
  return` keeps every run (1035); impossible other errors → `vec![Some(Err(e.to_string())); n]` (1036-1038); `runs =
  mem::take(&mut self.runs)`, so runs pushed during the pump land **before** `extend(alive)` (1059); per finished task:
  owners (`store.is_some()`) before sharers, `advance` then `settle`, then `forget(&[task])` (1054); `let now =
  Instant::now()` **after** the processing (1056); each expired run: `cache.fail(k)` if it stores, `forget` only if no
  live run shares its task (1064-1065), `settle(run, Fail("timed out"))`; reply only `if changed && self.cur.is_some()`
  (1069-1072).
- **P6-GEN-GT25 Abort is not failure**: an aborted wait returns with runs and cache untouched.
- **P6-GEN-GT26 `GenCache`** (`suggest.rs:1094-1177`): TTL inclusive `now.saturating_sub(*at) <= ttl` → `Fresh`; past
  TTL with `max_age` → `Shared(inflight)` or `Miss`, never `Stale`; else `Stale { refresh: inflight.is_none() }`; a
  value-less entry in flight → `Shared`; `start` keeps the old value; `store` overwrites the whole entry and clears
  in-flight; `fail` removes only value-less entries. Wall-clock ms (`now_ms`), never `Instant`.
- **P6-GEN-GT27 `cache_key` always has a comma** (`suggest.rs:1090-1092`): `",git branch"` without
  `cacheByDirectory`.
- **P6-GEN-GT28 The generator cache is never bounded or cleared** — not by Settings (476-482), CommandFinished/Ended
  (459-468) or Session.
- **P6-GEN-GT29 `GeneratorKey` = `DefaultHasher` over `(Rc::as_ptr(arg) as usize, idx)`** (`hash2`/`gen_key`,
  230-240); the arg key is the same hash over `(arg ptr or 0, Rc::as_ptr(&pr.completion) as usize)` (565-568).
  `slot_arg`, every run's `arg` and the served request hold the `Rc`s that keep addresses from being reused.
- **P6-GEN-GT30 `triggered`/`dir_changed`** (`suggest.rs:1069-1087`): JS `lastIndexOf` in CHARS; `Threshold(n)`: `new
  > n && old <= n` in chars; `Match` compares positions; `Fn` → `None` (→ `true`, 707); `dir_changed` cuts at
  `after_sep`.
- **P6-GEN-GT31 `record`** (1207-1236): `if name.contains('↪') { return; }` first (it duplicates `menu.rs:367`, D1:
  keep both); insert `now_ms()` even without a state path (memory only); `if self.saving.swap(true, SeqCst) {
  return; }` (1217); else thread `recency` (1222-1223): `sleep(1 s)`, `saving.store(false)`, THEN snapshot the LATEST
  map, save, `logger::warn("engine: saving recency: {e:#}")` (1229); spawn failure → `saving.store(false)` + a
  synchronous save (1243). Never snapshot at record time; never a trailing debounce.
- **P6-GEN-GT32 `save_recency`** (206-228): purge guard `if !path.parent().is_some_and(Path::is_dir) { return Ok(())
  }`; a corrupt or non-object `state.json` → `Err`, nothing written; a stored recency of the wrong shape → treated as
  empty and REPLACED; merge `or_insert(0)` then `max` per (cmd, name); other keys kept; `JsonDocumentStore::save`
  (pretty, trailing newline, atomic).
- **P6-GEN-GT33 `load_recency`** (195-201): missing, corrupt, non-object or wrong-shape → empty; loaded once in
  `Engine::with` (391), after the history module.
- **P6-GEN-GT34 The shutdown flush is conditional** (450-452): `if self.saving.load(SeqCst) { self.save_recency_now()
  }`. An unconditional flush writes `{"autocomplete.recency":{}}` on every exit. Nobody joins the engine thread.
- **P6-GEN-GT35 `save_recency_now`** (1238-1245) writes under the recency lock; no state path → nothing.
- **P6-GEN-GT36 Ranking reads recency** (`reply`, 1137-1146): `use_recency = sortMethod != Some("alphabetical")`; the
  map of `pr.tokens.get(pr.command_index)` (or `""`); a poisoned lock → no recency.
- **P6-GEN-GT37 Log text** `engine: saving recency: {e:#}` (warn) is anyhow's chain; the adapter renders it through
  `crate::shared::infrastructure::error_chain::display_chain` (same text); no third chain renderer.
- **P6-GEN-GT38 Env**: an EMPTY session env means "use the wrapper env" (`env`, 502-508; inline again in `compute`,
  544-548); the wrapper env is one `std::env::vars().collect()` (403) on the engine thread (it panics on a non-UTF-8
  variable; keep the call).
- **P6-GEN-GT39 Alias cache** (`alias_map`, 510-522): per `Shell`, rebuilt when the shell differs; non-empty text →
  `parse_aliases(text, shell)`, `None`/`""` → empty; returns a CLONE; cleared only by Session (486); filled (540)
  before a parse that may abort.
- **P6-GEN-GT40 Partial mutation before an abort**: the alias cache (540) and `user_fuzzy`/`last_arg_key` (569-572)
  change even when `queries` or `update_slots` then abort; the served request stays the older one.
- **P6-GEN-GT41 Fuzzy** (569-580): `if arg_key != self.last_arg_key { self.user_fuzzy = settings.bool(FUZZY); .. }`;
  strategy `Prefix`/`Fuzzy` override it; Settings does not reset it; ToggleFuzzy flips it with no reply (489).
  `user_fuzzy` is read from the settings **before** they move into the struct (`Engine::with`, 395).
- **P6-GEN-GT42 First-token cache** (1079-1098): `at.elapsed() < FIRST_TOKEN_TTL` (1086, strict); `Instant::now()` read
  again to store (1096); cleared ONLY by Settings (481) and Session (487).
- **P6-GEN-GT43 Message effects** (459-489): CommandFinished/Ended = record (Finished only) → ghost cache clear →
  `reset_slots()` (slots, `slot_arg`, `prev_search`) → `cur = None`; runs and cache stay (in-flight runs settle into
  missing slots). Settings = `apply_settings` → `settings = s` → `parser.clear_caches()` → `reset_slots()` →
  `first_token_cache = None`; it does NOT touch `cur`, runs, cache, aliases, `user_fuzzy`, recency. Session = env,
  alias, `aliases = None`, `first_token_cache = None`; slots stay.
- **P6-GEN-GT44 Startup order** (see "What Phase 5 leaves"): inbox → runtime (failure: `engine: js host: {e:#}`,
  nothing else built, no bundle install) → dependencies → repository → parser → bundle install + index (`engine: specs:
  {e:#}` + `SpecIndex::default()`) → history module → state path → recency load → wrapper env snapshot. Everything
  holding an `Rc<dyn ..>` is built inside the thread closure.
- **P6-GEN typed equivalences the 6.4d switch relies on** (Phase 5 P5, P11, P18; recheck R8):
  - `Value → SpecValue → Value` is the identity for parsed JSON (`spec_value_to_json` expects a finite float, which
    parsed JSON always is), so a cached `SpecValue` re-encoded for `TemplateSuggestions::Returned` or a decoder, and the
    `cache_key_source` text, are byte-identical.
  - `json!(a)` for `a: &Vec<String>` == `spec_value_to_json(&SpecValue::List(a.iter().cloned().map(SpecValue::Text)
    .collect()))`; `json!(s)` == `spec_value_to_json(&SpecValue::Text(s.clone()))`.
  - `function_result(QueryTerm)` = a string else `None`; `Trigger` = `truthy`; `Custom` = `{templated, json_items}`;
    `PostProcess` = `post_process_items(v, ctx.is_dangerous())`; `FilterTemplateSuggestions` = `json_items`; `Script` =
    the private `script_resolution` (`fig_spec_mapper.rs:525-539`): `script_exec_input` → `Skip`, then
    `cache_key_source = input.to_string()`, then `input["timeout"] = json!(requested.unwrap_or(0))`, then serde →
    `Invalid(text)` or `Ready { request, cache_key_source }`.
  - Typed `wait` = `wait_json` with `Err(Aborted) → Err(Interrupted)` and every value through `spec_value_from_json`;
    typed `call` forgets on Timeout and Aborted, not on a failed call.
  - **P6-M4** `template_json` emits `null` for a missing name/insertValue/templateType and sorts keys; after 6.4d the
    `Listed` path is encoded in the specs adapter and must keep that.

### Engine loop, use cases and server (6.0a, 6.5, 6.6) — `engine.rs` unless named
- **P6-ENG-E1 Drain-then-serve** (`run`, 421-449): block only when nothing is pending and no run is alive, then drain
  everything (`try_next_msg`), then serve the last pending request and `continue`; pump only when no request is
  pending. A Settings/Session/CommandFinished/ToggleFuzzy queued after a Complete is applied before it is computed.
- **P6-ENG-E2 Only the latest request is served**: `Complete(r) => *pending = Some(r)` (458) drops the older one with
  no reply.
- **P6-ENG-E3 Shutdown** (491): `handle` returns `false`; blocking branch `break`, drain `break 'outer` (437)
  **before** the pending request is computed, so a Complete drained with Shutdown gets no reply. After the loop the
  conditional recency flush (450-452, GT34).
- **P6-ENG-E4 Abort returns without a reply**: `compute` returns `None` on `ParseError::Aborted` (558), a getQueryTerm
  abort (655) and a trigger abort (700); `run` continues (444); the newer Complete, kept in the inbox's `deferred`, is
  popped first (`next_msg` 132, `try_next_msg` 145). `pump` returns on `Err(Aborted)` keeping every run (1035).
- **P6-ENG-E5 The abort check reads only kept messages**: `has_newer_request` = `deferred.iter().any(Complete)`
  (174-179); `wait_json` checks it before every pump (`quickjs_spec_runtime.rs:329`); `poll` receives at most one
  message and pushes a non-runtime one to the **back** of `deferred` (181-191). A Complete still in the channel aborts
  nothing. Never two channels; never an inbox that parks only Complete.
- **P6-ENG-E6 The inbox never disconnects**: it holds `sender` (115) and every `sink()` clones it (167); `next_msg`'s
  `None` arm (431) is unreachable; only Shutdown ends the loop. `spawn`'s doc (81) wrongly says "or when all senders
  are dropped" (**P6-M5**; fixed by 6.6b commit 3).
- **P6-ENG-E7 Runtime events are consumed by the inbox**: `deliver_runtime_event` (156-162) hands
  `EngineMsg::Runtime` to `runtime.handle_event` at once (158); `handle`'s `Runtime(_) => {}` (490) is unreachable. The
  inbox needs the **concrete** runtime (`handle_event` is not on `SpecRuntime`).
- **P6-ENG-E8 `RefCell` scoping**: `deferred.borrow_mut()` is a temporary of the `if let` (132, 145), released before
  `deliver_runtime_event`; never hold it across `handle_event`; no outer `RefCell` around the runtime.
- **P6-ENG-E9 Partial mutation before an abort**: see GT40; `prev_search` is **not** updated on a trigger abort (700
  returns before 711).
- **P6-ENG-E10 Settings does not touch** `user_fuzzy`, the served request, runs, the generator cache, the alias cache or
  the ghost cache; `clear_caches` keeps spec identities, so `arg_key` survives a settings reload.
- **P6-ENG-E11 Session does not reset the slots** and clears the first-token cache; CommandFinished does **not** clear
  the first-token cache.
- **P6-ENG-E12 CommandFinished/Ended leave runs and the generator cache alone.**
- **P6-ENG-E13 ToggleFuzzy** sends no reply and lasts until the argument changes (569-572); `FilterStrategy::Prefix`/
  `Fuzzy` override it (576-580).
- **P6-ENG-E14 Order inside `compute`** (528-620): clamp `cursor = req.cursor.min(req.buffer.len())` (529) → mid-word
  guard (531-538) → `alias_map(req.shell)` (540) → `current_command` (541) → env (544-548) → `ParseCtx` + parse
  (549-563) → arg key + fuzzy (564-580) → `GeneratorContext::new(cwd lossy, env.clone(), shell name, search term,
  is_dangerous)` (581-587) → `PATH`/`PATHEXT` via `env_var` (588) → `queries` (589, getQueryTerm fns first) → tokens
  from `command_index` (590-594) → `update_slots` (595, trigger fns) → first-token items only `if pr.first_token &&
  settings.bool(firstTokenCompletion)` (596-602) → history prefix (603-606) → `cur = Some(Current { .. })` (607-618)
  → `reply(Parse)`. `CompleteCommandLineUseCase` keeps this order exactly.
- **P6-ENG-E15 Error replies** (`error`, 622-631): `reset_slots(); cur = None;` then `Completion::error(req.id,
  req.buffer.clone(), cursor)` and a ghost only when the flag is set. Mid-word → `false` (537); no current command →
  `true` (542); NoSuggestions → `logger::debug("engine: no suggestions: {e}")` (560) then `true` (561). `phase: Parse`
  is load-bearing (CON-T1).
- **P6-ENG-E16 The ghost flag is redundant (proof)**: the guard fires only when `cursor < len`; `ghost()` returns
  `None` as soon as `cursor != req.buffer.len()` (1189-1193), before reading anything; error replies compute the ghost
  with `items = None`, which equals `Some(&[])` (`spec_ghost` is a `find_map`, `inline.rs:144-155`). So the flag can go
  (Q15).
- **P6-ENG-E17 Ghost on every reply**: `reply` computes `ghost(&cur.req, cur.cursor, Some(&items))` (1159) for **both**
  phases; pty keeps the old ghost when a Generators reply has `ghost: None` (`pty.rs:881-883`), so a Generators reply
  without a recomputed ghost leaves a stale or missing spec ghost. The plan's event-table letter ("only after
  Complete") would break this (Q15).
- **P6-ENG-E18 Ghost rules** (`ghost`, 1188-1202): `inline.enabled`, non-empty buffer, `cursor == buffer.len()`;
  `Ok(Specs)` → spec only; `Ok(History) | Ok(Profile(_))` → history only; `HistoryAndSpecs` **and `Err(_)`** →
  `history().or_else(specs)`.
- **P6-ENG-E19 Reply contents** (1160-1184): id, phase, `error: false`, request buffer, clamped cursor, `tokens`,
  `command_text = cmd.original_text`, `command_index`, `arg_key`, `current_arg = ArgInfo { name or "", description or
  "" }` (1171), `search_term`, `triggered_generators` **only for Parse** (1175-1179), items, `loading:
  slots.iter().any(loading)` (1181), ghost, fuzzy. A Generators reply rebuilds everything from the served request with
  the current slots, settings, history and recency.
- **P6-ENG-E20 Reply order of work** (1104-1158): first-token list or `assemble(pr, views)` (views =
  `slots.zip(&queries)`, which truncates) → history rows when `history_mode || mode == "show" || "history_only"`
  (1120-1124) → `FilterOpts` from five settings (1125-1136) → recency key `pr.tokens[command_index]` (1137) →
  `use_recency` (1138) → rank (1139-1146) → `InsCtx` (1147-1154) → `to_item` (1155-1158). **P6-M10:**
  `insertSpaceAutomatically` default `true`, `beta.history.mode` default `"off"`, a non-string `sortMethod` ranks by
  recency, a wrong-type getter gives the default — the builder copies every getter verbatim.
- **P6-ENG-E21 Identity**: same hasher and tuple type for the arg key and the generator key (GT29); `Rc::ptr_eq`
  decides "same argument" (678-681).
- **P6-ENG-E22 First-token cache timing**: one clock read for the TTL check before the scan (1086); the stamp is a
  **second** `Instant::now()` **after** the PATH scan (1096); the scan runs only on a miss.
- **P6-ENG-E23 Env snapshot timing**: `std::env::vars().collect()` (403) runs after the bundle install, the index
  load, the history load and the recency load; `debug shell` sets `ZDOTDIR`/`TABSMITH_DEBUG_SHELL` before the engine
  starts (`cli.rs:582-586`), so the snapshot includes them. Q26 keeps the snapshot after `completion.recency.load()`.
- **P6-ENG-E24 `pump`** as the driver sees it: GT24.
- **P6-ENG-E25 `settle` logs** `engine: generator failed: {e}` at debug for every failure before deciding (792-798); it
  is called inside `update_slots` (770) and `pump` (1052, 1067), interleaved with runtime calls that log `js
  console.{level}: ..`/`js settle ..`. The notice must be reported at that same moment (Q11).
- **P6-ENG-E26 Recency**: GT31-GT35.
- **P6-ENG-E27 `llm`** (1247-1275): a non-profile source → `Ghost { req, None }` at once on the engine thread (1250); a
  cache hit → `Ghost { req, Some }` at once (1253-1257); else snapshot `llm_history(settings, history.recent())` and
  `env().clone()` at request time, spawn thread `llm` (1263-1264) that calls `llm_complete`, inserts
  `format!("{buffer}{t}")` into the shared `Arc<Mutex<GhostCache>>` and sends `Ghost` (1270); spawn failure →
  `logger::warn("engine: llm worker: {e}")` (1273) and **no** reply. Moves verbatim into the legacy ghost block.
- **P6-ENG-E28 Spawn** (80-111): thread `engine`; spawn failure → `engine: spawn: {e}` (106) and the sender is still
  returned. Inside: E28's order is GT44's.
- **P6-ENG-E29 `!Send`**: every `Rc` (runtime, repository, history, use cases, session) is built inside the closure.
  Threads: `engine`, `recency`, `llm`, `tabsmith-exec`. `Msg::Completion(Box<..>)` and the engine message cross threads
  (CON-T17).
- **P6-ENG-E30 Spawn point**: CON-T11. Moving the spawn into bootstrap before `pty::run` changes files written, log
  lines and the debug-shell env snapshot; so the pty receives a starter (Q12).
- **P6-ENG-E31 Every send ignores errors** (CON-T14): port methods are infallible; the channel adapter drops
  `SendError`.
- **P6-ENG-E32 Send order** (CON-T15); `Llm` evaluates `self.req`, buffer, `self.cwd()` (I/O fallback to
  `current_dir`), `self.shell()` in that order (`pty.rs:1240-1245`).
- **P6-ENG-E33 Replies share the pty inbox** (CON-T10).
- **P6-ENG-E34 `cleanup` order** (`pty.rs:1304-1314`; CON-T15).

**Log lines (D5: same text, level, condition, place).**

| Text | Level | Where at `ff2db5a` | After Phase 6 |
|---|---|---|---|
| `engine: js host: {e:#}` | error | `spawn`, `engine.rs:93` | `engine_server::spawn`, verbatim |
| `engine: spawn: {e}` | error | `spawn`, 106 | same |
| `engine: specs: {e:#}` | error | `Engine::new`, 367 | `spawn`'s index build (Q26 step 5), verbatim |
| `engine: no suggestions: {e}` | debug | `compute`, 560 | `CompletionNotice::NoSuggestions` from `CompleteCommandLineUseCase`, logged by the server at the same moment |
| `engine: generator failed: {e}` | debug | `settle`, 793 | `CompletionNotice::GeneratorFailed` from `GeneratorRunner`, right before each `session.settle(run, Fail(e))` |
| `generateSpec ({}): {e:?}` | warn | `parser.rs:919`, mid-parse | `CompletionNotice::GenerateSpecFailed(line)` built in the parser with its comment, logged by the caller mid-parse |
| `engine: saving recency: {e:#}` | warn | `record`, 1229, 1243 | `StateFileRecencyRepository`, via `display_chain` |
| `engine: llm worker: {e}` | warn | `llm`, 1273 | the legacy ghost block of `engine_server.rs` |
| `history: {e:#}` | warn | `build_history_module` | unchanged |

### Phase 7 traps Phase 6 must not disturb
- **P6-CON-T21…T26** (inline CLI, Phase 7): the strict settings load and raw `SettingsError` chain of the inline
  commands (golden `golden.txt:6157-6160, 6250-6253, 8889-8892`), the inline texts, the `InlineCmd` clap identifiers
  (D4), `cli.rs` imports the inline move leaves unused (clippy budget), doctor's inline check, pty's `llm_enabled`
  (`pty.rs:902-908`). Phase 6 touches none of these files' inline paths (Q30).
- **P6-M1, P6-M2, P6-M6…P6-M9** are Phase 7's (pty `ghost_text` guards `pty.rs:912-929`; the `Msg::Ghost` arm
  682-690; the prompt numbering and the 5000 cap; no `temperature`/`top_p`/`top_k`; exact profile texts; `inline
  status` reads a non-bool as enabled). Phase 6 leaves `ghost_keys_and_words`,
  `ghost_with_control_chars_is_never_shown_or_accepted` and `inline_show_customizations` untouched.

### Cross-cutting
- **Fitness facts the phase relies on.**
  - **P6-CON-T27** `LEGACY_MODULES` keeps `types`, `engine`, `parser`, `shell_parser`, `suggest`, `inline` listed
    after their files go: `should_flag_a_domain_or_application_file_that_uses_a_legacy_module` feeds `use
    crate::types::SessionCtx;` and `crate::engine::spawn(..)`. 6.7 never "cleans up" the list.
  - **P6-CON-T28** A test-only **file** is read as production code: `production = !test_only_item.holds(..) &&
    !test_file`, `test_file = path.ends_with("/tests.rs")`; an inner `#![cfg(test)]` is not recognised; items are
    exempt only under `#[cfg(test)]`/`#[cfg(all(test, ..))]`. So `domain/test_fixtures.rs` and
    `application/test_fixtures.rs` import only domain/application/port types (no `serde_json`, no
    `settings::infrastructure`), or carry `#[cfg(test)]` on each item.
  - **P6-CON-T29** A domain file may name no `application` path, not even its own module's (`breaks_domain_layer`,
    `architecture_tests.rs:320-324`): a completion domain type cannot hold `CompletionRequest`, `RuntimeEvent`,
    `SpecRuntime` or `Interrupted`. Hence `ServedRequest` (Q5) and the `GeneratorCommand` payload types, which all live
    in `specs/domain`.
  - **P6-CON-T30** std is allowed in domain/application by the checker; the `DirectoryLister`/`ExecutableCatalog`
    ports and the thread placement are enforced by review only.
  - Rule 3: completion may not import `fig_spec_mapper` or any other module's infrastructure; it reads results through
    the typed port only. Rule 6: every `mod.rs` holds only `mod x;`/`pub(crate) mod x;` lines, alphabetical. Rule 7:
    `main.rs` only gains/loses `mod` lines. Rule 8: `include_str!` passes.
  - Rule 3's accepted ceilings (plan section 9, Phase 5 notes): another module's `*UseCase`/`*_use_case` is flagged on
    any path, but a re-export under another name (a `type` alias, a wrapper fn), an `as` rename, a `super::` path and
    an adapter re-exported by a legacy file or module root are not. So no `pub(crate) use` shim is added anywhere
    (6.3d's `Parser` facade is a struct, not a re-export) and every step greps for one by hand. Infrastructure may
    still import a legacy module (rule 3 never flags it); domain and application may not (rules 1-2 with
    `LEGACY_MODULES`).
  - `spec_value_to_json` `.expect`s a finite float: after 6.4d, build `SpecValue::Float`s only from parsed JSON, never
    from arithmetic, or a NaN/inf panics the engine thread.
- **Local fitness check, every step.** `completion` is registered only at 6.7. Before each commit (once
  `src/completion/` exists, 6.1a on; `rust_files` panics on a missing folder), add `"completion"` to
  `REGISTERED_MODULES` in a copy of the exact tree being committed (`git archive "$(git write-tree)"` after staging, or
  `git archive HEAD` after the commit; never `git stash create`, never the working tree) (from 6.6a also
  `"terminal_session"`, for the new port file — accept only the lines the step brief lists as legacy `terminal_session`
  findings, if any), run `cargo test --locked architecture` there, then delete the copy. The list already holds `"specs"`. Every completion line is a defect of
  that step.
- **Comments that name deleted files** (recheck, the `ff2db5a` precedent): `architecture_tests.rs:371`
  (`crate::parser::InstallSpecBundleUseCase`) and `:374-375` (`crate::engine::X`, `src/engine.rs`) as rule 3's
  legacy-shim examples, and `settings_catalog.rs`'s default tests naming `engine.rs` (315, 335, 355, 365, 402, 415, 428,
  441, 454, 467, 480, 493, 565, 575) and `parser.rs` (388) as readers. Owners: 6.3e repoints 371 (to
  `crate::types::InstallSpecBundleUseCase`) and 388 (to `command_line_parser.rs`); 6.7 repoints 374-375 and the
  `engine.rs` reader comments (6.6b leaves them). Comment-only, no assertion change. `parser.rs:1-2`'s module doc ("OWNER: parser
  agent…") goes with the file.
- **Test lock per phase.** The orchestrator tags `phase6-start` (local, lightweight) at the docs commit that hands the
  phase over (its `src/` tree equals `ff2db5a`). Every step runs
  `python3 scripts/hexagonal-refactor/testlock.py . ef807c1 --diff` (MISSING 0, DUPLICATE 0) **and**
  `python3 scripts/hexagonal-refactor/testlock.py . phase6-start --diff` (MISSING 0, DUPLICATE 0, every CHANGED test in
  the step's list or an earlier step's). testlock keys tests by fn name: a new name that already exists hides one of
  the two (`grep -rn "fn <name>(" src` must be empty before adding a name, Q22).
- **`all_bundle_modules`**: `cargo test --locked --release -- --ignored all_bundle_modules` must print `1 passed`
  after 6.3e, 6.4c, 6.4d, 6.4f, 6.5c, 6.6b and 6.7 (its path,
  `specs::infrastructure::adapters::quickjs_spec_runtime::tests::all_bundle_modules`, does not move).
- **Clippy budget 3, all used** (one of the three is `default_constructed_unit_structs`): prune unused imports as
  `types.rs`, `engine.rs`, `suggest.rs`, `parser.rs` shrink (`types.rs` loses `HashMap`, `PathBuf`, `Settings`,
  `HistoryEntry`, `RuntimeEvent` as items leave); never `X::default()` on a unit struct; no fn with more than 7
  parameters (`too_many_arguments`: hence the `…Parts` structs); `large_enum_variant` compares the two largest
  variants.
- **cfg code never compiled here**: `exe_name`'s `cfg(not(unix))` twin, `env_var`'s `cfg!(windows)`, the unix-only
  `PermissionsExt` import inside `first_token_sources` (an import it needs outside the fn carries `#[cfg(unix)]`),
  `list_dir_skips_control_char_names` (`cfg(unix)`), `#[cfg(windows)] conpty_is_closed_off_the_main_loop`
  (`pty.rs:2504-2533`, built through `session()`: after 6.6a check by hand that `session()` and the
  `RecordingCompletionEngine` import are not under a unix-only cfg), `debug_shell_survives_closed_terminal`
  (`cfg(unix)`, runs here). Grep every touched file:
  `grep -n 'cfg(windows)\|cfg(unix)\|cfg(not(\|cfg(target_os\|cfg(all(' <files>`.
- **No golden covers the engine.** The 131 golden commands never run it. The guards are the unit tests, 6.0a's
  engine-loop suite, `debug_shell_survives_closed_terminal` and the plan §8 manual smoke (items 1-2 after 6.4d, all
  items after 6.7, asked of the user by the orchestrator). `integrations-diff.sh` and `precmd-diff.sh` cover nothing
  Phase 6 touches except `state.json` (6.4b); they run at 6.4b and 6.7 as insurance.
- **A flaky gate test.** `integrations_zsh_powerlevel10k_prompt_wrapped` (shell_integration's `embedded_shell_scripts`
  tests) is pty-timing flaky under parallel load. When it is the gate's only failure, rerun it alone with
  `cargo test --locked shell_integration::infrastructure::adapters::embedded_shell_scripts::tests::integrations_zsh_powerlevel10k_prompt_wrapped -- --exact`
  (`1 passed`), then run the full gate again. Never edit, ignore or skip it.
- **History is append-only.** Never amend, rebase, reset, `filter-branch` or force-push. Fix forward. Subjects ≤ 72
  characters, written with `git commit -F` (the RTK hook rewrites words inside `-m`).

## Decisions

The judge's ids and content, with the recheck's corrections folded in. "Rejected" names what a reviewer must not
demand.

### Q1 Phase start and run order
- The orchestrator tags `phase6-start` (local, lightweight) at the docs commit `docs: hand Phase 6 over with its
  research` (recheck: the plan says Phase 5 was "done at `d7079b9`", but `75b9334` and `ff2db5a` came after it and
  `ff2db5a` edits comments in `parser.rs`; the docs commit sits on `ff2db5a` and changes no `src/` file, so every
  `git show phase6-start:<file>` block diff matches the tree the first step starts from). 6.0a's step 0 checks that
  the tag exists and that `git diff --quiet phase6-start HEAD -- src` holds, and stops otherwise.
- The first Phase 6 commit is **6.0a**, a test-only characterization of the engine loop. **6.1a** then creates
  `src/completion/` and `mod completion;`.
- Runs, in order: 1 guards, tokenizer, wire rows; 2 the whole parser (6.3a-6.3e; `parser.rs` gone at its end); 3 the
  suggestion pipeline and the pty port; 4 generator vocabulary, recency, typed port; 5 session, runner, five small use
  cases, request pipeline; 6 lifecycle, `GeneratorCommand`, server, close.
- The parser step owns `AnnKind`/`Annotation`/`ParseResult`/`ParseError` (6.3a verbatim, 6.3b private).
- Facts: no golden run reaches the engine; `suggest.rs:5, 202, 268, 1327` read `ParseResult`/`AnnKind` and rule 1
  forbids a domain file naming `crate::parser`, so the parse vocabulary moves before the pipeline services; 6.3c-6.3e
  touch no `suggest.rs` code; testlock needs the tag before the first commit.
- Rejected: moving types before any guard; characterization only at 6.4a; the pipeline before the parser; splitting
  the parser over two runs.

### Q2 The wire rows: one move step, one privacy pass, both in run 1
- **6.2a** moves seven `types.rs` rows into completion, one commit per type, fields `pub(crate)`, derives kept
  (`Insertion`: `Clone, Debug, Default, PartialEq, Eq`; `Suggestion`: `Clone, Debug`), every importer switched in the
  same commit: `Insertion`; `Item` → `Suggestion` (`title`, `first_name` verbatim); `GenKey` → `GeneratorKey` (Q3);
  `ArgInfo` → `ArgumentHint`; `Phase` → `CompletionPhase`; `Completion` → `CompletionResult` with `error()` verbatim;
  `Request` → `CompletionRequest` (Q4).
- **6.2b** privatises them in one pass: `InsertionParts`, `SuggestionParts`, `CompletionResultParts` (named-field
  construction structs, `Default` where a test literal uses `..Default::default()`; the `HistoryEntryPrimitives`
  precedent, named "Parts" because the fields hold VOs); accessors; production `Insertion::with_execute` (applied on
  the clone under `menu.rs:361-364`'s condition, CON-T5); `CompletionResult::take_suggestions` (`menu.rs:108`'s
  `mem::take`, CON-T2) and `with_ghost`; `ArgumentHint::new` + accessors; `#[cfg(test)]` `with_*` only for test
  writes (D7; find unused ones with a temporary `#![warn(dead_code)]`). No positional constructor takes two `usize`s or
  two `bool`s.
- TC4 (`full_insertion_reopens_only_for_new_arg_or_retrigger`): 6.2a `Item {` → `Suggestion {`, `Some(7)`/`vec![7]` →
  `GeneratorKey::from_raw(7)`; 6.2b `Insertion::new(InsertionParts { .. })`, `item(..).with_generator(..)`,
  `c = c.with_triggered_generators(..)`. TC5: the six result-writing tests get their `#[cfg(test)]` `with_*` in 6.2b.
- Facts: clippy's budget is full and `too_many_arguments` fires above 7; a swapped positional `usize` compiles;
  testlock compares final bodies, so the CHANGED set does not depend on when privacy happens.
- Rejected: privacy at the move (mixes path and accessor edits); a late privacy pass (the builder, the use cases and
  their tests would be written against `pub(crate)` fields, then rewritten); a positional `Insertion::new`; mixed
  `Primitives`/`Parts` naming.

### Q3 `GeneratorKey` is a newtype; one hasher
- `domain/value_objects/generator_key.rs`: `GeneratorKey(u64)` (`Clone, Copy, Debug, PartialEq, Eq`);
  `for_arg(&Rc<Arg>, usize) -> GeneratorKey`; `#[cfg(test)] from_raw(u64)`; the free fn `argument_key(Option<&Rc<Arg>>,
  &Rc<Subcommand>) -> u64`; private `identity_hash(usize, usize) -> u64` = `hash2` verbatim.
- 6.2a deletes `engine::{hash2, gen_key}`; `compute` calls `argument_key`, `update_slots` calls `for_arg`.
  `CompletionRequestPolicy` has no hasher.
- TC3: `generator_keys_differ_per_index` moves to `generator_key.rs` in 6.2a, CHANGED `gen_key` → `for_arg`.
- Facts: plan §1 and §7 name the VO and `for_arg`; the menu only compares keys (`menu.rs:136-138, 373`); raw keys
  appear only at `menu.rs:618, 626`, in a test Q2 already changes; same `DefaultHasher` over `(usize, usize)`, so equal
  values (ENG-E21). The newtype adds 0 CHANGED tests.
- Rejected: a type alias (a recorded deviation that saves no test edit); a second hasher in the policy.

### Q4 `CompletionRequest` is an application DTO
- `completion/application/completion_request.rs` (6.2a): `pub(crate)` fields `id`, `buffer`, `cursor`, `cwd:
  PathBuf`, `shell: Shell`, `history_mode`; `Clone, Debug`. Glossary edit: "shell", "cwd (path)".
- TC6 (`wait_aborts_and_defers_on_complete`, `engine.rs:1500-1554`, its literal at 1502-1509): `Request {` →
  `CompletionRequest {` in 6.2a (CHANGED); path-only into `bootstrap/engine_inbox.rs` in 6.6b.
- Facts: DTOs are port contracts (`CompletionEngine::complete`); rule 1 rejects a domain file naming `application`;
  CON-T9 (lossy cwd); it stays `Send`.
- Rejected: a domain VO.

### Q5 `ServedRequest` replaces `Current`
- `domain/value_objects/served_request.rs`: `ServedRequest` + `ServedRequestParts` with request id, buffer, shell,
  history mode, clamped cursor, parsed command line, command text, argument key, fuzzy, queries, triggered keys,
  first-token candidates, history prefix. Accessors return the stored `Rc`s. Built once per request by the request
  pipeline; held as `Option<ServedRequest>` by `CompletionSession` (6.4e). `cwd` is not stored. Glossary row added.
- Facts: `Current` (`engine.rs:317-329`) is built once (607-618) and never mutated; `reply` reads every field plus
  `req.{id, buffer, shell, history_mode}`; `ghost` reads `req.buffer`; `req.cwd` is read only inside `compute` (550,
  582).
- Rejected: loose session fields; storing the DTO (the domain would name an application path).

### Q6 `ParsePrefixCache` is passed `&mut`; one `clear_caches`
- `CommandLineParser::parse(&self, cache: &mut ParsePrefixCache, …)`. Owner: the legacy `Engine` from 6.3e, then
  `CompletionSession` from 6.4e (`parse_inputs()` split borrow).
- One `CommandLineParser::clear_caches(&self, cache)` = `specs.forget_loaded_specs()` then `cache.clear_keeping_first()`
  (today's `Parser::clear_caches`, `parser.rs:137-143`, forget first), called at the old position by
  `ApplyCompletionSettingsUseCase`.
- Facts: plan §1, in-process aggregates are driver-owned and passed `&mut`. Four parser tests already own their cache
  (recheck correction: at `ff2db5a` `parser/tests.rs:514` `generate_spec_cache_key`, 634 `prefix_cache`, 673
  `cached_nested_state_keeps_this_commands_prefix`, 710 `first_token_spec_identity_is_stable`).
- Rejected: a `RefCell` inside the parser; the use case holding the repository.

### Q7 Homes of the small constants and helpers
- `SYNC_CALL`: `pub(crate)` in `generator_trigger_policy.rs` (callers `engine.rs:652` getQueryTerm → use case, 697
  function trigger → runner). `SLICE` (50 ms): private in `generator_runner.rs` (read only by `pump`, 1032).
  `FIRST_TOKEN_TTL`: `completion_session.rs`. `env_var` (with its `cfg!(windows)`): `completion_request_policy.rs`,
  moved only in 6.5b (one caller, 588). `CacheLookup<V>`: its own file `value_objects/cache_lookup.rs` (6.4a).
- None is merged with `LOAD_TIMEOUT`, the runtime's 1000 ms `SLICE` (`quickjs_spec_runtime.rs:57-59`) or the two 5 s
  `TIMEOUT`s (`parser.rs:34`, `quickjs_spec_repository.rs:29`); the parser's `TIMEOUT` and `dev_mode` move privately
  into `command_line_parser.rs` and stay separate from the repository's copies (P14).

### Q8 The generator loop: `GeneratorRunner`, `Rc`-held; `GeneratorCommand` keeps the hand-off payloads
- `completion/application/generator_runner.rs`: `GeneratorRunner { runtime: Rc<dyn SpecRuntime>, directories: Rc<dyn
  DirectoryLister>, clock: Arc<dyn Clock + Send + Sync> }` with `trigger(&self, session, parsed, context, tokens,
  report) -> Option<Vec<GeneratorKey>>` and `progress(&self, session, report) -> Option<bool>`; `None` = a newer
  request interrupted (today's `return None` and `Err(Aborted) => return`).
- `CompleteCommandLineUseCase` and `ProgressGeneratorsUseCase` each hold it as `Rc<GeneratorRunner>` (precedent:
  Phase 4's `IntegrationInstaller` held by `Rc` in two use cases; a use case never calls another).
- **6.4f** moves `update_slots`, `begin`, `fetch`, `script_input`, `exec_output`, `custom_result`, `advance` and `pump`
  minus its reply verbatim: `self.` → `session.`/the ports; `Instant::now()` → `clock.now_instant()` at the same places
  and counts (GEN-GT8; ENG-E24: two eager reads in the deadline, one after); `now_ms()` → `clock.now_ms()`;
  `report(CompletionNotice::GeneratorFailed(e))` right before each `session.settle(run, Fail(e))`, background runs
  included (ENG-E25).
- **6.4g** (after the lifecycle suite, Q18) moves the pure step transitions into `GeneratorRun` methods returning
  `GeneratorCommand`: `StartCall(SpecFunctionCall, GeneratorContext)`, `StartExec(ExecRequest, GeneratorContext)`,
  `StartSleep(u64)`, `Settle(GeneratorOutcome)`. The runner keeps the cache protocol (`fetch`), the directory listing
  and the port calls, one method per legacy fn.
- Facts: `update_slots` starts runs and may settle them synchronously (`begin` at 768); `pump` advances them (1051).
  Clippy `large_enum_variant` compares the largest and second-largest variants: `StartExec` ≈ 264 B, `StartCall` ≈
  192 B (difference ≈ 72 B < 200), so the hand-off shape is clippy-safe (not measurable without cargo: the gate's
  clippy run decides; box the `StartExec` payload only if it fires, O8).
- Rejected: the use cases owning the bodies (duplicated, or one use case calling the other); a per-call
  `GeneratorRunner<'a>`; context-free commands.

### Q9 `SpecRuntime` gains `read` and `read_exec_stdout`; no `resolve_script`
- 6.4c adds `read(&self, value: &SpecValue, kind: SpecFunctionKind, context: &GeneratorContext) -> SpecFunctionResult`
  (QuickJs: `function_result(kind, &spec_value_to_json(value), context)`) and `read_exec_stdout(&self, value:
  &SpecValue) -> String` (`spec_value_to_json(value).get("stdout").and_then(Value::as_str).unwrap_or("").to_string()`).
  `InMemorySpecRuntime` implements both through the real decoders (D2; the fitness test skips `#[cfg(test)]` items, so
  a test-only `fig_spec_mapper` import in `spec_runtime.rs` passes).
- In the typed switch (6.4d): `Argv(a)` → `SpecValue::List(a.iter().cloned().map(SpecValue::Text).collect())`,
  `Shell(s)` → `SpecValue::Text(s)`; every script value goes through `read(.., SpecFunctionKind::Script, ..)` (which
  reaches the private `script_resolution`); the timeout is exactly `script_timeout(settings).max(g.script_timeout()
  .unwrap_or(0)).max(request.timeout().unwrap_or(0))`, applied with `ExecRequest::with_timeout(Some(t))` (6.4c, TDD)
  after `ScriptResolution::Ready` and before `start_exec`; the cache key uses `cache_key_source`;
  `GeneratorCache<V>`/`CacheLookup<V>` are generic (`V = Value` from 6.4a, `SpecValue` from 6.4d); raw pre-step values,
  a malformed value still gives `[]` (GEN-GT21/22).
- Facts: `script_resolution` = `engine::script_input` (925-937) up to the timeout; `input.get("timeout").and_then(as_u64)`
  ≡ `input["timeout"].as_u64()` because `script_exec_input` always returns an object; the final timeout is the same max;
  serde reports the same first error; `json!(a)`/`json!(s)` equal the lifted values; the `SpecValue` round trip is exact.
- Rejected: `resolve_script(ScriptValue)` (a third entry point re-joining paths the code never split). Recorded as
  amending Phase 5 deviation 9 (plan lines 1032-1033 and 1120).

### Q10 Recency
- Entity `SuggestionRecency` (private map): `new(record)`, `to_primitives()`, `record(command, name, at_ms) -> bool`
  (`false`, no change, for a name holding `'↪'`), `of_command(command) -> Option<&HashMap<String, u64>>`.
  `CompletionSession` holds it; `CompletionSession::new(settings, wrapper_env, recency)`.
- `domain/repositories/recency_repository.rs`: `trait RecencyRepository { fn load(&self) -> SuggestionRecency; fn
  save(&self, recency: &SuggestionRecency); fn flush(&self); }` + `#[cfg(test)] InMemoryRecencyRepository`.
- `StateFileRecencyRepository::new(Option<PathBuf>)`: private `RECENCY_KEY`, `type RecencyRecord` (the serde map;
  `HistoryRecord` precedent) and `load_recency`/`save_recency`, bodies verbatim including the purge guard and the
  newest-wins merge loop; error type `JsonDocumentStoreError` (`to_value` mapped to `Serialize`); warn `engine: saving
  recency: {}` via `display_chain` (same text as anyhow's `{:#}`); fields `saving: Arc<AtomicBool>` and `pending:
  Arc<Mutex<SuggestionRecency>>`.
- `save`: no state file → nothing; else set `pending` **before** `saving.swap(true)`; if the swap was `false`, spawn
  thread `recency`: sleep 1 s → `saving.store(false)` → snapshot `pending` → write; spawn failure: `saving.store(false)`
  and a synchronous write. `flush` writes only if `saving` is set (GT34).
- `RecordSuggestionInsertionUseCase`: `if session.record_insertion(cmd, name, clock.now_ms()) {
  recency.save(session.recency()) }`. Ranking reads `session.recency().of_command(cmd)` without a lock.
- **The merge stays in the adapter**: it resolves conflicts with the file another session wrote, and only the adapter's
  save uses it (recorded against §4's "Recency + merge").
- Proof of equality: only the engine thread inserts (1212); the thread clears `saving` before it snapshots
  (1226-1227). If an insert's pending update comes after the thread's snapshot, the thread's `store(false)`
  happens-before that insert's `swap(true)`, so the swap returns `false` and a new write follows a second later — the
  guarantee today's live-map snapshot gives. The shutdown flush writes the latest map in both shapes. Today's
  poisoned-lock branch (1140) is unreachable: no code panics while holding the lock.
- Rejected: a live map in the adapter (a repository holding state and running the entity's rule); a `merge_newest`
  entity method (YAGNI).
- Tests: `recency_round_trip_keeps_other_keys`, `recency_save_merges_other_sessions` are CHANGED only by the rename
  `Recency` → `RecencyRecord`.

### Q11 D5: one `CompletionNotice` callback
- Application DTO `CompletionNotice` (`Debug, PartialEq`): `GenerateSpecFailed(String)` (the whole line),
  `GeneratorFailed(String)`, `NoSuggestions(String)`. Created in 6.3d by its first producer (the parser); the other
  variants arrive with their producers (6.4f runner, 6.5b use case).
- One callback shape, `report: &mut dyn FnMut(CompletionNotice)`, through `CommandLineParser::parse`,
  `GeneratorRunner::{trigger, progress}` and both big use cases' `execute`.
- `GenerateSpecFailed` carries `format!("generateSpec ({}): {e:?}", tokens[0])`, built in the parser with its comment.
- The infrastructure caller logs, with explicit match arms: `warn(line)`; `debug("engine: generator failed: {e}")`;
  `debug("engine: no suggestions: {e}")`. It is `engine.rs`'s free `log_completion_notice` until 6.6b, then
  `EngineServer::log_notice`. In 6.3d the `Parser { parser, cache }` facade logs the notice at warn.
- `CompletionSession::settle` returns `bool` and never logs.
- Facts: the three sites log mid-call, interleaved with the adapter's `js console`/`js settle` lines (ENG-E25,
  PAR-T2).
- Rejected: returning texts after the call (reorders lines); three callback shapes; `GenerateSpecFailed { spec_name }`
  (prints the wrong word); `&dyn Fn` (a collecting test needs a `RefCell`); `settle` returning text.

### Q12 The `CompletionEngine` port, its double, the starter, the switch
- `terminal_session/application/ports/completion_engine.rs` (+ `ports/mod.rs`, + `application/mod.rs`:
  `pub(crate) mod ports;`) holds the infallible trait (ENG-E31):

  ```rust
  trait CompletionEngine {
      fn complete(&self, request: CompletionRequest);
      fn command_finished(&self, entry: HistoryEntry);
      fn command_ended(&self);
      fn record_insertion(&self, command: String, name: String);
      fn request_llm_ghost_text(&self, request: u64, buffer: String, cwd: PathBuf, shell: Shell);
      fn apply_settings(&self, settings: Settings);
      fn apply_session_context(&self, env: HashMap<String, String>, alias: Option<String>);
      fn toggle_fuzzy_search(&self);
      fn shut_down(&self);
  }
  ```

- `#[cfg(test)] enum CompletionEngineCall`, one variant per method named after it: `Complete`, `CommandFinished`,
  `CommandEnded`, `RecordInsertion { command, name }`, `RequestLlmGhostText { .. }`, `ApplySettings`,
  `ApplySessionContext { env, alias }`, `ToggleFuzzySearch`, `ShutDown`. `#[cfg(test)] RecordingCompletionEngine {
  calls: Sender<CompletionEngineCall> }` + `new(sender)`; each method does `let _ = send`.
- `bootstrap/channel_completion_engine.rs`: `ChannelCompletionEngine { sender }` (each method `let _ =
  self.sender.send(..)`) + `pub(crate) fn start_completion_engine(replies: Sender<Msg>, settings: Settings) -> Rc<dyn
  CompletionEngine>`, wrapping `crate::engine::spawn` and `types::EngineMsg` from 6.6a, then `engine_server::spawn` and
  `EngineMessage` from 6.6b.
- `pty.rs`: `pub(crate) type CompletionEngineStarter = fn(Sender<Msg>, Settings) -> Rc<dyn CompletionEngine>;` threaded
  through `run(shell, cmd, start)` and `Session::start`, called at `pty.rs:461` inside the same `catch_unwind`
  (ENG-E30); field `engine: Rc<dyn CompletionEngine>`; the nine sends become port calls at their lines, in order
  (ENG-E32). `cli::debug(cmd, start)` and `debug_shell(shell, start)`; `bootstrap/cli.rs:139, 141` pass
  `start_completion_engine`. No legacy → bootstrap edge appears in production.
- **When**: 6.6a, the last step of run 3; it needs only `CompletionRequest` (6.2a).
- TC7: `enter_records_the_submitted_line` and `no_history_command_still_ends` change one line each (`s.engine =
  Rc::new(RecordingCompletionEngine::new(tx));`); `rx.try_iter()` and the patterns `CommandFinished(h)`/`Complete(_)`/
  `CommandEnded` stay path-only under testlock's loose form (`(?:\b\w+::)+(\w+)` → last segment, `testlock.py:125`);
  helpers change: `session()`, `session_reading_context_in` (returns `Receiver<CompletionEngineCall>`),
  `contexts_sent` (matches `ApplySessionContext { env, alias }`, keeps its panic text); the five context tests keep
  their bodies. TC8: `debug_shell(Some(Shell::Bash), start_completion_engine)`, `--exact` string unchanged.
- Rejected: a `RefCell<Vec<..>>` double + `take_calls()` (two edits per test); names mirroring `EngineMessage`; a
  stub spawner; the late switch.

### Q13 Bootstrap files, the `git mv`, the false doc
- Nothing creates `bootstrap/engine_server.rs` before **6.6b**, which has three commits:
  1. `git mv src/engine.rs src/bootstrap/engine_server.rs`; `Engine` → `EngineServer`; `spawn` stays.
  2. `EngineInbox` + its two tests + `runtime_with_inbox` → `bootstrap/engine_inbox.rs`; `types::EngineMsg` →
     `bootstrap/engine_message.rs` as `EngineMessage` (`pub(crate)`), variant names and payloads unchanged,
     `Runtime(RuntimeEvent)` kept. The adapter switches; `types.rs` drops `EngineMsg` and the imports only it used.
  3. Doc only (U1/M5): "The thread exits on `EngineMessage::Shutdown`; the inbox keeps a sender, so its channel never
     disconnects."
- The server still owns the inbox **instance**; only the type's file differs from §4 and the Phase 5 hand-off (a
  recorded deviation, O3). `bootstrap/cli.rs`'s four clap types are the D4 exception, not a precedent.

### Q14 Legacy ghost touchpoints and `llm_history` stay in the server
- No `legacy_ghost_text.rs`. One `impl EngineServer` block (in `engine.rs` until 6.6b, then
  `bootstrap/engine_server.rs`) marked `// legacy ghost text: Phase 7.6 replaces this block` holds:
  1. `local_ghost_text(&self, result: &CompletionResult) -> Option<String>`: the body of `ghost` (1188-1202) over
     `session.settings()`, `history.recent()`, `result.buffer()`, `result.cursor()` and
     `inline::spec_ghost(result.suggestions())` (6.2b left `CompletionResult::suggestions()` `#[cfg(test)]` by
     D7: drop that attribute, keep the name, in the commit that first reads it here);
  2. `request_llm_ghost_text(&mut self, req, buffer, cwd, shell)`: `llm` (1247-1275) verbatim, settings and env from
     the session, the private `llm_history` (267-270) and the field `ghost_cache: Arc<Mutex<inline::GhostCache>>`;
  3. `clear_ghost_text_cache(&self)`: the clear of 463-465, called between `record_command` and
     `ResetGeneratorsUseCase`.
- `llm_history_honours_history_count` rides the `git mv` path-only.

### Q15 The local ghost on every reply
- From 6.5b the use cases return results with `ghost: None`. The engine (later `EngineServer::send_reply`) sets
  `result.with_ghost(self.local_ghost_text(&result))` on **every** result it sends (Parse, Generators, error), just
  before `Msg::Completion`. `error()`'s ghost flag is deleted. 6.7 amends the plan's event table: the idle row becomes
  `ProgressGenerators → SuggestLocalGhostText → reply if changed`, and the `Complete` row says error replies get the
  local ghost too.
- Proof: ENG-E16/E17; the ghost reads settings, history and the result, logs nothing, and no message is handled between
  build and send; the recompute is load-bearing (`pty.rs:881-883`).

### Q16 Test fixtures
- `completion/domain/test_fixtures.rs` (`#[cfg(test)] pub(crate) mod test_fixtures;`, each item domain-only, CON-T28)
  is created in 6.2d with `pr_for`, `opts`, `sub`, `opt`, `arg`, `names`, `ctx` (`suggest.rs:1442-1512`). Phase 7
  imports `opts`/`ctx` from it; Phase 8 adds `comp`/`item`.
- `completion/application/test_fixtures.rs` (`#[cfg(test)]`) holds Q18's harness; it imports only
  domain/application/port types, so it passes even when read as production code.

### Q17 Port doubles come with their ports, before any consumer
- 6.2e writes `DirectoryLister` + `InMemoryDirectoryLister` and `ExecutableCatalog` + `FixedExecutableCatalog` (with a
  `scans()` counter) test-first, in the port files.
- 6.4c adds, test-first: `InMemorySpecRuntime::{completing(function, outcome), completing_execs(outcome),
  interrupting(), applied_settings()}` and sleeps that finish `Ok(SpecValue::Null)` (the names do not collide with
  `answering`/`calls`); `InMemorySpecRepository::forgets()`; `FixedClock` with its instant behind a `Mutex` plus
  `#[cfg(test)] advance(Duration)` (still `Send + Sync`).
- Facts: plan §1 (double in the port file); Phase 5 deviation 16 (a double may land before its consumer).

### Q18 New use-case tests at the use-case seam, over one InMemory harness
- The harness wires the use cases over `InMemorySpecRepository`, scripted `InMemorySpecRuntime`,
  `InMemoryDirectoryLister`, `FixedExecutableCatalog`, `InMemoryHistoryRepository`, `InMemoryRecencyRepository` and
  `FixedClock`, line for line as `build_completion_module` wires the adapters.
- 6.5b: the request-pipeline tests on `CompleteCommandLineUseCase`; 6.5c: 12 lifecycle tests on
  `ProgressGeneratorsUseCase`, arranged through `CompleteCommandLineUseCase`; `GeneratorRun`'s transitions get pure
  tests in 6.4g, **after** the lifecycle suite. No runner-level suite; 6.0a's engine-level tests stay the black-box
  guard.

### Q19 Session-context tests and `session_update_refreshes_first_token_items` (TC9)
- The five Phase 4 tests move in 6.5a to `apply_session_context_use_case.rs` with **byte-identical bodies**; the
  helpers absorb the change: `engine_with_wrapper_env(_: &TempDir) -> (CompletionSession, ApplySessionContextUseCase)`;
  `report_session_context(e: &mut CompletionSession, js: &mut ApplySessionContextUseCase, env, alias)` (calls
  `execute`); `e.env()` and `e.alias_map(Shell::Bash)` are session methods with today's names and types (`AliasTable` is
  the std map). The helper doc says why the names stay (O2).
- `session_update_refreshes_first_token_items` (`engine.rs:1361-1391`) is edited in this order (recheck correction:
  its body names only `parser_over(&js)`, never the parser type, so 6.3e does **not** change it): 6.2f `s.names[0]` →
  `s.names()[0]` (1386); 6.4f `Engine::with` gains the runtime; 6.5a **D6 split**. The completion half keeps the name
  in `apply_session_context_use_case.rs`: fresh session; `session.first_token_suggestions(&Instant::now, ||
  first_token_items(&[], &HashMap::new(), &[]))` is empty; `ApplySessionContextUseCase.execute(&mut session,
  HashMap::new(), None)`; the same call with `{zzal: "git status"}` gives `["zzal"]`. The bootstrap half
  `session_update_refreshes_first_token_items_keeps_the_engine_running` asserts `handle(Session{..})` is `true` and
  carries the old helpers' `assert!(e.handle(..))`, through two test helpers, `engine_over(dir)` and
  `session_context(env, alias)`: its body never names `Engine`, a constructor or `EngineMsg`, so 6.5c (`Engine::with`
  goes) and 6.6b (`EngineServer`, `EngineMessage`) edit the helpers only.
- `parser_over` (1393-1397) stays in the engine tests until 6.5c (6.0a's `serve` and `engine_over` use it; the plan
  hand-off lists it among the helpers, but the five moved bodies do not need it). 6.5c builds both helpers' engines
  through `build_completion_module` and replaces it with `spec_repository_over(js)` (the same bundle, no parser).
- Facts: plan §9's Phase 4 hand-off names `ApplySessionContextUseCase` for all six; Phase 5 P12 kept these bodies
  identical through helper changes; `SpecIndex::default()` plus `PATH` `None` give no specs and no executables.
- Rejected: `complete_command_line_use_case.rs` as the destination; rewriting the five bodies (5 CHANGED for nothing).

### Q20 D6 splits only across modules (TC1, TC2)
- `cache_ttl_swr_max_age` moves **whole** to `generator_cache.rs`, path-only (the `GenCache`/`Lookup` renames sit before `::`, which testlock's loose form strips; CHANGED only if a turbofish is needed);
  `json!(1)` stays (`V = Value` in the test, D2).
- Phase 6's D6 splits: `spec_identity_survives_dev_reload_and_cache_clear` (specs/completion, P16) and
  `session_update_refreshes_first_token_items` (completion/bootstrap). TC2 (Phase 7): `request_bodies`/
  `response_parsing` stay whole.

### Q21 Legacy files leave with their last item

| File | Leaves in | How |
|---|---|---|
| `shell_parser.rs` | 6.1a | `git mv` |
| `parser.rs` + `src/parser/` | 6.3e | deleted; `tests.rs` is `git mv`'d |
| `suggest.rs` + `mod suggest;` | 6.4a (U2) | deleted once `triggered`/`dir_changed`/`cache_key`/`Lookup`/`GenCache` and `triggers`/`cache_ttl_swr_max_age` leave |
| `engine.rs` | 6.6b | `git mv` to `bootstrap/engine_server.rs` |

6.7 deletes nothing; it asserts all four are gone.

### Q22 New test names (TC11)
- One owner per name ("Test moves", new tests): `should_not_record_an_auto_execute_row` belongs to `SuggestionRecency`
  (6.4b); `should_not_save_the_recency_after_an_auto_execute_row` to the use case (6.5a); the port tests to 6.6a; the
  lifecycle names to 6.5c. Each step runs `grep -rn "fn <name>(" src` (must be empty) before adding a name. At
  `ff2db5a` none of the planned names exists (recheck R11).

### Q23 `disableForCommands`
- It stays in `SpecRepository::find` through `SpecLookup::disables` (Phase 5 deviation 3). `CompletionRequestPolicy`
  has no such rule. **Recheck:** 5.5 already rewrote plan Phase 6 step 3 and §4's policy line, so 6.7 makes no plan
  edit for it beyond the Phase 6 note.

### Q24 The result builder is a domain service
- `domain/services/completion_result_builder.rs`: `build_completion_result(session: &CompletionSession, phase:
  CompletionPhase, recent_history: &[HistoryEntry]) -> CompletionResult` (ghost `None`). Its body is `reply`
  (1101-1158 + 1160-1184) minus the ghost line (1159): every settings getter verbatim (M10),
  `expect("reply without a current request")` kept. Callers pass `history.recent()` (an `Rc` of the load-time
  snapshot, no I/O).
- Rejected: an application service over two repositories; ranking inside the aggregate.

### Q25 `CompletionSession` (6.4e): contents and API
Fields: settings, wrapper env, session env/alias, aliases memo, recency, `user_fuzzy`, `last_arg_key`, parse-prefix
cache, slot argument, slots, previous search term, next generation, runs, `GeneratorCache<SpecValue>`,
`Option<ServedRequest>`, first-token cache. Every method is a moved field-access group, no new logic:
- **life**: `new(settings, wrapper_env, recency)` (sets `user_fuzzy` from `AUTOCOMPLETE_FUZZY_SEARCH` before moving the
  settings, GT41); `settings()`, `replace_settings`; `env()` (an empty session env means the wrapper env, GT38);
  `alias_map(&mut self, shell) -> AliasTable` (memoised per shell, returns a clone, GT39);
  `apply_session_context(env, alias)` (also clears the aliases memo and the first-token cache);
  `toggle_fuzzy_search()`; `enter_argument(argument_key)` (569-572); `user_fuzzy()`.
- **parse**: `parse_inputs() -> (&mut ParsePrefixCache, &Settings, &HashMap<String, String>)`; `parse_prefix_cache()`.
- **served request**: `serve`, `served`, `forget_served_request`.
- **first token**: `first_token_suggestions(&mut self, now: &dyn Fn() -> Instant, list: impl FnOnce() ->
  Vec<SuggestionCandidate>) -> Vec<SuggestionCandidate>` (one clock read for the TTL check, `list` only on a miss, a
  second read for the stamp, ENG-E22); `forget_first_token_suggestions()`.
- **recency**: `recency()`, `record_insertion(command, name, at_ms) -> bool`.
- **generators** (only `GeneratorRunner` uses these): `reset_generator_slots()` (496-500), `clear_generator_slots()`
  (713-716), `assign_generator_slots(arg, keys)`; `slot_argument()`, `slot_count()`, `previous_search_term()`,
  `remember_search_term`; `fire_slot(i) -> (u64, GeneratorKey)`, `is_current(&run)`, `settle(run, outcome) -> bool`
  (no log); `push_run`, `take_runs`, `keep_runs`, `has_run_on(task)`, `has_running_generators()`;
  `slot_views(queries)`, `any_slot_loading()`, `generator_cache_mut()`.

The step brief fixes the final list. Its tests may go to `completion_session/tests.rs` past 500 lines.

### Q26 Use cases, module, spawn order

| Use case | Holds | Does |
|---|---|---|
| `ApplySessionContextUseCase` | unit struct | session context effects |
| `ToggleFuzzySearchUseCase` | unit struct | flips the fuzzy toggle |
| `ResetGeneratorsUseCase` | unit struct | reset slots + forget the served request (`engine.rs:466-467`) |
| `RecordSuggestionInsertionUseCase` | `{ recency, clock }` | records, then saves when recorded |
| `ApplyCompletionSettingsUseCase` | `{ runtime, parser: Rc<CommandLineParser> }` | the sequence below |
| `CompleteCommandLineUseCase` | `{ parser, runtime, generators, executables, history, clock, index: SpecIndex }` | `execute(&self, session, &CompletionRequest, report) -> Option<CompletionResult>` |
| `ProgressGeneratorsUseCase` | `{ generators, history }` | `execute(&self, session, report) -> Option<CompletionResult>`, `Some` only if a current slot changed and a request is served |

- Unit structs are built as `ToggleFuzzySearchUseCase`, never `::default()` (clippy).
- `ApplyCompletionSettingsUseCase` runs today's sequence (`engine.rs:476-482`) on every Settings message, nothing else
  (ENG-E10): 1 `runtime.apply_settings(&s)` (the i64 floor); 2 `session.replace_settings(s)`; 3
  `parser.clear_caches(session.parse_prefix_cache())`; 4 `session.reset_generator_slots()`; 5
  `session.forget_first_token_suggestions()`.
- `CompleteCommandLineUseCase` keeps `compute`'s order (ENG-E14) and its partial mutations before an abort (ENG-E9).
- `infrastructure/completion_module.rs`: `CompletionModule { the seven use cases, pub(crate) recency: Rc<dyn
  RecencyRepository> }` (the `HistoryModule` precedent exposes its repository); `build_completion_module(&SharedDependencies,
  specs, runtime, history, index)`, no I/O.
- `spawn` (moved out of `Engine::new` in 6.5c; GT44/ENG-E28) builds, in order: 1 the inbox; 2 the runtime (on error
  log `engine: js host: {e:#}` and return); 3 `SharedDependencies::from_env()`; 4 the repository; 5 the index
  (`install_spec_bundle.execute().and_then(load_index)`, `engine: specs: {e:#}`, `SpecIndex::default()`, ponytail
  double ensure kept); 6 `build_history_module`; 7 `build_completion_module`; 8 `completion.recency.load()`; 9
  `CompletionSession::new(settings, std::env::vars().collect(), recency)`; 10 `EngineServer { .. }.run(&runtime,
  &inbox)`. After the loop: `completion.recency.flush()`. No `LoadSpecIndexUseCase` (Phase 5 open question 5).

### Q27 The parser step's safety cut
6.3d has three commits, the risky one last:
1. **Test-only harness swap** while `Host` still exists. The Mock keeps the fields `loads` and `fns` (test bodies read
   `m.loads` as a field 12 times — `parser/tests.rs:305, 327, 344, 362, 363, 387, 404, 415, 422, 642, 653, 684` — and
   call `m.fns.insert(id, Box::new(..))` after `Mock::with` at 8 sites — 438, 458, 500, 507, 591, 607, 608, 778).
   Recheck: there is no `Functions` at HEAD; this commit introduces it (`prep6/parser.md` §4.3: an `Rc` map with an
   `insert(id, Box)` method, so the callbacks survive being handed to a runtime) as the type of `fns`. The Mock's
   `Host` impl moves its existing `call` body (`call_arguments` → callback → `function_result`, D2) into an
   `InMemorySpecRuntime` answer, swaps `specs`/`loads` onto `InMemorySpecRepository` (`loads` is filled with
   `extend(repository.finds())` after each parse, whether or not it failed), and uses the real `substitute_token`
   (PAR-T36). Bodies stay unchanged. The error round trip is exact: a callback's `Aborted` → `SpecError::Aborted` →
   `Aborted`; `NoSuggestions(t)` → `SpecError::Invalid(t)` → `NoSuggestions(t)` (`Invalid`'s Display is its text); an id
   never inserted gives `InMemorySpecRuntime`'s `Invalid("unknown fn")` (`spec_runtime.rs:133`), which comes back as the
   old `no("unknown fn")` text. `GeneratorContext::default()` inside the answer is exact (the parser calls only `Alias`,
   `LoadSpec`, `GenerateSpecCacheKey`, `GenerateSpec`; the context is read only for `Script` and `PostProcess`).
   `Mock::default()` keeps working (`InMemorySpecRepository: Default`).
2. TDD: `ParsePrefixCache`.
3. `CompletionNotice`, then `CommandLineParser` with a private `ParsePass` inside `parser.rs` (`host.` → `self.`, cache
   methods, the warn → the notice with the same format string and comment), `clear_caches`; `Host`/`JsHost`/
   `ParseCaches` deleted; a `Parser { parser, cache }` facade keeps `engine.rs` unchanged until 6.3e deletes it (an
   intra-phase shim); D8 on `apply_load_spec`'s `_ => Ok(None)` (862); `Parser::load_spec` deleted with its last caller;
   `spec_identity_survives_dev_reload_and_cache_clear` split (P16). The repository half lands in
   `quickjs_spec_repository.rs`'s tests, which already have the helpers `repository()` (250), `write_mk(dir, source)`
   (262), `find_mk(repository, dir, settings)` (269) and `const MK` (247), the same `mk` spec the split test writes.

### Q28 Placements beyond §4's letter, and Phase 7/8 items §4 lists
- `after_sep` → `suggestion_query.rs`; `same_row` → private in the ranker; `cmp_first` → private in the assembler;
  `exe_name` and the PATH split → `PathExecutableCatalog`, and `first_token_items` takes the executables; `ArgvState`
  (+ private `ArgState`) lives in `argv_parser.rs`; `CompletionError` has no Display (only its Debug is printed);
  `AliasTable` is a type alias; escapes are not merged into `ShellQuoting`.
- Phase 7 items: `ghost_key` stays in `pty.rs` (`ghost_keys_and_words` is D6-split across ghost_text/pty);
  `cd_target_validator` is a pure domain `cd_target` + an application `validate(.., probe)`. Phase 8: the menu fixtures
  join `test_fixtures.rs`.

### Q29 `GhostTextError` (Phase 7, C24)
Recommendation to Phase 7: one module error with **transparent** `Settings(SettingsError)` and
`SourceChooser(io::Error)` (Display and `source()` delegate), pinned by a test comparing `format!("{:#}",
anyhow::Error::from(..))` with and without the wrapper for a corrupted settings file and the no-home error (golden
`golden.txt:6157-6160, 6250-6253, 8889-8892`, CON-T21).

### Q30 `inline-diff.sh`
Phase 6 changes no inline CLI path (only `Suggestion` reads in `spec_ghost`, guarded by `spec_ghost_rules`). The script
is Phase 7's first step (7.0), run against the baseline and the Phase 6 end binary before any inline code moves.

### Q31 Trap ids
Every brief and plan note cites prefixed ids (here `P6-…`; the research's `GEN-GT24`, `ENG-E3`, `PAR-T1`, `CON-T16`
mean the same traps).

The briefs also cite the research reports' **proposal ids** (not traps; the reports are not committed). What each
means here:

| Id | Meaning (where this file has it) |
|---|---|
| TOK2 | `AliasTable` is a type alias of the std map (Q28, deviation 3, P6-TOK-16) |
| TOK3 | `Token` and `CommandLine` get private fields, `new` and accessors (6.1c) |
| TOK4 | the 16 tests go to the sibling `command_line_tokenizer/tests.rs`, as §4 says (P6-TOK-22) |
| TOK5 | `shell_parser.rs` leaves in 6.1a by `git mv`, no shim (Q21, deviation 2) |
| TOK7 | the two quote strips stay separate (P6-TOK-14) |
| TOK9 | the private AST names (`P`, `Node`, `Kind`, `command_info`, `descendant_at`, `reduce`) are kept |
| TOK10 | D8 on `P::inner`'s `_ => (0, 0)`: the 15 other `Kind`s listed explicitly, mapping as the catch-all did (P6-TOK-9) |
| TOK12 | no new tokenizer tests |
| TOK13 | `substitute_token` rebuilds through `Token::with_raw` and `CommandLine::new` instead of mutating fields (P6-TOK-17) |
| TOK 2.6 / 2.7 / 2.8 | the tokenizer report's trap sections = P6-TOK-17 / P6-TOK-18 / P6-TOK-19 |
| SUG3 | the pipeline VOs' fields go private; in-place writers (`update_priorities`, `option_chain`) use `set_*` (6.2f) |
| SUG4 | `GenView` becomes the VO `GeneratorSlotView` in its own file (deviation 21) |
| SUG5 | `after_sep` lives in `suggestion_query.rs` (deviation 22) |
| SUG6 | `same_row` private in the ranker, `cmp_first` private in the assembler, `cmp_locale` in the filter (deviation 22) |
| SUG8 | `ExecutableCatalog` returns every executable on `PATH`; `exe_name` and the PATH split sit in the adapter (deviation 22) |
| SUG9 | `DirectoryLister` is synchronous; `list_dir` moves whole into the adapter (deviation 36) |
| SUG10 | `first_token_sources` stays whole in `path_executable_catalog.rs`'s tests (deviation 36) |
| SUG11 | `empty_search_term_is_no_prefix_match` lives in `suggestion_filter.rs` until Phase 7.3 (deviation 36) |
| SUG13 | suggest.rs's function names are kept (deviation 36) |
| SUG14 | escapes are not merged into `ShellQuoting` (deviation 22) |
| SUG16 | no positional bool/`usize` constructors: `Default` or `new` plus named `with_*` (Q2, deviation 6) |
| PAR3 | `ArgvState` and its API live in `argv_parser.rs` (deviation 8) |
| PAR5 | accessor shapes (`&Vec<T>`, `&Option<T>`, Copy by value) and the kept accessor `completion()` (deviation 33) |
| PAR6 | `CompletionError` is `ParseError` renamed, Debug only, no Display (deviation 7) |
| PAR10 | `ParseCtx` becomes `ParseContext` in its own application file, `pub(crate)` fields (deviation 21) |
| PAR11 | the parser test fixture keeps the name `Mock` and its fields (deviation 33) |
| PAR17 | the parser gets only the four new tests of 6.3d |
| PAR18 | `no()` stays a private fn of `command_line_parser.rs` (deviation 33) |
| PAR19 | carried deviations: the let-else and `_ => None` reads of `SpecFunctionResult` move unchanged; matches on `CompletionError`/`SpecError` keep explicit arms (D8) |
| PAR20 | `Parser::load_spec` is deleted with its last caller in 6.3d (deviation 29) |
| CON10 | `EngineMessage` keeps every variant name and payload (6.6b) |

### Q32 Source-map traps M1-M10

| Trap | Brief that carries it |
|---|---|
| M3 (a quoted spec name holding `{cursor}` still becomes the marker; only the first is consumed) | 6.2b and 6.2d |
| M4 (`template_json`: `null` for missing fields, sorted keys; the `Listed` path encodes in the adapter) | 6.4d |
| M5 (the false spawn doc) | 6.6b commit 3 (Q13) |
| M10 (call-site defaults inside `reply`) | 6.5b (Q24) |
| M1, M2 | Phase 7.1 |
| M6 | Phase 7.4 |
| M7 | Phase 7.5 |
| M8, M9 | Phase 7.7 |

### Q33 Per-step rules (gate, lock, evidence)
Every step:
- **gate**: full `scripts/hexagonal-refactor/gate.sh` (`GATE PASS`) before each commit;
- **test lock**: `testlock.py . ef807c1 --diff` → MISSING 0, DUPLICATE 0; `testlock.py . phase6-start --diff` →
  MISSING 0, DUPLICATE 0, every CHANGED test in the step's list (recheck: the "every CHANGED listed" check applies to the
  `phase6-start` run only; against `ef807c1` many tests are CHANGED since earlier phases);
- **moves**: by script or `git mv`, with a block diff against `git show phase6-start:<file>` showing only paths,
  visibility, glossary names and the listed edits;
- **fitness**: the local registration of "Cross-cutting", silent, then reverted;
- **cfg**: the grep of "Cross-cutting" on touched files and a hand check of never-compiled branches;
- **clippy budget 3**; no `saturating_*` rewrite of the budget arithmetic (GT9);
- **commits**: `refactor(completion): <why>` / `test(completion): …` (or the touched module: `specs`, `shared`,
  `terminal_session`, `bootstrap`), ≤ 72 characters, the trailer, `git commit -F`; new seams follow RED → GREEN →
  commit → REFACTOR → commit with `should_…` names, AAA and no mocks;
- **`all_bundle_modules`** after 6.3e, 6.4c, 6.4d, 6.4f, 6.5c, 6.6b and 6.7;
- **engine steps**: the 6.0a suite 20× (`cargo test --locked engine::tests -- --test-threads=1`, later by its new path
  `bootstrap::engine_server::tests`); the two inbox tests 50× by full path at 6.5c and 6.6b
  (`engine::tests::wait_aborts_and_defers_on_complete`,
  `engine::tests::should_deliver_a_runtime_event_read_by_the_engine_loop_to_the_runtime`; after 6.6b
  `bootstrap::engine_inbox::tests::…`), each run printing `1 passed`; `debug_shell_survives_closed_terminal` alone by
  full path (`cli::tests::debug_shell_survives_closed_terminal -- --exact`) at 6.6a, 6.5c, 6.6b and 6.7.

### Module map (binding; from the decisions' section 3)

Layers: **D** domain, **A** application, **I** infrastructure. `pub(crate)` at most everywhere. Domain fields private
by the end of the step marked "priv"; `…Parts` structs and application inputs/DTOs keep named `pub(crate)` fields.
Every `mod.rs` holds only child declarations, alphabetical. `src/completion/mod.rs` declares `domain` (6.1a),
`application` (6.2a) and `infrastructure` (6.2e); `domain/mod.rs` declares `completion_error`, `entities`,
`repositories`, `services`, `#[cfg(test)] test_fixtures` and `value_objects`.

| File (under `src/completion/`) | Content | From (`ff2db5a`) | Step | priv |
|---|---|---|---|---|
| D `completion_error.rs` | `CompletionError { Aborted, NoSuggestions(String) }`, `Debug`, `From<SpecError>` with explicit arms; no Display | `parser.rs:88-104` | 6.3a | (enum) |
| D `test_fixtures.rs` (`#[cfg(test)]`) | `pr_for`, `opts`, `sub`, `opt`, `arg`, `names`, `ctx` | `suggest.rs:1442-1512` | 6.2d | |
| D `entities/completion_session.rs` | aggregate (Q25) + `FIRST_TOKEN_TTL` | `Engine` fields 331-356 and their methods | 6.4e | 6.4e |
| D `entities/generator_slot.rs` | `GeneratorSlot`: `new`, `key`, `result`, `is_loading`, `generation`, `fire`, `finish -> bool` | `Slot` 273-279 | 6.4e | 6.4e |
| D `entities/generator_run.rs` | `GeneratorRun` (accessors, `wait_on`, `store_result_as`, `take_store`, `background_copy`); transitions returning `GeneratorCommand` | `Run` 292-308; pure halves of 811-1020 | 6.4e; 6.4g | 6.4e |
| D `entities/generator_cache.rs` | `GeneratorCache<V>` (hand-written `impl<V> Default`) + private `CacheEntry<V>` | `suggest.rs:1107-1177` | 6.4a | 6.4a |
| D `entities/parse_prefix_cache.rs` | `ParsePrefixCache` + `MAX_CACHED` (with its `ponytail:`) | `ParseCaches` 589-599 | 6.3d | 6.3d |
| D `entities/suggestion_recency.rs` | `SuggestionRecency` (Q10) | `Recency` 78, `record` 1208-1213 | 6.4b | 6.4b |
| D `repositories/recency_repository.rs` | `RecencyRepository { load, save, flush }` + `#[cfg(test)] InMemoryRecencyRepository` | | 6.4b | |
| D `value_objects/token.rs` | `Token` + `new`, accessors, `with_raw` | `shell_parser.rs:8-18` | 6.1b | 6.1c |
| D `value_objects/command_line.rs` | `CommandLine` (← `CommandInfo`) | `shell_parser.rs:20-29` | 6.1b | 6.1c |
| D `value_objects/alias_table.rs` | `type AliasTable = HashMap<String, String>` + `parse_aliases` | `shell_parser.rs:63-81` | 6.1b | (alias) |
| D `value_objects/insertion.rs` | `Insertion` + `InsertionParts`, `with_execute` | `types.rs:11-23` | 6.2a | 6.2b |
| D `value_objects/suggestion.rs` | `Suggestion` + `SuggestionParts`, `title`, `first_name` | `types.rs:28-67` | 6.2a | 6.2b |
| D `value_objects/generator_key.rs` | `GeneratorKey`, `for_arg`, `from_raw`, `argument_key`, `identity_hash` | `types.rs:26`, `engine.rs:230-240, 565-568` | 6.2a | 6.2a |
| D `value_objects/argument_hint.rs` | `ArgumentHint` | `types.rs:69-73` | 6.2a | 6.2b |
| D `value_objects/completion_phase.rs` | `CompletionPhase` | `types.rs:75-81` | 6.2a | (enum) |
| D `value_objects/completion_result.rs` | `CompletionResult` + `CompletionResultParts`, `error`, `take_suggestions`, `with_ghost` | `types.rs:84-132` | 6.2a | 6.2b |
| D `value_objects/annotation_kind.rs` | `AnnotationKind` | `parser.rs:36-44` | 6.3a | (enum) |
| D `value_objects/token_annotation.rs` | `TokenAnnotation` | `parser.rs:46-54` | 6.3a | 6.3b |
| D `value_objects/parsed_command_line.rs` | `ParsedCommandLine` + `ParsedCommandLineParts` (17 fields; accessors return `&Rc`/`&Option<Rc>`, PAR-T27) | `parser.rs:56-86` | 6.3a | 6.3b |
| D `value_objects/suggestion_query.rs` | `SuggestionQuery` + `after_sep` | `suggest.rs:24-57` | 6.2c | (enum) |
| D `value_objects/suggestion_candidate.rs` | `SuggestionCandidate` (`first`/`from_sub`/`from_opt` `pub(crate)`) | `suggest.rs:59-176` | 6.2c | 6.2f |
| D `value_objects/filter_options.rs` | `FilterOptions` (`Default` + `with_*`) | `suggest.rs:459-467` | 6.2c | 6.2f |
| D `value_objects/insertion_context.rs` | `InsertionContext<'a>` (`new(search_term, raw, shell)` + three `with_*`) | `suggest.rs:740-748` | 6.2c | 6.2f |
| D `value_objects/generator_slot_view.rs` | `GeneratorSlotView<'a>` (`key: GeneratorKey`) | `suggest.rs:192-199` | 6.2c | 6.2f |
| D `value_objects/cache_lookup.rs` | `CacheLookup<V> { Fresh, Stale { value, refresh }, Miss, Shared(TaskId) }` | `suggest.rs:1094-1105` | 6.4a | (enum) |
| D `value_objects/generator_step.rs` | `GeneratorStep` | `engine.rs:281-289` | 6.4e | (enum) |
| D `value_objects/generator_outcome.rs` | `GeneratorOutcome { Pending, Done, Fail }` | `engine.rs:310-314` | 6.4e | (enum) |
| D `value_objects/served_request.rs` | `ServedRequest` + `ServedRequestParts` | `Current` 317-329 | 6.4e | 6.4e |
| D `value_objects/generator_command.rs` | `GeneratorCommand` (Q8) | new | 6.4g | (enum) |
| D `services/command_line_tokenizer.rs` (+ `command_line_tokenizer/tests.rs`) | `current_command`, `substitute_token`; private AST unchanged | `shell_parser.rs` whole | 6.1a | |
| D `services/argv_parser.rs` | `ArgvState` (private fields + PAR3 API), private `ArgState`, `update`, `update_final`, `result` | `parser.rs:222-587` | 6.3c | |
| D `services/fuzzy_matcher.rs` | `fuzzysort` | `suggest.rs:469-555` | 6.2d | |
| D `services/suggestion_filter.rs` | `filter`, `partial`, `prefix_score`, `auto_exec`, `cmp_locale` | `suggest.rs:178-183, 557-694` | 6.2d | |
| D `services/suggestion_ranker.rs` | `update_priorities`, `dedupe`, `rank`; private `same_row` | `suggest.rs:437-457, 696-735` | 6.2d | |
| D `services/insertion_composer.rs` | `insertion`; private escapes, `At`, `unquote`, … | `suggest.rs:750-989` | 6.2d | |
| D `services/suggestion_finalizer.rs` | `to_item`; private `args_hint`, `highlight` | `suggest.rs:991-1064` | 6.2d | |
| D `services/suggestion_assembler.rs` | `assemble`; private `cmp_first`, `static_options`, `short_name`, `option_chain` | `suggest.rs:185-187, 201-351` | 6.2d | |
| D `services/history_suggestions.rs` | `history_rows`, `mix_history` | `suggest.rs:356-432` | 6.2d | |
| D `services/path_template.rs` | `shell_expand`, `inserted_directory`, `help_items` | `suggest.rs:1182-1275, 1326-1353` | 6.2d | |
| D `services/first_token_suggestions.rs` | `first_token_items(specs, aliases, executables: &[String])`, private `is_root_spec` | `suggest.rs:1358-1365, 1397-1431` minus the scan | 6.2e | |
| D `services/generator_trigger_policy.rs` | `triggered`, `dir_changed`, `cache_key`, `is_path_template`, `script_timeout` (f64), `SYNC_CALL`, extracted `effective_debounce`, `run_budget` | `suggest.rs:1069-1092`; `engine.rs:73, 242-246, 263-265, 745-748` | 6.4a | |
| D `services/completion_request_policy.rs` | `is_mid_word`, `uses_fuzzy_search`, `history_prefix`, `env_var` | `engine.rs:248-260, 531-535, 573-580, 603-606` | 6.5b | |
| D `services/completion_result_builder.rs` | `build_completion_result` (Q24) | `reply` minus 1159 | 6.5b | |
| A `completion_request.rs` | DTO (Q4) | `types.rs:134-144` | 6.2a | |
| A `completion_notice.rs` | DTO (Q11) | new | 6.3d (+ variants 6.4f, 6.5b) | |
| A `parse_context.rs` | `ParseContext<'a> { cwd, settings, env, shell, index }` | `ParseCtx` 111-118 | 6.3e | |
| A `command_line_parser.rs` (+ `command_line_parser/tests.rs` ← `git mv src/parser/tests.rs`) | `CommandLineParser { specs, runtime }`: `new`, `parse`, `clear_caches`; private `ParsePass`, `TIMEOUT`, `MAX_DEPTH`, `dev_mode`, `generator_context`, `no` | written in `parser.rs` in 6.3d | 6.3e | |
| A `generator_runner.rs` | Q8 | `engine.rs:670-1078` | 6.4f | |
| A `apply_session_context_use_case.rs`, `toggle_fuzzy_search_use_case.rs`, `reset_generators_use_case.rs`, `record_suggestion_insertion_use_case.rs`, `apply_completion_settings_use_case.rs` | Q26 | `handle`'s arms | 6.5a | |
| A `complete_command_line_use_case.rs` | Q26 | `compute`, `error`, `queries`, first-token path | 6.5b | |
| A `progress_generators_use_case.rs` | Q26 | `run`'s idle branch + `pump`'s reply | 6.5c | |
| A `test_fixtures.rs` (`#[cfg(test)]`) | the use-case harness (Q18) | new | 6.5b (+ 6.5c) | |
| A `ports/directory_lister.rs` | `trait DirectoryLister { fn list(&self, dir: &Path, files: bool, is_dangerous: bool) -> Vec<SpecSuggestion>; }` + `#[cfg(test)] InMemoryDirectoryLister` | new | 6.2e | |
| A `ports/executable_catalog.rs` | `trait ExecutableCatalog { fn executables(&self, path: Option<&str>, pathext: Option<&str>) -> Vec<String>; }` + `#[cfg(test)] FixedExecutableCatalog` | new | 6.2e | |
| I `completion_module.rs` | Q26 | `Engine::new`'s wiring | 6.5c | |
| I `adapters/fs_directory_lister.rs` | `FsDirectoryLister` (unit) + `list_dir` verbatim | `suggest.rs:1278-1324` | 6.2e | |
| I `adapters/path_executable_catalog.rs` | `PathExecutableCatalog` (unit): the `split_paths` loop, private `executables`, both `exe_name` cfg twins verbatim | `suggest.rs:1367-1395` + the scan of 1422-1429 | 6.2e | |
| I `adapters/state_file_recency_repository.rs` | Q10 | `engine.rs:75, 78, 195-228, 1214-1245` | 6.4b | |

Outside completion:

| File | Content | Step |
|---|---|---|
| `bootstrap/engine_server.rs` | `git mv src/engine.rs`: `spawn` (fixed doc), `EngineServer { replies, session, completion, record_command, history, ghost_cache }`, `run`, `handle`, `send_reply`, `log_notice`, the legacy ghost block (Q14), the 6.0a suite, the D6 half | 6.6b |
| `bootstrap/engine_message.rs` | `EngineMessage` (← `types::EngineMsg`) | 6.6b |
| `bootstrap/engine_inbox.rs` | `EngineInbox` + `impl RuntimeEventSource` + 2 tests, unchanged | 6.6b |
| `bootstrap/channel_completion_engine.rs` | Q12 | 6.6a |
| `bootstrap/mod.rs` | + the four children | 6.6a, 6.6b |
| `terminal_session/application/ports/{mod.rs, completion_engine.rs}` (+ `application/mod.rs`) | Q12 | 6.6a |
| `specs/application/ports/spec_runtime.rs` | `read`, `read_exec_stdout`; scripted `InMemorySpecRuntime` | 6.4c |
| `specs/infrastructure/adapters/quickjs_spec_runtime.rs` | the two methods | 6.4c |
| `specs/domain/value_objects/exec_request.rs` | `with_timeout(self, Option<u64>) -> Self` | 6.4c |
| `specs/domain/repositories/spec_repository.rs` | `InMemorySpecRepository::forgets()` | 6.4c |
| `specs/infrastructure/adapters/quickjs_spec_repository.rs` (tests) | repository half of the `spec_identity_…` split | 6.3d |
| `shared/application/ports/clock.rs` | `FixedClock` `Mutex` + `#[cfg(test)] advance` | 6.4c |
| `types.rs` (legacy) | loses seven rows (6.2a) and `EngineMsg` (6.6b); keeps `Msg` (`Completion(Box<CompletionResult>)`), `now_ms`, `now_secs`, `Model` | 6.2a, 6.6b |
| `pty.rs`, `cli.rs` (legacy) | accessor reads (6.2b); the starter, the field, the nine calls (6.6a) | |
| `menu.rs`, `render.rs`, `icons.rs`, `inline.rs` (legacy) | paths (6.2a), accessors (6.2b) | |
| `main.rs` | `+ mod completion;` `- mod shell_parser;` (6.1a); `- mod parser;` (6.3e); `- mod suggest;` (6.4a); `- mod engine;` (6.6b) | |
| `architecture_tests.rs` | `"completion"` in `REGISTERED_MODULES` (`LEGACY_MODULES` untouched, CON-T27) | 6.7 |

Engine-server dispatch (final shape, `bootstrap/engine_server.rs`):

| `EngineMessage` | Calls, in order | Reply |
|---|---|---|
| `Complete(r)` (latest after the drain, ENG-E1/E2) | `CompleteCommandLineUseCase` → `send_reply` (legacy `local_ghost_text`) | `Msg::Completion` unless `None` (interrupted) |
| idle, `session.has_running_generators()` | `ProgressGeneratorsUseCase` → `send_reply` | `Msg::Completion(Generators)` only if `Some` |
| `CommandFinished(e)` | `record_command.execute(e)` → legacy `clear_ghost_text_cache` → `ResetGeneratorsUseCase` | none |
| `CommandEnded` | legacy `clear_ghost_text_cache` → `ResetGeneratorsUseCase` | none |
| `RecordInsert { cmd, name }` | `RecordSuggestionInsertionUseCase` | none |
| `Llm { req, buffer, cwd, shell }` | legacy `request_llm_ghost_text` (thread `llm`) | `Msg::Ghost` |
| `Settings(s)` | `ApplyCompletionSettingsUseCase` | none |
| `Session { env, alias }` | `ApplySessionContextUseCase` | none |
| `ToggleFuzzy` | `ToggleFuzzySearchUseCase` | none |
| `Runtime(_)` | no-op (unreachable: the inbox hands events to the runtime) | none |
| `Shutdown` | loop ends; a `Complete` drained with it gets no reply (ENG-E3) → `completion.recency.flush()` | none |

`run` blocks only when nothing is pending and `!session.has_running_generators()`.

## Deviations from the plan's letter (record them in plan section 9 at 6.7, as the "Phase 6 notes")

1. **Order.** Tokenizer, then the wire rows (plan step 5's result VOs) moved and made private in run 1, then the whole
   parser, then the pipeline, then the generators, the use cases and the server. The pty port switch (plan step 6)
   runs at the end of run 3, before the engine carve. Twelve engine-loop characterization tests (6.0a) precede any
   engine edit (Q1, Q12).
2. **Deletions.** Legacy files leave with their last item, not in plan step 7: `shell_parser.rs` 6.1a, `parser.rs` +
   `src/parser/` 6.3e, `suggest.rs` 6.4a; `engine.rs` is `git mv`'d to `bootstrap/engine_server.rs` (6.6b). The
   `Parser { parser, cache }` facade lived in 6.3d-6.3e only (Q21, Q27).
3. **`AliasTable`** is a type alias of the std map (Q28).
4. **`GeneratorKey`** is a newtype with `for_arg` (+ `#[cfg(test)] from_raw`); the argument key
   (`generator_key::argument_key`) shares its hasher, so section 7 overrides section 4's "policy ← … arg key" line
   (Q3).
5. **`CompletionRequest`** keeps `cwd: PathBuf` and `shell: Shell` (not primitives). The session stores a
   `ServedRequest` (a new VO), never the DTO (Q4, Q5).
6. **Parts.** The rows, `ParsedCommandLine` and `ServedRequest` are built from `…Parts` structs: no positional
   constructors (clippy `too_many_arguments`) (Q2).
7. **`CompletionError`** has no Display; only its derived Debug is printed (PAR-T1, Q28).
8. **`ArgvState`** lives in `argv_parser.rs`.
9. **`ParsePrefixCache`** is owned by `CompletionSession` and passed `&mut`;
   `CommandLineParser::clear_caches(&self, cache)` (Q6).
10. **`disableForCommands`** stays in `SpecRepository::find` (Phase 5 deviation 3; already written into plan Phase 6
    step 3 and section 4 by 5.5; Q23).
11. **`GeneratorRunner`**, an application service `Rc`-shared by the two generator use cases, executes the
    `GeneratorCommand`s (the plan says "the use case"). `GeneratorCommand` keeps the Phase 5 hand-off payloads (Q8).
12. **`SpecRuntime`** gains `read` and `read_exec_stdout` only: no `resolve_script`/`ScriptValue` (amends Phase 5
    deviation 9 and its hand-off) (Q9).
13. **`GeneratorCache<V>`** is generic; production uses `SpecValue`, the moved test `Value` (Q9, Q20).
14. **Recency.** `SuggestionRecency` sits in `CompletionSession`; `RecencyRepository { load, save, flush }`. The
    adapter keeps the pending copy, the throttle thread and the newest-wins merge with the file: section 4's "Recency +
    merge" stays in the adapter (Q10).
15. **D5** goes through one `CompletionNotice` callback (`&mut dyn FnMut`), called at the old moments (Q11).
16. **Ghost on every reply.** The server adds the local ghost to every reply (Parse, Generators, error); the event
    table's idle and `Complete` rows are amended; `error()`'s ghost flag is gone (Q15).
17. **Legacy ghost code.** Until Phase 7.6 the legacy ghost/LLM code is one `impl EngineServer` block of
    `engine_server.rs`; there is no `legacy_ghost_text.rs` (Q14).
18. **Bootstrap files.** `engine_server.rs`, `engine_message.rs`, `engine_inbox.rs` (one type per file; section 4 and
    the Phase 5 hand-off put the message and the inbox in `engine_server.rs`) and `channel_completion_engine.rs` (Q13).
19. **`CompletionEngineStarter`.** The pty receives a starter (`fn(Sender<Msg>, Settings) -> Rc<dyn
    CompletionEngine>`), called where `engine::spawn` was, so no legacy → bootstrap edge appears in production (Q12).
20. **`RecordingCompletionEngine`** forwards `CompletionEngineCall`s, named after the port methods, into a channel
    (Q12).
21. **Files beyond section 4**: domain `annotation_kind.rs`, `cache_lookup.rs`, `generator_step.rs`,
    `generator_outcome.rs`, `generator_command.rs`, `generator_slot_view.rs`, `served_request.rs`,
    `services/completion_result_builder.rs` (a domain service, Q24); application `completion_notice.rs`,
    `parse_context.rs`, `generator_runner.rs`, `test_fixtures.rs`; bootstrap `engine_message.rs`, `engine_inbox.rs`.
22. **Placements** (Q28): `after_sep` in `suggestion_query.rs`; `same_row` private in the ranker; `cmp_first`
    private in the assembler; `exe_name` and the PATH split in `PathExecutableCatalog`, `first_token_items` takes the
    executables; `SYNC_CALL` in the trigger policy, `SLICE` in the runner, `FIRST_TOKEN_TTL` in the session; escapes not
    merged into `ShellQuoting`.
23. **Test fixtures.** `domain/test_fixtures.rs` exists from 6.2d with the pipeline helpers;
    `application/test_fixtures.rs` holds the use-case harness (Q16).
24. **Use-case tests.** Lifecycle and request-pipeline tests live at the use-case seam over InMemory doubles;
    `GeneratorRun` has pure transition tests; there is no runner-level suite (Q18).
25. **D6.** Splits happen only across modules: `spec_identity_survives_dev_reload_and_cache_clear` and
    `session_update_refreshes_first_token_items`. `cache_ttl_swr_max_age` stays whole (Q20).
26. **Session tests.** The five Phase 4 tests keep their bodies; their helpers keep the engine-era names
    (`engine_with_wrapper_env`, `js`) (Q19, O2).
27. **Test doubles outside completion:** `FixedClock::advance`, the scripted `InMemorySpecRuntime` (+
    `applied_settings`; sleeps finish only on the opt-in `finishing_sleeps()`, a 6.4c correction of the design),
    `InMemorySpecRepository::forgets()` (Q17). They land before their first consumer (the Phase 5 deviation 16
    precedent).
28. **Module wiring.** `CompletionModule` exposes the recency repository (the `HistoryModule` precedent). Bootstrap
    loads it before the env snapshot and flushes it after the loop. There is no `LoadSpecIndexUseCase` (Q26).
29. **`Parser::load_spec`** was deleted with its last caller in 6.3d, not in Phase 11 (Q27).
30. **`types::now_ms`** has no engine caller left; it stays until Phase 11.
31. **`spawn`'s doc** no longer claims the thread ends when the senders drop (Q13, M5).
32. **Use-case construction.** Unit-struct use cases are never built with `::default()` (clippy).
33. **The parser test fixture keeps the name `Mock`** (and its `specs`/`fns`/`loads` fields), now over the InMemory
    doubles, in `completion/application/command_line_parser/tests.rs`, so the parser test bodies that name it stay unchanged; testing-standard's
    no-mocks vocabulary is not applied to its name (PAR11, 6.3d). `no()` stays a private fn of
    `command_line_parser.rs` (PAR18), and `ParsedCommandLine` keeps the accessor `completion()` (PAR5).
34. **Two 5 s `TIMEOUT`s.** The parser's `TIMEOUT` and `dev_mode` move privately into `command_line_parser.rs` and stay
    separate from the repository's copies; both `script_timeout`s and both generator-context builders stay (D1, Q7).
35. **Trigger policy.** `effective_debounce` and `run_budget` are extracted free functions of
    `generator_trigger_policy.rs` (the plan says "debounce/budget"), called where `update_slots` computed them (6.4a).
36. **Pipeline choices** (6.2c-6.2e): `DirectoryLister` is synchronous and `list_dir` moves whole into
    `FsDirectoryLister` (SUG9); `first_token_sources` stays whole in `path_executable_catalog.rs`'s tests, the catalog's
    output fed to the domain merge (SUG10); `empty_search_term_is_no_prefix_match` lives in `suggestion_filter.rs` until
    Phase 7.3 (SUG11); suggest.rs's function names are kept, only types take glossary names (SUG13).
37. **`CompletionSession` is a field-access aggregate** (Q25): it holds all engine state behind about 37 moved
    field-access methods with no new logic, while the generator orchestration (when to fire, slots for a new argument,
    the superseded check, run order, the cache protocol in `fetch`) sits in the application `GeneratorRunner`, which
    drives the session through fine-grained accessors. `generator_cache_mut()` is the one deliberate escape hatch
    (it hands out internal state). This keeps the moves verbatim; it is not the intended end design (hand-off,
    Phase 11).
38. **History.** Run 4 left two 73-character subjects, not rewritten: `0d1ea43` (6.4a) and `87dc339` (6.4c).
    Later steps commit through a script that refuses a subject over 72 characters.

## Hand-off notes for later phases

- **Phase 7 (ghost_text) inherits exactly this.**
  - One block of `impl EngineServer` in `src/bootstrap/engine_server.rs`, marked `// legacy ghost text: Phase 7.6
    replaces this block`, holding `local_ghost_text(&self, result: &CompletionResult) -> Option<String>`,
    `request_llm_ghost_text(&mut self, req, buffer, cwd, shell)` (thread `llm`, warn `engine: llm worker: {e}`, no reply
    on spawn failure), `clear_ghost_text_cache(&self)`, the private `llm_history(settings, entries)` and the field
    `ghost_cache: Arc<Mutex<inline::GhostCache>>`. The test `llm_history_honours_history_count` sits in
    `engine_server.rs`'s tests. There is **no** `legacy_ghost_text.rs`.
  - Other `crate::inline` users at the end of Phase 6: `pty.rs` `llm_enabled` (902-908), `doctor.rs`, `cli.rs`, and the
    test `empty_search_term_is_no_prefix_match` in `completion/domain/services/suggestion_filter.rs` (calls
    `crate::inline::spec_ghost`).
  - Dispatch hooks: `send_reply(result)` is the one place a `CompletionResult` is sent and calls `local_ghost_text`;
    the CommandFinished/CommandEnded arm calls `record_command.execute` (Finished only) → `clear_ghost_text_cache` →
    `ResetGeneratorsUseCase`; the `Llm` arm calls `request_llm_ghost_text`. `EngineMessage` is in
    `bootstrap/engine_message.rs`, `EngineInbox` in `bootstrap/engine_inbox.rs`.
  - Ghost recompute: the use cases return `ghost: None`; `send_reply` sets the local ghost for every result. 7.6
    replaces `local_ghost_text`'s body with `SuggestLocalGhostTextUseCase::execute(settings, result.buffer(),
    result.cursor(), Some(result.suggestions()))`, builds `GhostTextModule` on the engine thread right after
    `HistoryModule`, replaces the other two touchpoints and the field, moves the `llm_history` test and deletes
    `inline.rs`. The event table is already amended (6.7).
  - What completion offers: `Suggestion::{kind(), insert(), prefix_match()}`; `Insertion::{delete(), text(),
    cursor_back()}`; `CompletionResult::{buffer(), cursor(), suggestions(), is_error(), with_ghost(..)}`;
    `CompletionSession::{settings(), env()}`; `inline.rs` already reads `Suggestion` through accessors; `completion`
    is registered.
  - Step 7.0 creates `inline-diff.sh` (Q30). 7.3 moves `empty_search_term_is_no_prefix_match` and imports
    `opts`/`ctx` from `crate::completion::domain::test_fixtures`. TC2: `request_bodies`/`response_parsing` stay whole.
    C24 per Q29; M1, M2 → 7.1; M6 → 7.4; M7 → 7.5; M8, M9 → 7.7. `ghost_key` stays in `pty.rs`; `cd_target` is split.
  - `ghost_text` is registered at the end of Phase 7; `inline` stays in `LEGACY_MODULES`.
- **Phase 8 (dropdown).** `completion/domain/test_fixtures.rs` exists; `comp`/`item` join it, built through
  `CompletionResultParts`/`SuggestionParts`; `settings(json)` can join only if each of its items carries
  `#[cfg(test)]` (CON-T28). The rows are already private: the menu uses `take_suggestions()`; `with_execute()` is
  applied on the clone under `menu.rs:361-364`'s condition; `Effect` still derives `PartialEq` over `Insertion`;
  `GeneratorKey::from_raw` is test-only. `render::describe` goes to a dropdown domain service (Phase 5 P3).
- **Phase 9 (terminal_session).** `terminal_session::CompletionEngine` + `RecordingCompletionEngine`/
  `CompletionEngineCall` (channel-backed) are the port and double the session use cases take as `Rc<dyn
  CompletionEngine>`; the port file already passes the local fitness check. `CompletionEngineStarter` (`pty.rs`)
  dissolves into the module wiring, but the engine must still start at today's moment (`pty.rs:461`, inside
  `catch_unwind`, ENG-E30); `debug_shell_command` receives the starter from bootstrap. The reply path is still
  `Sender<types::Msg>` (`Completion(Box<CompletionResult>)`, `Ghost`); `Msg` moves to `session_message.rs`;
  `ApplyCompletionResultUseCase` reads the accessors and `take_suggestions`; `ApplyLlmGhostTextUseCase` replaces the
  `Msg::Ghost` arm.
- **Later cleanup (Phase 11 or a dedicated step).** `CompletionSession` is a state bag on purpose (deviation 37):
  group the slots, the runs and the generator cache behind one domain entity with intention-revealing methods, move
  the generator rules the runner holds into it, and retire `generator_cache_mut()`. Behaviour-preserving, under the
  6.5c lifecycle suite and the 6.0a suite.
- **Phase 11.** `types.rs` keeps `Msg`, `now_ms`, `now_secs`, `Model`; `now_ms` has no engine caller. `LEGACY_MODULES`
  keeps every deleted name.

## Glossary additions (proposed for `docs/ubiquitous-language.md`; 6.7 writes them)

| Term | Kind | Meaning | Replaces |
|---|---|---|---|
| `CommandLineParser` | Application service | Parses a `CommandLine` against the specs found through `SpecRepository` and the spec functions run through `SpecRuntime`, using and filling the session's `ParsePrefixCache`; reports a failed generateSpec as a notice. | `parser::Parser`, `Host`, `JsHost`, `ParseCaches::parse*` |
| `ParseContext` | Application input | What one parse needs from the request: cwd, settings, env, shell, the `SpecIndex`. | `ParseCtx` |
| `ArgvState` | VO | Where the argv state machine stands after each token: the spec level, inherited persistent options, the argument being filled, what was passed. | `parser::State` + `ArgState` |
| `CompletionError` | Domain error | Aborted (a newer request is waiting) or no suggestions (with the reason). Printed only through Debug. | `parser::ParseError` |
| `CompletionNotice` | DTO | Something the completion code tells its caller to log, at the moment it happens: a failed generateSpec, a failed generator, no suggestions. | inline `logger` calls |
| `GeneratorSlotView` | VO | A generator slot as the assembler sees it: key, rows, loading, and the query its rows match. | `suggest::GenView` |
| `GeneratorOutcome` | VO | Where a generator run ended this time: still pending, done with rows, or failed with a text. | `engine::Outcome` |
| `GeneratorCommand` | VO | What a generator run asks for next: start a spec function call or a process (each with its generator context), sleep, or settle with an outcome. | the start calls inside `engine::{begin, advance}` |
| `GeneratorRunner` | Application service | Starts, feeds and settles generator runs through the spec runtime, the directory lister and the clock, for both generator use cases. | `Engine::{update_slots, begin, fetch, advance, pump}` |
| `GeneratorTriggerPolicy` | Domain service | When a generator fires again (triggers, a changed directory, a debounce of 200 ms for 0), how long a run may take, the script-timeout floor, and the generator cache key. | `suggest::{triggered, dir_changed, cache_key}`, engine consts |
| `CompletionRequestPolicy` | Domain service | Request-level rules: the mid-word guard, whether fuzzy search applies, the history prefix, the env lookup (case-insensitive on Windows). | parts of `Engine::compute` |
| `CompletionResultBuilder` | Domain service | Builds the reply for a served request: assembled, mixed with history, ranked by recency, filtered and turned into rows. | `Engine::reply` |
| `ServedRequest` | VO | The request the session is serving: its id, buffer, shell, clamped cursor, parsed command line, argument key, fuzzy flag, queries, triggered generators, first-token candidates and history prefix. | `engine::Current` |
| `RecencyRepository` | Repository | Loads the suggestion recency, saves it (at most once a second, merged newest-wins with the file) and flushes a pending save at shutdown. | `load_recency`/`save_recency` + the engine's save thread |
| `DirectoryLister` | Port | Lists a directory for a path template: sorted, `../` last, control-char names dropped. | `suggest::list_dir` |
| `ExecutableCatalog` | Port | The executables on `PATH`, directory by directory in `PATH` order, each sorted. | `suggest::executables` |
| `CompletionEngine` (terminal_session) | Port | What the terminal session asks of completion: complete, a finished or ended command, an inserted suggestion, an LLM ghost text, settings, the session context, the fuzzy toggle, shut down. Never fails. | `Sender<EngineMsg>` in `pty.rs` |

Bootstrap names in prose (no row of their own): `EngineServer`, `EngineMessage`, `EngineInbox`,
`ChannelCompletionEngine`.

Rows to edit:
- `AliasTable`: a type alias of the std map.
- `GeneratorKey`: a newtype; `for_arg`; the argument key shares its hash.
- `CompletionRequest`: shell (not a name), cwd as a path.
- `CompletionSession`: + settings, the served request, the parse-prefix cache, the recency, the first-token cache.
- `ParsePrefixCache`: cleared wholesale past 1000 entries; keeps the first-token spec.
- `GeneratorRun` / `GeneratorStep`: replaces `engine::{Run, Step}` only (`GeneratorOutcome` replaces `engine::Outcome`);
  a run returns `GeneratorCommand`s.
- `host / pump` (already rewritten by 5.5): add `GeneratorRunner` next to `ProgressGeneratorsUseCase`.
- `cache` (already names the first-token cache in `CompletionSession`): add the parse-prefix cache's home.

## Test moves

"path" means testlock's loose form shows no change; "CHANGED" is a restructuring (same scenario, same assertions;
the step that edits it); "split" is a D6 split across modules. Expected totals against `phase6-start`: **66 CHANGED,
0 MISSING, 0 DUPLICATE, about 90 NEW**. Recount with `testlock --diff` at every run end.

### `shell_parser.rs` (16) → `completion/domain/services/command_line_tokenizer/tests.rs` (6.1a)
- path: `spec_table`, `operators_without_spaces`, `assignments`, `substitutions_and_nesting`, `statement_structure`,
  `multibyte_and_no_panics`, `alias_expansion_cases`, `parse_aliases_formats` (imports `parse_aliases` from 6.1b).
- CHANGED: `parse_tree_fixtures` (6.1a, the eight include paths); `unterminated_quotes`, `escapes`, `concatenation`,
  `token_offsets`, `aliases_expansion`, `substitute`, `pwsh` (6.1c, accessors).
- **8 CHANGED.**

### `suggest.rs` (23)

| Test | Destination | Kind |
|---|---|---|
| `fuzzysort_vectors` | `fuzzy_matcher.rs` | path (6.2d) |
| `prefix_ladder` | `suggestion_filter.rs` | path |
| `bucket_order_and_auto_execute` | `suggestion_filter.rs` | CHANGED 6.2c, 6.2f |
| `partial_sorted_by_score_then_priority` | `suggestion_filter.rs` | CHANGED 6.2f |
| `empty_search_term_is_no_prefix_match` | `suggestion_filter.rs` (Phase 7 → `spec_ghost_text.rs`) | CHANGED 6.2a (`Vec<Suggestion>`) |
| `directory_and_current_token_rows` | `suggestion_filter.rs` | CHANGED 6.2c, 6.2f |
| `dedupe_only_up_to_50` | `suggestion_ranker.rs` | CHANGED 6.2c, 6.2f |
| `recency_bands_and_priority_zero` | `suggestion_ranker.rs` | CHANGED 6.2f |
| `triggers` | `generator_trigger_policy.rs` | path (6.4a) |
| `cache_ttl_swr_max_age` | `generator_cache.rs`, whole | path (6.4a: testlock's loose form strips `GenCache::`/`Lookup::`; CHANGED only if a turbofish is needed) |
| `assembly_options_rules` | `suggestion_assembler.rs` | CHANGED 6.3b, 6.2f |
| `option_chain_rewriting` | `suggestion_assembler.rs` | CHANGED 6.3a, 6.3b, 6.2f |
| `history_rows_priority_and_mixing` | `history_suggestions.rs` | CHANGED 6.2f |
| `history_rows_skip_multiline_entries` | `history_suggestions.rs` | path |
| `insertion_examples`, `insertion_review_regressions`, `insertion_round2_regressions` | `insertion_composer.rs` | CHANGED 6.2b (`InsertionParts`), 6.2c, 6.2f |
| `item_highlight_and_hint` | `suggestion_finalizer.rs` | CHANGED 6.2b, 6.2c, 6.2f |
| `shell_expand_and_directory` | `path_template.rs` | CHANGED 6.2b (`i.delete()`, `i.text()`), 6.2f |
| `help_template` | `path_template.rs` | CHANGED 6.3a, 6.3b |
| `list_dir_sorted`, `list_dir_skips_control_char_names` (`cfg(unix)`) | `fs_directory_lister.rs` | path (6.2e) |
| `first_token_sources` (`cfg(unix)`) | `path_executable_catalog.rs` | CHANGED 6.2e (catalog output fed to the merge; same files, asserts, order), 6.2f |

**16 CHANGED.** Helpers → `completion/domain/test_fixtures.rs` (6.2d).

### `parser/tests.rs` (23 at `ff2db5a`) → `completion/application/command_line_parser/tests.rs` (`git mv`, 6.3e)
- Left in Phase 5: `diff_merge`, `version_pick` (5.4a), `cli_version_detection_survives_an_aborted_parse` (5.4b).
- Body unchanged: `relative_path_first_token_loads_local_spec`, `options_can_break_variadic`,
  `load_spec_cycle_is_an_error`.
- CHANGED by 6.3a (renames) and/or 6.3b (accessors): `parse_arguments_table`, `result_fields`,
  `exceptions_and_first_token`, `single_token_paths`, `load_spec_and_is_module`, `recursive_load_spec`,
  `is_command_recursion`, `sudo_git_command_index`, `generate_spec_merge`, `persistent_options_inherit_without_mutation`,
  `is_repeatable`, `inline_and_function_load_specs`, `git_alias_directive`,
  `composite_option_value_raw_is_the_value_only`, `aborts_propagate`.
- CHANGED by 6.3a/6.3b and 6.3d (`parse(&mut cache, ..)`): `generate_spec_cache_key` (514), `prefix_cache` (634),
  `cached_nested_state_keeps_this_commands_prefix` (673), `first_token_spec_identity_is_stable` (710).
- Split (D6, 6.3d): `spec_identity_survives_dev_reload_and_cache_clear` keeps its name in `quickjs_spec_repository.rs`'s
  tests (finds through the repository, `forget_loaded_specs`, the same assertions on the specs, over the existing
  `repository()`/`write_mk`/`find_mk`/`MK` helpers); `spec_identity_survives_dev_reload_and_cache_clear_first_token`
  (the `first` `Rc` kept through `CommandLineParser::clear_caches`) goes to the parser tests.
- Helpers: `Mock` keeps its name and its `loads`/`fns` fields over the InMemory doubles, `fns` typed by the new
  `Functions` (6.3d commit 1, Q27); `parse(cache, ..)`; `cmd()` over `Token::new`/`CommandLine::new`.
- **20 CHANGED.**

### `engine.rs` (12)

| Test | Destination | Kind |
|---|---|---|
| `recency_round_trip_keeps_other_keys`, `recency_save_merges_other_sessions` | `state_file_recency_repository.rs` (6.4b) | CHANGED (`Recency` → `RecencyRecord`) |
| `llm_history_honours_history_count` | `bootstrap/engine_server.rs` (6.6b `git mv`); Phase 7.6 → ghost_text | path |
| `session_update_refreshes_first_token_items` | split 6.5a: the name → `apply_session_context_use_case.rs`; `…_keeps_the_engine_running` → `engine.rs` → `engine_server.rs` | CHANGED 6.2f, 6.4f, 6.5a (not 6.3e, recheck) |
| `should_use_the_env_of_a_session_context_in_place_of_the_wrapper_env`, `should_fall_back_to_the_wrapper_env_after_a_session_context_with_an_empty_env`, `should_parse_the_aliases_of_a_session_context_for_the_shell`, `should_find_no_aliases_after_a_session_context_with_an_empty_alias_text`, `should_find_no_aliases_after_a_session_context_without_alias_text` | `apply_session_context_use_case.rs` (6.5a) | path (bodies byte-identical; helpers change) |
| `wait_aborts_and_defers_on_complete` | `bootstrap/engine_inbox.rs` (6.6b) | CHANGED 6.2a (`CompletionRequest {`) |
| `should_deliver_a_runtime_event_read_by_the_engine_loop_to_the_runtime` | `bootstrap/engine_inbox.rs` (6.6b) | path |
| `generator_keys_differ_per_index` | `generator_key.rs` (6.2a) | CHANGED (`for_arg`) |

**5 CHANGED.** The helpers `engine_with_wrapper_env`, `report_session_context`, `wrapper_env`, `parser_over` and
`runtime_with_inbox` change with the signatures they call (the test-lock lens diffs them); `parser_over` stays with
the engine tests until 6.5c replaces it with `spec_repository_over` (Q19). 6.5a adds `engine_over` and
`session_context` for the D6 half.

### Consumer files (tests stay in their files)

| File | Tests | Kind |
|---|---|---|
| `menu.rs` | `full_insertion_reopens_only_for_new_arg_or_retrigger` | CHANGED 6.2a, 6.2b (TC4) |
| `menu.rs` | `common_prefix` | CHANGED 6.2a, 6.2b: the five `Insertion { delete, text, .. }` patterns → accessor guards with the same values |
| `menu.rs` | `large_change_counts_skipped_requests` | CHANGED 6.2a, 6.2b |
| `menu.rs` | `esc_hides_for_the_line`, `error_reset_and_loading`, `sticky_selection_across_recompute`, `execute_and_record` | CHANGED 6.2b |
| `menu.rs` | the others | none (helpers `item`, `comp` change) |
| `render.rs` | `round_trip_arg_hint_spinner_and_wide_ghost`, `row_contents` | CHANGED 6.2b |
| `render.rs` | the other 9 | none |
| `icons.rs` | `resolves_like_q`, `tmux_uses_placeholders` | CHANGED 6.2a, 6.2b |
| `pty.rs` | `disabled_menu_lets_tab_through`, `prompt_redraw_does_not_reshow_a_hidden_menu` | CHANGED 6.2b |
| `pty.rs` | `enter_records_the_submitted_line`, `no_history_command_still_ends` | CHANGED 6.6a (one line each) |
| `pty.rs` | the five context tests, `llm_ghost_requested_for_commands_without_spec`, `conpty_is_closed_off_the_main_loop` | none / path |
| `pty.rs` | `ghost_keys_and_words`, `ghost_with_control_chars_is_never_shown_or_accepted` | untouched (Phase 7) |
| `inline.rs` | `spec_ghost_rules` | CHANGED 6.2b (`cursor_back` through a `#[cfg(test)]` builder, same value) |
| `cli.rs` | `debug_shell_survives_closed_terminal` | CHANGED 6.6a (starter argument; the `--exact` string unchanged) |
| `cli.rs` | `inline_show_customizations` | untouched (Phase 7) |

**17 CHANGED** (menu 7, render 2, icons 2, pty 4, inline 1, cli 1).

**Total:** 8 + 16 + 20 + 5 + 17 = **66 CHANGED**, each a restructuring (paths, glossary renames,
constructors/accessors, helper-built scenarios, the two D6 splits). MISSING 0.

### New tests (one owner each; `grep -rn "fn <name>(" src` empty before adding)

| Step | File | Tests |
|---|---|---|
| 6.0a | `engine.rs` → `engine_server.rs` | `should_reply_again_with_the_split_lines_of_a_script_generator`, `should_reply_again_with_the_rows_a_post_process_function_returns`, `should_reply_again_with_the_filtered_rows_of_a_templated_custom_generator`, `should_serve_a_fresh_cached_custom_result_in_the_parse_reply`, `should_settle_a_generator_with_bad_script_input_with_no_rows`, `should_keep_the_rows_of_a_superseded_debounced_run_out_of_the_slot`, `should_serve_only_the_latest_request_after_draining_the_inbox`, `should_apply_settings_queued_after_a_request_before_serving_it`, `should_drop_a_request_that_arrives_with_shutdown`, `should_add_the_local_ghost_text_to_a_generators_reply`, `should_add_a_history_ghost_text_to_the_error_reply_of_a_command_without_a_spec`, `should_reply_an_error_without_ghost_text_for_a_cursor_inside_a_word` (characterization: pass with no production change) |
| 6.2e | ports, `first_token_suggestions.rs` | `should_list_the_entries_it_was_given_for_a_directory`, `should_list_nothing_for_a_directory_it_was_not_given`, `should_offer_the_executables_it_was_given`, `should_count_every_scan_of_the_path`; characterization `should_offer_spec_names_then_aliases_then_executables_each_name_once` |
| 6.3d | `parse_prefix_cache.rs`, parser tests | `should_forget_every_prefix_state_before_storing_one_past_the_limit`, `should_forget_every_generated_spec_before_storing_one_past_the_limit`, `should_report_a_failed_generate_spec_with_its_debug_text`, `should_report_a_failed_generate_spec_even_when_the_parse_then_fails`; split half `spec_identity_survives_dev_reload_and_cache_clear_first_token` |
| 6.6a | port, adapter | `should_record_every_call_in_the_order_it_was_made`, `should_send_each_call_as_its_engine_message`, `should_ignore_an_engine_that_has_stopped` |
| 6.4a | `generator_trigger_policy.rs` | `should_debounce_a_zero_debounce_for_200_ms`, `should_budget_a_run_for_its_debounce_twice_its_script_timeout_and_a_second`, `should_read_the_script_timeout_floor_in_whole_milliseconds` (sets the value: the default is `cfg!(windows)`) |
| 6.4b | recency entity, repository, adapter | `should_record_the_insertion_time_under_the_command_and_name`, `should_not_record_an_auto_execute_row`, `should_answer_no_times_for_an_unknown_command`, `should_load_the_recency_it_saved`, `should_write_nothing_on_flush_without_a_pending_save`, `should_write_the_latest_recency_on_flush_after_a_save`, `should_keep_a_session_without_a_state_file_in_memory` |
| 6.4c | specs + shared | `should_read_a_kept_value_like_a_fresh_result_of_its_kind`, `should_read_the_stdout_of_an_exec_result`, `should_read_no_stdout_from_a_value_without_a_text_stdout`, `should_decode_a_read_like_the_runtime`, `should_finish_a_started_call_with_its_scripted_outcome_once`, `should_finish_a_sleep_with_null`, `should_interrupt_every_wait_when_asked_to`, `should_record_the_settings_it_was_given`, `should_count_the_times_it_forgot_the_loaded_specs`, `should_replace_only_the_timeout`, `should_move_its_instant_forward_when_advanced`, `should_finish_an_exec_with_its_scripted_outcome` (drives `completing_execs` test-first; not in the 6.4c brief) |
| 6.4e | slot, session | `should_ignore_the_result_of_an_older_generation`, `should_keep_first_token_suggestions_for_ten_seconds`, `should_reset_the_fuzzy_search_to_the_setting_for_a_new_argument` |
| 6.5a | the five use cases | `should_flip_the_fuzzy_search_of_the_current_argument`, `should_forget_the_served_request_and_the_generator_results`, `should_record_when_a_suggestion_was_inserted_for_its_command`, `should_not_save_the_recency_after_an_auto_execute_row`, `should_hand_the_new_settings_to_the_spec_runtime`, `should_forget_the_loaded_specs_when_settings_change`, `should_drop_first_token_items_and_generator_results_when_settings_change`, `should_keep_the_fuzzy_toggle_and_the_served_request_when_settings_change`; D6 half `session_update_refreshes_first_token_items_keeps_the_engine_running` |
| 6.5b | policy, use case | `should_guard_a_cursor_inside_a_word`, `should_never_guard_a_cursor_at_the_end_of_the_buffer`, `should_reply_with_the_subcommands_of_the_spec_for_the_typed_word`, `should_reply_an_error_for_a_cursor_inside_a_word`, `should_reply_an_error_and_report_no_suggestions_for_a_command_without_a_spec`, `should_reply_nothing_when_a_newer_request_interrupts_the_parse`, `should_start_the_generators_of_a_new_argument_and_report_them_loading`, `should_keep_the_toggled_fuzzy_search_until_the_argument_changes`, `should_reply_first_token_suggestions_for_a_single_word`, `should_scan_the_path_only_on_a_first_token_cache_miss`, `should_rank_recently_inserted_suggestions_first` |
| 6.5c | `progress_generators_use_case.rs` | `should_reply_again_with_the_rows_of_a_finished_generator`, `should_not_reply_while_no_generator_of_the_current_argument_changed`, `should_reply_nothing_and_keep_the_runs_when_a_newer_request_interrupts`, `should_wait_for_the_debounce_before_running_a_generator`, `should_serve_a_fresh_cached_result_without_starting_a_task`, `should_serve_a_stale_result_and_refresh_it_in_the_background`, `should_share_the_task_of_an_identical_fetch_in_flight`, `should_keep_a_failed_background_refresh_silent`, `should_cache_the_value_of_a_superseded_run`, `should_fail_an_expired_run_with_timed_out` (`FixedClock::advance`), `should_report_every_failed_generator`, `should_run_a_script_with_the_largest_timeout_of_floor_generator_and_request` |
| 6.4g | `generator_run.rs` | `should_list_templates_then_help_rows`, `should_filter_template_rows_through_filter_template_suggestions`, `should_split_trimmed_stdout_into_untrimmed_rows`, `should_post_process_stdout_when_no_split_is_set`, `should_settle_with_no_rows_without_split_or_post_process`, `should_filter_a_templated_custom_result_with_its_raw_value`, `should_settle_an_untemplated_custom_result_with_its_rows`, `should_fail_bad_script_input_with_its_serde_text`, `should_raise_the_exec_timeout_to_the_largest_of_floor_generator_and_request` |

At `ff2db5a` none of these names exists (recheck R11: the only hits are the six existing tests the tables above name).

**6.0a's harness** (test code in `engine.rs` only; recheck §5; the 6.0a brief gives the exact code):
- `serve(settings, history: Vec<HistoryEntry>, first: Vec<EngineMsg>, grace, react) -> Vec<Reply>` builds the engine
  itself; there is no separate engine-building helper. It creates a real `EngineInbox::new(receiver, sender)`,
  `Rc::new(QuickJsSpecRuntime::new(inbox.clone(), &settings))` (an `Rc`: `runtime_with_inbox` returns a bare runtime
  while `parser_over` takes `&Rc<QuickJsSpecRuntime>`), and `Engine::with(replies, settings, parser_over(&runtime),
  SpecIndex::default(), HistoryModule::new(Rc::new(InMemoryHistoryRepository::new(history))), None)`;
- a Local spec `<tmp>/.fig/autocomplete/build/tool.js` (`spec_dir(tag, arg)` writes `tool_spec(arg)` into a
  `TempDir`) reached as `./tool …` with cwd = tmp
  (`quickjs_spec_repository.rs:177-196` `spec_file`; `spec_locator.rs:7-31` gives `Local { dir: <cwd>/, name: "tool" }`,
  pinned by `relative_path_first_token_loads_local_spec`). Two limits: the buffer holds **at least two tokens**
  (`./tool ` or `./tool x`; a single token holding `/` loads the Global `dotslash` spec from the real bundle,
  `parser.rs:613-630`), and the spec's default export is an **object**, not a function (a function default runs the
  CLI-version probe, which starts `tool --version` as a real process, `quickjs_spec_repository.rs:108-110, 213-219`);
- `serve` sends `first`, then runs `Engine::run` on the test thread (QuickJS is `!Send`; `run` drains the inbox before
  computing, 434-439, which is what makes "latest only", "settings before the queued request" and "no reply with
  Shutdown" observable); a watcher thread owns the reply `Receiver<Msg>`, a cloned `Sender<EngineMsg>` and `react`:
  each reply becomes `observe(&c)`, and `react(&replies)` answers `Then::Wait`, `Then::Deliver(messages)` or
  `Then::Stop` (`stop_when(done)` builds the common reaction); after `Stop` it collects for `grace` and then sends
  `Shutdown`, and after 10 s in all it sends `Shutdown` too, so a regression fails instead of hanging;
- generators use only the native `ls -1ApL` (`process_executor.rs:29`) and the `fig`/`tabsmith` short-circuit (38,
  `{stdout: "", stderr: "", status: 1}`), never a real process;
- the history ghost on an error reply needs `inline.enabled` (default true), `cursor == buffer.len()` and an entry in
  the `history` the test passes to `serve` (`ran("./missing --flag")`, held by the InMemory repository); for "a command
  without a spec" use a Local `./missing ` (no file → NoSuggestions), because `parser_over` resolves Global names
  against the machine's real bundle (`data_directory_from_env()`, 1394);
- `state_path = None`, so the end-of-loop flush writes nothing;
- the helpers are edited at every engine signature change, the twelve bodies never. 6.5c rebuilds `serve`'s engine
  over `build_completion_module` + `Engine::new` and keeps the InMemory history over the caller's `history`.

## Step briefs

The full prompts are in `phase-6-completion.args.json` (ids below). Each implementer reads `agent-brief.md`, then
this file and the source maps named in its prompt. Every step ends with:
- the full gate green and everything committed, one commit per sub-step or TDD phase (Q33);
- `testlock.py . ef807c1 --diff` (MISSING 0, DUPLICATE 0) and `testlock.py . phase6-start --diff` (MISSING 0,
  DUPLICATE 0, every CHANGED test listed for this or an earlier step);
- the local fitness run (completion registered, from 6.6a also terminal_session) on a copy of the committed tree;
- the `all_bundle_modules` release run where Q33 lists the step, and the engine-step repeat runs where Q33 lists them;
- block diffs of every moved item against `git show phase6-start:<file>`;
- no rewritten history, subjects ≤ 72 characters, and a lone `integrations_zsh_powerlevel10k_prompt_wrapped` failure
  rerun alone before the gate is run again.

Each step compiles on its own; the only intra-phase shim is 6.3d's `Parser { parser, cache }` facade, gone in 6.3e.
Every run ends gate-green with no shim alive.

**Run 1: guards, tokenizer, wire rows.** Ends with the engine characterized, the tokenizer done, `types.rs` holding only
`Msg`, `EngineMsg`, the clocks and `Model`, and the result rows private.

| Id | Title | Plan step | What it does |
|---|---|---|---|
| 6.0a | Characterize the engine loop at its inbox | 5 (guard) | step 0: `phase6-start` exists and `src/` equals it; 12 test-only characterization tests in `engine.rs` over a real inbox, runtime, repository and `Engine::with` (harness above); run 20× with no production change; `test(completion): …` |
| 6.1a | Move the tokenizer into the completion domain | 1 | 2 commits: `git mv src/shell_parser.rs src/completion/domain/services/command_line_tokenizer.rs`, `pub` → `pub(crate)`, `completion/{mod.rs, domain/mod.rs, domain/services/mod.rs}`, `main.rs` (`+ mod completion;`, `- mod shell_parser;`), importers `engine.rs:5, 22, 513, 541`, `parser.rs:5, 218`, `parser/tests.rs:5`; then the tests to `command_line_tokenizer/tests.rs` with the eight `include_str!` paths (P6-TOK-22) |
| 6.1b | Token, CommandLine, AliasTable value objects | 1 | `token.rs`, `command_line.rs` (`CommandInfo` → `CommandLine`), `alias_table.rs` (`type AliasTable` + `parse_aliases`) verbatim with `pub(crate)` fields; importers engine/parser/parser tests; the tests' own `HashMap` import |
| 6.1c | Private tokenizer VOs; explicit `Kind` arms | 1 | 2 commits: D8 on `P::inner`'s `_ => (0, 0)` (the 15 named kinds); private fields, `new`, accessors, `Token::with_raw`, `substitute_token` rebuilt through them, `engine.rs:611` → `cmd.original_text().to_string()` |
| 6.2a | The wire rows leave `types.rs` | 5 | 7 commits, one per type (Q2, Q3, Q4): `Insertion`, `Suggestion`, `GeneratorKey` (newtype; `hash2`/`gen_key` deleted; `argument_key`, `for_arg`; `generator_keys_differ_per_index` moves), `ArgumentHint`, `CompletionPhase`, `CompletionResult`, `CompletionRequest` (`application/{mod.rs, completion_request.rs}`); `Msg::Completion(Box<CompletionResult>)`, `EngineMsg::Complete(CompletionRequest)`; every importer (suggest, engine, menu, render, icons, inline, pty) in the same commit; evidence `grep -rn '\bGenKey\b\|types::.*\(Insertion\|Item\|ArgInfo\|Phase\|Completion\|Request\)\b' src` empty |
| 6.2b | The wire rows keep their fields private | 5 | 2 commits (`Suggestion` + `Insertion`; `CompletionResult` + `ArgumentHint`): `…Parts`, accessors, `with_execute`, `take_suggestions`, `with_ghost`, `ArgumentHint::new`, `#[cfg(test)]` `with_*`; production readers in menu, render, icons, inline, pty, suggest, engine; test helpers `item`, `comp`, `ins`, `reply`, `subs`, `menu`; P6-M3 inputs kept |

**Run 2: the parser.** Ends with `parser.rs` and `src/parser/` gone and `CommandLineParser` in the application layer.

| Id | Title | Plan step | What it does |
|---|---|---|---|
| 6.3a | The parse vocabulary moves into the domain | 3 | `completion_error.rs`, `annotation_kind.rs`, `token_annotation.rs`, `parsed_command_line.rs` verbatim and renamed, `pub(crate)` fields; `no()` stays in `parser.rs` returning `CompletionError`; importers `parser.rs`, parser tests, `suggest.rs` (+ tests, `pr_for`), `engine.rs` (`Current.pr`, `compute`'s arms, `queries`, `update_slots`) |
| 6.3b | The parse result keeps its fields private | 3 | `ParsedCommandLine::new(ParsedCommandLineParts { .. })` (`result()`'s literal keeps its field names), accessors returning `&Rc`/`&Option<Rc>`; `TokenAnnotation::new` + accessors; D8 on `result`'s `match a.parts.last()` (547); the arg key keeps `Rc::as_ptr(pr.completion())` |
| 6.3c | The argv state machine moves into the domain | 3 | `argv_parser.rs` ← `parser.rs:222-587` verbatim + the `ArgvState` API (`apply_inline_spec` keeps its `ponytail:`); `parser.rs`'s field writes call it; no test body changes |
| 6.3d | `CommandLineParser` replaces the `Host` trait | 3 | 3 commits (Q27): the test-only harness swap; TDD `parse_prefix_cache.rs`; `completion_notice.rs` + `CommandLineParser` with a private `ParsePass` in `parser.rs`, `clear_caches`, `Host`/`JsHost`/`ParseCaches` deleted, the `Parser { parser, cache }` facade (logs the notice at warn), D8 on `apply_load_spec`'s `_ => Ok(None)` (862), `Parser::load_spec` deleted, the `spec_identity_…` split |
| 6.3e | The parser moves to the application layer; `parser.rs` goes | 3 | `command_line_parser.rs` (+ `git mv src/parser/tests.rs …/command_line_parser/tests.rs`), `parse_context.rs`; engine fields `command_line_parser`, `parse_prefix_cache`; `compute` passes `&mut self.parse_prefix_cache` and `&mut \|n\| log_completion_notice(n)` (a free fn in `engine.rs`, explicit arms); the Settings arm calls `clear_caches(&mut self.parse_prefix_cache)` at the old position; spawn builds `CommandLineParser::new(repository, runtime.clone())`; deletes `parser.rs`, `src/parser/`, `mod parser;`; repoints `architecture_tests.rs:371` and `settings_catalog.rs:388` comments; evidence `grep -rn 'crate::parser\b\|mod parser;' src` empty; `all_bundle_modules` |

**Run 3: the suggestion pipeline and the pty port.** Ends with the pipeline, its ports, doubles and fs adapters in
completion, `suggest.rs` holding only the generator group, and the pty talking to `CompletionEngine`.

| Id | Title | Plan step | What it does |
|---|---|---|---|
| 6.2c | Pipeline value objects | 2 | `suggestion_query.rs` (+ `after_sep`), `suggestion_candidate.rs`, `filter_options.rs`, `insertion_context.rs`, `generator_slot_view.rs` (`key: GeneratorKey`) verbatim, `pub(crate)` fields; importers `suggest.rs`, `engine.rs` (49 and every `Query::`/`GenView`/`FilterOpts`/`InsCtx`/`Sugg` site) |
| 6.2d | Pure services and the domain test fixtures | 2 | one commit per service: `test_fixtures.rs` + `fuzzy_matcher.rs`, then `suggestion_filter`, `suggestion_ranker`, `insertion_composer` (P6-M3; escapes not merged), `suggestion_finalizer`, `suggestion_assembler`, `history_suggestions`, `path_template`; their tests move; `empty_search_term_is_no_prefix_match` → `suggestion_filter.rs` (still calling `crate::inline::spec_ghost`, test code); importers `engine.rs` (`reply`, `update_slots` 730, `begin` 818) and what is left of `suggest.rs` |
| 6.2e | Ports with their doubles, the fs adapters, the first-token merge | 2 | TDD `directory_lister.rs` + `InMemoryDirectoryLister`, `executable_catalog.rs` + `FixedExecutableCatalog` (`scans()`); move `fs_directory_lister.rs`, `path_executable_catalog.rs` (both `exe_name` cfg twins verbatim; `#[cfg(unix)]` on the test's `PermissionsExt` import), `first_token_suggestions.rs`; `completion/infrastructure/` created; engine: `begin` → `FsDirectoryLister.list(..)` (synchronous, same Parse reply), `Engine::first_token_items` keeps its signature and scans through `PathExecutableCatalog` only on a miss; the characterization test |
| 6.2f | Pipeline value objects keep their fields private | 2 | accessors, `with_*`, the four `set_*` for the two in-place loops (`update_priorities`, `option_chain`), `FilterOptions::default()` + `with_*`, `InsertionContext::new(..)` + `with_*`; test-only accessors `#[cfg(test)]`; `session_update_refreshes_first_token_items`'s `s.names()[0]` |
| 6.6a | The `CompletionEngine` port; the pty switches to it | 6 | 3 commits (Q12): TDD port + `CompletionEngineCall` + `RecordingCompletionEngine` (+ `terminal_session/application/{mod.rs: ports, ports/mod.rs}`); TDD `bootstrap/channel_completion_engine.rs` over `Sender<types::EngineMsg>` + `start_completion_engine` over `crate::engine::spawn`; switch `pty.rs` (field, nine sites, the starter through `run`/`Session::start`), `cli::debug`/`debug_shell`, `bootstrap/cli.rs:139, 141`; evidence `grep -n 'EngineMsg\|engine::spawn' src/pty.rs src/cli.rs` empty; `debug_shell_survives_closed_terminal` alone; the windows test checked by hand |

**Run 4: generator vocabulary, recency, the typed port.** Ends with `suggest.rs` gone and the engine's generators on
the typed port.

| Id | Title | Plan step | What it does |
|---|---|---|---|
| 6.4a | Trigger policy and the generator cache; `suggest.rs` goes | 4 | move `triggered`, `dir_changed`, `cache_key`, `is_path_template`, `script_timeout`, `SYNC_CALL` verbatim; TDD `effective_debounce` (`0 → 200`) and `run_budget` (plain `+`/`*`, GT9) and switch `update_slots` to them; `cache_lookup.rs`, generic `generator_cache.rs`; the engine holds `GeneratorCache<Value>`; delete `suggest.rs` + `mod suggest;` |
| 6.4b | Recency | 4 | Q10, TDD: entity, repository + InMemory, adapter (moved private fns; `pending` before the swap); engine: `recency: SuggestionRecency`, the repository built in `Engine::with` from `state_path`, `record` → entity + `save`, the loop's end → `flush()`, `reply` ranks over `of_command`; `integrations-diff.sh` (state.json keys) |
| 6.4c | The typed read API and the scripted doubles | 4 | Q9 + Q17 in specs/shared files, TDD, no completion caller yet: `read`, `read_exec_stdout`, `ExecRequest::with_timeout`, scripted `InMemorySpecRuntime`, `InMemorySpecRepository::forgets()`, `FixedClock` `Mutex` + `advance`; `all_bundle_modules` |
| 6.4d | The engine's generators switch to the typed port, in place | 4 | alone, `engine.rs` only: `queries`/triggers through `call` (explicit `SpecError` arms); `begin`/`exec_output`/`custom_result` start typed calls (`FilterTemplateSuggestions { Listed \| Returned }`, `Script`, `Custom`, `PostProcess`) with `Some(&run.ctx)`; `fetch` returns `Result<SpecValue, Outcome>` over `GeneratorCache<SpecValue>`; `script_input` reads a `ScriptResolution` with Q9's timeout; `advance` takes a `TaskOutcome` and reads PostProcess/Filter results through `read`; `pump` uses the typed `wait` (`Err(Interrupted) => return`); production imports of `ExecInput`, `fig_spec_mapper`, `JsArg`, `TaskResult`, `serde_json` go; P6-M4; 6.0a 20×; evidence `grep -n 'serde_json\|fig_spec_mapper\|JsArg\|call_json\|wait_json\|start_call' src/engine.rs` hits test code only; then the user smoke items 1-2 (O6) |

**Run 5: session, runner, small use cases, request pipeline.** Ends with `CompleteCommandLineUseCase` serving every
request and the engine adding the ghost to every reply.

| Id | Title | Plan step | What it does |
|---|---|---|---|
| 6.4e | Generator entities, `ServedRequest`, `CompletionSession` | 4 | 2 commits: `generator_slot.rs`, `generator_step.rs`, `generator_outcome.rs`, `generator_run.rs`; then `served_request.rs`, `completion_session.rs` (Q25) taking `ParsePrefixCache`, `SuggestionRecency`, `GeneratorCache<SpecValue>` and `Current` → `ServedRequest`; `Engine` keeps `tx`, `index`, `history`, `record_command`, `ghost_cache`, `command_line_parser`, `recency_repository`, `session`; a `settle` wrapper logs `engine: generator failed: {e}` right before `session.settle`; delegators `env`, `alias_map`, `first_token_items` keep the six session-related bodies; `engine_with_wrapper_env` builds the session |
| 6.4f | `GeneratorRunner` (verbatim move) | 4 | Q8; `CompletionNotice::GeneratorFailed`; the engine holds `Rc<GeneratorRunner>` built in `Engine::with` (which gains the runtime `Rc`) over `Rc::new(FsDirectoryLister)` and `dependencies.clock`; `compute` → `trigger`, `run` → `progress` + reply when `Some(true)` and a request is served; per-fn block diffs; 6.0a 20×; `all_bundle_modules` |
| 6.5a | The five small use cases | 5 | TDD each (Q26); `handle`'s arms call them in today's order (`handle` loses `js`; `Engine::with` holds `Rc<CommandLineParser>`); the five Phase 4 tests move with identical bodies; `session_update_refreshes_first_token_items` split (Q19) |
| 6.5b | `CompleteCommandLineUseCase`, the policy, the builder | 5 | TDD order: `completion_request_policy.rs`; `completion_result_builder.rs` (Q24, P6-M10); `CompletionNotice::NoSuggestions`; `application/test_fixtures.rs` (the harness); `complete_command_line_use_case.rs` from `compute`/`error`/`queries`/the first-token path (ENG-E14 order; the alias memo and `enter_argument` before anything can abort; two clock reads around the PATH scan); `run` → use case → `send_reply` (adds the ghost, Q15); `pump` builds its reply with the builder + `send_reply`; `reply`, `ghost`'s flag and `error` deleted from `engine.rs` |

**Run 6: lifecycle, commands, server, close.** Ends with `engine.rs` gone and `completion` registered.

| Id | Title | Plan step | What it does |
|---|---|---|---|
| 6.5c | `ProgressGeneratorsUseCase`, the lifecycle suite, the completion module | 5 | TDD the use case over the harness (12 lifecycle tests); `completion_module.rs`; `spawn` builds in Q26's order; `Engine { tx, session, completion, record_command, history, ghost_cache }` with `run` (blocking only when nothing is pending and `!session.has_running_generators()`), `handle` (the dispatch table), `send_reply`, `log_completion_notice`, the legacy ghost block; 6.0a 20×, the inbox tests 50×, `debug_shell_survives_closed_terminal`, `all_bundle_modules` |
| 6.4g | `GeneratorRun` returns `GeneratorCommand`s | 4 | under the lifecycle suite: `generator_command.rs`; `GeneratorRun` transitions (template list → filter or settle; script call/value → exec with the composed timeout; exec stdout → split / postProcess / settle; custom → filter or settle; rows → settle); the runner executes `StartCall`/`StartExec`/`StartSleep` (through `fetch` when cached) and `Settle`, keeping `begin`'s order (GT11-GT14) and the order of effects (GT15, GT19); 9 pure `GeneratorRun` tests; box a variant only if clippy fires |
| 6.6b | `bootstrap/engine_server.rs` | 6 | 3 commits (Q13); `channel_completion_engine.rs` switches to `EngineMessage` and `engine_server::spawn`; `bootstrap/mod.rs`; `main.rs` (`- mod engine;`); `types.rs` drops `EngineMsg`; leaves `architecture_tests.rs:374-375` and the `settings_catalog.rs` comments that name `engine.rs` to 6.7; inbox tests 50× by full path; `debug_shell_survives_closed_terminal`; `all_bundle_modules`; evidence `git diff -M --stat` shows a rename |
| 6.7 | Register `completion` and record the phase | 7 | add `"completion"` to `REGISTERED_MODULES`; the greps below; repoints `architecture_tests.rs:374-375` and the `settings_catalog.rs` comments that name `engine.rs` (371 and 388 are 6.3e's); plan §4 (completion and bootstrap as built, the event table), §5 (Phase 6 done; Phase 7 step 4 "replace the three legacy ghost touchpoints of `engine_server.rs`"), §9 "Phase 6 notes" (the deviations above), the glossary, README and handoff status, this file's status line; full gate; testlock vs `ef807c1` and `phase6-start`; `all_bundle_modules`; `integrations-diff.sh` and `precmd-diff.sh` empty; `cargo check --locked --no-default-features --all-targets`; `debug_shell_survives_closed_terminal` alone; the orchestrator then asks the user for the plan §8 smoke (items 1-8) |

**6.7's greps** (empty outside `#[cfg(test)]`; every `use crate::{` hit reviewed by hand):
- `crate::(types|engine|parser|suggest|pty|inline|menu|render|icons|doctor|cli|tracker|keys|shell_parser|js|spec)\b`
  over `src/completion` and `src/terminal_session/application/ports`;
- `crate::(ghost_text|dropdown|terminal_session|diagnostics|shell_integration|bootstrap)\b` over `src/completion`;
- `anyhow|logger` over `src/completion/{domain,application}`;
- `crate::engine|crate::parser|crate::suggest|crate::shell_parser|EngineMsg` over `src`;
- `ls src/shell_parser.rs src/parser.rs src/parser src/suggest.rs src/engine.rs` fails for all four (Q21).

## Open questions (for the plan owner; the default applies if nobody answers)

| # | Question | Default |
|---|---|---|
| O1 | Privatise the result rows in run 1 (6.2b) or after the use cases? | Run 1: later code is written once, against accessors. |
| O2 | Keep the five session tests' bodies byte-identical through helpers named `engine_with_wrapper_env`/`js`, or rename them (5 CHANGED)? | Keep (Phase 5 P12 precedent; the helper doc says why). |
| O3 | `EngineInbox` in its own file (one type per file) or private in `engine_server.rs` (the Phase 5 hand-off's letter)? | Its own file `bootstrap/engine_inbox.rs`. |
| O4 | Switch the pty to the port in run 3 or after the engine carve? | Run 3. |
| O5 | The newest-wins merge: adapter (verbatim) or a `SuggestionRecency` method (§4's letter)? | Adapter. |
| O6 | Ask the user for smoke items 1-2 right after 6.4d (the typed switch)? | Yes, once; the full plan §8 smoke after 6.7. |
| O7 | Phase 7 TC2: one shared module for `build_request`/`parse_response`, or one HTTP adapter for both providers? | Phase 7 decides; either keeps both tests whole. |
| O8 | Box a `GeneratorCommand` payload pre-emptively? | No; only if clippy's `large_enum_variant` fires. |

## Launch

From the repo root, at `ff2db5a` with a clean tree:
1. `docs/hexagonal-refactor/phase-6-completion.md` (this file) and `docs/hexagonal-refactor/phase-6-completion.args.json`
   (`phase`, `startRev`, `reviewNotes` — the binding reviewers' contract, prepared in
   `target/hexagonal-refactor/prep6/review-notes.md` — and the 28 `steps`) are untracked. Commit them as
   `docs: hand Phase 6 over with its research` (written with `git commit -F`, with the trailer), as `6f38da1` did for
   Phase 4. The step prompts point at both files, and 6.0a's step 0 stops on a dirty `git status`.
2. Tag that commit: `git tag phase6-start` (local, lightweight, never pushed). Its `src/` equals `ff2db5a`.
3. Run the workflow **once per run**, in order, each time with that run's steps only (in run order) and `startRev` =
   HEAD at that run's launch:

```js
Workflow({
  scriptPath: "docs/hexagonal-refactor/phase-workflow.js",
  args: {
    phase: "Phase 6 (completion), run N",
    startRev: /* HEAD when this run is launched */,
    steps: /* the args file's steps of run N: run 1 = 6.0a, 6.1a, 6.1b, 6.1c, 6.2a, 6.2b; run 2 = 6.3a, 6.3b, 6.3c,
              6.3d, 6.3e; run 3 = 6.2c, 6.2d, 6.2e, 6.2f, 6.6a; run 4 = 6.4a, 6.4b, 6.4c, 6.4d; run 5 = 6.4e, 6.4f,
              6.5a, 6.5b; run 6 = 6.5c, 6.4g, 6.6b, 6.7 */,
    reviewNotes: /* the orchestrator's binding "SINCE" preamble for run N, then the args file's reviewNotes */
  }
})
```

   The orchestrator writes the "SINCE" preamble before each run from the previous runs' results: what has landed
   (commits, files created/moved/deleted), the deviations taken beyond this file, the CHANGED/NEW totals against
   `phase6-start` so far, and any finding left open. It binds the run's implementers and reviewers like this file.
4. Between runs: the full gate is green, `testlock.py . phase6-start --diff` is recounted against the "Test moves"
   totals, and nothing from the run is left uncommitted. After run 4 (6.4d) the orchestrator asks the user for smoke
   items 1-2 (O6); after run 6 for the full plan §8 smoke.

