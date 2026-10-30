# Phase 7 (ghost_text): preparation

Status: **prepared, not started**; re-checked against `ff2db5a`, the end of Phase 5; re-check again at the end of
Phase 6 (the DEFERRED items of `target/hexagonal-refactor/prep7/recheck.md`, listed below under "Re-check at
`phase7-start`"). At `ff2db5a` the gate was green: 851 passed, 1 ignored (`all_bundle_modules`), clippy 3 warnings
(budget 3), golden identical, testlock MISSING 0 and DUPLICATE 0. Phase 6 (completion) runs between `ff2db5a` and
this phase; its end is this phase's start (`phase7-start`).

**How it was researched.** On a frozen snapshot of `c2f0825` (the end of Phase 5 run 3,
`target/hexagonal-refactor/prep6-tree`):
- one ghost_text researcher (`target/hexagonal-refactor/prep7/ghost-text.md`, traps **GHO-GT1…43**, proposals
  **GHO1…20**);
- the Phase 6 consumers researcher, whose "inline consumers" section is Phase 7's (`prep6/consumers.md`, traps
  **CON-T19…T27**, proposals **CON13…19**);
- the Phase 6 critic (`prep6/critic.md`, items **C22-C24**, **U3**, **TC1/TC2/TC10/TC11**, **X14/X15**, source-map
  traps **M1-M10**);
- the binding Phase 6 design (`prep6/decisions.md`, **P6-Q1…Q33**, hand-off **P6 §7.1**).

A designer merged these into the binding Phase 7 design (`target/hexagonal-refactor/prep7/decisions.md`: decisions
**R1-R24**, rechecks **K1-K16**, open questions **O1-O9**, steps **7.0-7.10** in three runs). A recheck read the design
against `ff2db5a` (`prep7/recheck.md`): every claim not depending on Phase 6 holds there except eight, which this file
folds in as **RC1-RC8** (the recheck's own C1-C8; renamed so they never clash with the critic's C22-C24). This file is
the merge: where it and the research differ, this file wins; where it and `prep7/decisions.md` differ, the RC
corrections win. The 11 step prompts (ids `7.0` … `7.10`, see "Step briefs") are in `phase-7-ghost-text.args.json`,
next to this file; the reviewers' contract is its `reviewNotes`.

Plan reference: [the plan](../hexagonal-refactor-plan.md).
- Section 5, "Phase 7: ghost_text" (four steps) and "Phase 6: completion" (it builds the engine server Phase 7
  switches).
- The `ghost_text`, `completion`, `bootstrap` and `terminal_session` layouts and the "Event → use case tables" in
  section 4.
- Sections 1 (Rust conventions), 2 (allowed dependencies: `ghost_text → shared, settings, history, specs,
  completion`), 7 (risks), 8 (verification, manual smoke item 4) and 9 (D1-D9 and the Phase 1-6 notes).

Glossary: [`docs/ubiquitous-language.md`](../ubiquitous-language.md), sections ghost_text, completion and "Words with
more than one meaning" (prompt, ghost, cache, profile). Source maps: [`source-map/inline.md`](source-map/inline.md)
(all), [`pty.md`](source-map/pty.md) (ghost parts), [`engine.md`](source-map/engine.md) (ghost and LLM),
[`cli.md`](source-map/cli.md) (inline command), [`doctor.md`](source-map/doctor.md) (Inline check),
[`settings.md`](source-map/settings.md) (inline keys), [`suggest.md`](source-map/suggest.md) and
[`types.md`](source-map/types.md) (ghost lines). Their numbers are at `af1b304`.

**Ids in this file.** Traps **P7-T1…P7-T56** (each names its research origin: `GHO-GT…`, `CON-T…`, `M…`); decisions
**R1-R24**; recheck corrections **RC1-RC8**; rechecks **K1-K16**; open questions **O1-O9**; steps **7.0-7.10**. Phase 6's
decisions are **P6-Q…**, its rechecks **P6-R…**. Never cite a bare `GT`, `T`, `R` or `C` from another file: prefix it
(P6-Q31, R2).

**Line numbers.** This file cites **`ff2db5a`**.
- `git diff c2f0825 ff2db5a` over `src/inline.rs src/pty.rs src/cli.rs src/doctor.rs src/bootstrap src/types.rs
  src/main.rs scripts` is empty: the research's numbers for these files hold at `ff2db5a`.
- `src/engine.rs` changed in 5.4b (parser wiring only); its ghost and LLM bodies are byte-identical, shifted by
  about 13 lines (map in "Where the code is today").
- **Phase 6 deletes `engine.rs`** (its ghost/LLM code becomes one legacy block of `bootstrap/engine_server.rs`) and
  edits `pty.rs`, `cli.rs`, `doctor.rs`, `types.rs` and `inline.rs` (accessor reads, `Suggestion` in place of
  `types::Item`, the `CompletionEngine` port). At `phase7-start` locate every site **by symbol or content**, never by
  number alone. Each Phase 7 step also moves code for the next one.

### What Phase 6 leaves for Phase 7 (assumed; the items marked [K…] are rechecked at `phase7-start`)
- `src/inline.rs` (legacy) still holds everything in "Where the code is today", reading completion's `Suggestion` and
  `Insertion` through accessors. Its test `spec_ghost_rules` was CHANGED by 6.2b (against `ef807c1`) and its helper
  `item()` builds a `Suggestion` (P6-Q2) **[K9]**.
- `src/bootstrap/engine_server.rs` holds **one** `impl EngineServer` block marked
  `// legacy ghost text: Phase 7.6 replaces this block` (P6-Q14) with:
  - `local_ghost_text(&self, result: &CompletionResult) -> Option<String>`;
  - `request_llm_ghost_text(&mut self, req, buffer, cwd, shell)` (thread `llm`, warn `engine: llm worker: {e}`, no
    reply on spawn failure);
  - `clear_ghost_text_cache(&self)`;
  - the private `llm_history(settings, entries)` and the field `ghost_cache: Arc<Mutex<inline::GhostCache>>`.
  `send_reply` calls `local_ghost_text` for **every** result it sends (Parse, Generators, error; P6-Q15). The test
  `llm_history_honours_history_count` sits in its tests. There is no `legacy_ghost_text.rs` **[K1]**.
  `EngineMessage` lives in `bootstrap/engine_message.rs`, `EngineInbox` in `bootstrap/engine_inbox.rs` (P6 §7.1).
- 6.0a's twelve engine-loop tests live in `engine_server.rs`. Three pin the local ghost:
  `should_add_the_local_ghost_text_to_a_generators_reply`,
  `should_add_a_history_ghost_text_to_the_error_reply_of_a_command_without_a_spec`,
  `should_reply_an_error_without_ghost_text_for_a_cursor_inside_a_word` **[K6]**.
- `empty_search_term_is_no_prefix_match` lives in `completion/domain/services/suggestion_filter.rs`, calls
  `crate::inline::spec_ghost` and imports `opts`/`ctx` from `completion/domain/test_fixtures.rs` (P6-Q16) **[K5]**.
- Completion offers `Suggestion::{kind(), insert(), prefix_match()}`, `Insertion::{delete(), text(), cursor_back()}`,
  `CompletionResult::{buffer(), cursor(), suggestions(), is_error(), with_ghost(..)}` and
  `CompletionSession::{settings(), env()}` **[K2, K3, K8]**.
- Other `crate::inline` users: `pty.rs` `llm_enabled`, `doctor.rs` `inline_outcome`, `cli.rs`'s inline section.
- `completion` is registered; `inline` stays in `LEGACY_MODULES` (CON-T27). Clippy budget 3, all used **[K11, K12]**.
- The plan's event table is already amended by 6.7 (idle row: `ProgressGenerators → SuggestLocalGhostText → reply if
  changed`; error replies get the local ghost too) **[K10]**.

### Re-check at `phase7-start` (the DEFERRED items; baseline facts at `ff2db5a`)
Before launching run 1, the orchestrator re-reads these against the end of Phase 6 and corrects the step briefs where
they differ.

| K | Claim | Step | Fact at `ff2db5a` (baseline) |
|---|---|---|---|
| K1 | The legacy block's names and signatures, its marker comment, `send_reply` as the one place results are sent | 7.2-7.7 | No `engine_server.rs`. Legacy names: `Engine::ghost(&self, req, cursor, Option<&[Item]>)` (`engine.rs:1187-1202`), `llm(&mut self, req, buffer, cwd, shell)` (1247-1275), the clear in `handle` (463-465), `fn llm_history` (267-270), field `ghost_cache` (345, 408). No marker. |
| K2 | `CompletionResult::{buffer(), cursor(), suggestions(), ghost()}` (`cursor()` = the clamped cursor of every result kind) and how a test builds a result with suggestions (`CompletionResultParts` + `Default`, or `error()` + a `#[cfg(test)]` setter); `SuggestionParts` | 7.6 | `types::Completion` pub fields `buffer`, `cursor`, `items`, `ghost` (`types.rs:90-105`); `Completion::error(req, buffer, cursor)` (`types.rs:112`). |
| K3 | `Suggestion::{kind(), insert(), prefix_match()}`, `Insertion::{delete(), text(), cursor_back()}` | 7.3 | `spec_ghost` reads `Item.kind`, `insert.{text, delete, cursor_back}`, `prefix_match` as pub fields (`inline.rs:145-152`). |
| K4 | `pty.rs` `on_completion`'s post-6 ghost line, the `Msg::Ghost` arm, `self.engine.request_llm_ghost_text(..)` | 7.1 | `pty.rs:881-883`, `682-690`; the LLM send is `self.engine.send(EngineMsg::Llm{..})` (1236-1247), which 6.6a turns into a port call. |
| K5 | `empty_search_term_is_no_prefix_match`'s home and imports (`opts`, `ctx`, `filter`, `to_item`, `SuggestionCandidate`) | 7.3 | `suggest.rs:1602-1620`, using `filter`, `to_item`, `opts()`, `ctx(..)`, `Sugg::new`, `Item`, `crate::inline::spec_ghost` (1616). |
| K6 | The 6.0a harness (`serve(messages, until)`, the watcher's 10 s cap, how settings are injected) and whether the watcher already collects `Msg::Ghost` | 7.0 | No harness exists. |
| K7 | `spawn`'s build order (P6-Q26) and `EngineServer`'s fields; drop `history` if nothing reads it after 7.7 | 7.6, 7.7 | The history module is built in `Engine::new` on the engine thread (`build_history_module(dependencies)`); `Engine.history` at `engine.rs:336`. |
| K8 | `CompletionSession::{settings(), env()}` return `&Settings` and `&HashMap<String, String>` (empty session env ⇒ wrapper env) | 7.6, 7.7 | `self.settings` replaced only by `EngineMsg::Settings` (`engine.rs:476-482`); `env()` (502-508). |
| K9 | `inline.rs`'s post-6 imports and the `item()` helper of `spec_ghost_rules` | 7.2, 7.3 | `inline.rs:11` `use crate::types::Item;`, test 509 `use crate::types::Insertion;`, `item()` (798-821) a 15-field `Item` literal. |
| K10 | 6.7's plan edits (Phase 7 step wording, event table rows, section 9 numbering) so 7.10 edits in place | 7.10 | Event table at plan:495-500 (`Llm`: "on a worker thread, as today"); section 4 ghost_text at plan:377-395 lists `openai_llm_client`, `anthropic_llm_client`; Phase 7 steps at plan:658-669. |
| K11 | `REGISTERED_MODULES` (+ `completion`) and `LEGACY_MODULES` unchanged | 7.1-7.10 | `REGISTERED_MODULES` = history, settings, shared, shell_integration, specs (`architecture_tests.rs:9`); `LEGACY_MODULES` holds `"inline"` (15-33). |
| K12 | Gate numbers and clippy (3) at `phase7-start`; testlock counts against it | all | 851 passed, 1 ignored, clippy 3. |
| K13 | No new name of "Test moves" collides with a Phase 6 name (`grep -rn "fn <name>(" src`) | all | All 42 names: 0 definitions. |
| K14 | `cli.rs`'s inline section and `doctor.rs`'s `inline_outcome` untouched by Phase 6 except paths | 7.2, 7.9 | Inline section `cli.rs:199-367`; `inline_outcome`/`is_local` `doctor.rs:612-636`. P6 edits `cli.rs` for the debug starter and accessor reads (6.2b, 6.6a). |
| K15 | `bootstrap/cli.rs`'s `Inline` arm still calls `cli::inline(c)`; `cli::settings_repository()` still `JsonFileSettingsRepository::new(data_directory_from_env())` | 7.9 | `bootstrap/cli.rs:49` `Inline(cli::InlineCmd)`, `:127` `cli::inline(c)`; `cli.rs:79-82`. 6.6a edits `bootstrap/cli.rs` (debug starter). |
| K16 | `HistoryEntry` is `Send` | 7.7 | Confirmed, final: fields `String`, `Option<i32>`, `u64`, `Option<u64>`, `Option<String>` (`history_entry.rs:3-14`); already moved into the `llm` thread (`engine.rs:1260-1266`). |

Also re-check RC8: Phase 6 may already rewrite the "engine.rs" words of the settings-catalog test comments.

## Where the code is today (`ff2db5a`)

| What | Where | Used by | Goes to (step) |
|---|---|---|---|
| `Provider` (`Clone, Copy, Debug, PartialEq, Eq`), `Profile` (7 pub fields, `Clone, Debug, PartialEq`), `Source` (`History, Specs, HistoryAndSpecs, Profile`), `BUILTIN_SOURCES`, `PROFILES_PREFIX`, `profiles`, `source`, private `parse_profile` | `inline.rs:18-123` | `cli.rs:3, 223, 256-261, 267, 282, 292, 296, 324, 333, 346`, test helper 687-697; `doctor.rs:614, 620-628`; `pty.rs:902-908`; `engine.rs:8, 1197-1201, 1249` | `llm_provider.rs`, `llm_profile.rs`, `ghost_text_source.rs`, `ghost_text_source_resolver.rs` (7.2) |
| `selected_source`, `provider_name` | `cli.rs:252-254, 256-261` | `cli::inline`, `choose_source`, `render_sources` | resolver; `LlmProvider::name` (7.2) |
| `history_ghost`, `spec_ghost(&[Item])` | `inline.rs:125-155` | `engine.rs:1195-1196`; test `suggest.rs:1616` | `history_ghost_text.rs`, `spec_ghost_text.rs` (7.3) |
| `GhostCache` + `clear/insert/lookup` (private `entries`, `ponytail:` comment at 174) | `inline.rs:157-186` | `engine.rs:8, 345, 408, 463-465, 1253, 1262-1268` | `ghost_text_cache.rs` as `GhostTextCache` (7.3) |
| `llm_history(entries, count)`, `SYSTEM`, `PROMPT_MAX_BYTES`, `PRIVATE_WORDS`, `is_private`, `usable`, private `history_prompt` | `inline.rs:188-200, 252-307` | `engine.rs:268-269` (wrapper), `llm_complete` | `llm_prompt_builder.rs` (7.4) |
| user-text composition in `llm_complete` | `inline.rs:212-218` | | `llm_prompt_builder::llm_prompt` → `LlmPrompt` (7.4) |
| private `clean` | `inline.rs:369-397` | `llm_complete` | `llm_completion_cleaner.rs` (7.4) |
| `validate` (target part 401-412, fs calls 414, 417), private `split_words`, `expand` (fallback home 471-475) | `inline.rs:399-504` | `llm_complete` (249) | `cd_target_validator::cd_target` (7.4); `validate` over `DirectoryProbe` (7.5), into `llm_ghost_text_job.rs` (7.7) |
| `llm_complete` (key lookup 219-224, HTTP 226-246, parse + clean + validate 247-249) | `inline.rs:202-250` | `engine.rs:1266` (worker) | `HttpLlmClient::complete` (7.5); `LlmGhostTextJob::run` (7.7) |
| private `build_request`, `parse_response` | `inline.rs:309-367` | `llm_complete` | `http_llm_client.rs` (7.5) |
| inline tests (12) and helpers `entry` 514-521, `settings` 523-525, `profile` 527-537, `item` 798-821 | `inline.rs:506-932` | | "Test moves" |
| `ghost_key` | `pty.rs:174-196` | `on_keys` (1069-1071) | stays (Phase 9 `key_router`) |
| `next_word`, `ghost_rest` | `pty.rs:197-211` | `pty.rs:928, 1093`; tests 1691-1728 | `GhostText` (7.1) |
| `Session.ghost`, `.llm: Option<(String, String)>` (computed-for buffer, text) | `pty.rs:378-380`; written 685-687, 882; read 925-928; accept 1091-1098; `None` at 557-558, 802-803, 1096-1097, 1847-1848 | | `Option<GhostText>` (7.1) |
| `llm_enabled` | `pty.rs:902-908` | `on_completion` (886-893), test `llm_ghost_requested_for_commands_without_spec` | resolver import switch (7.2) |
| engine ghost/LLM code: `use crate::inline::{self, GhostCache, Source}` (8), `INLINE_ENABLED`/`INLINE_HISTORY_COUNT` (18), `fn llm_history` (267-270), field `ghost_cache` (345/408), `CommandFinished`/`CommandEnded` arm (459-468, clear 463-465), `Llm` arm (470-475), `env()` (502-508), clamp (529), `error(.., false)` (537), `error(.., true)` (542, 561), `fn error` (623-631), `pump` Generators reply (1069-1073), `reply` (1101-1185, ghost at 1159), `ghost()` (1187-1202), `llm()` (1247-1275), test `llm_history_honours_history_count` (1334-1358) | `engine.rs` | | Phase 6: the legacy block of `engine_server.rs` [K1]; then 7.6 (local), 7.7 (LLM, clear, wrapper, test) |
| `InlineCmd` (42-59), `settings_repository()` (79-82, stays for `theme`), `PROFILE_EXAMPLE` (202), `inline` (204-231), `set_inline_enabled` (233-250), `set_customization` (263-289), `choose_source` (291-319), `render_sources` (321-367), test helper `profile(name)` (687-697), test `inline_show_customizations` (700-732) | `cli.rs` | `bootstrap/cli.rs:47-49` (`/// Inline shell completions`, `#[command(subcommand)] Inline(cli::InlineCmd)`), `:127` (`cli::inline(c)`) | use cases (7.8); `infrastructure/cli/inline_command.rs`, `stdin_source_chooser.rs`, `inline_command_module.rs` (7.9) |
| `inline_kind` (528-532), the check (593-594), `inline_outcome` (612-630), `is_local` (632-636) | `doctor.rs` | `doctor --all` | imports/accessors only (7.2); stays (Phase 10) |
| test `empty_search_term_is_no_prefix_match` | `suggest.rs:1602-1620` (Phase 6: `suggestion_filter.rs` [K5]) | | `spec_ghost_text.rs` (7.3) |
| `mod inline;` | `main.rs:13` | | deleted (7.7) |
| `REGISTERED_MODULES`, `LEGACY_MODULES` | `architecture_tests.rs:9, 15-33` | | `"ghost_text"` registered (7.10); `"inline"` stays |
| test doc comments naming the files Phase 7 empties | `settings_catalog.rs:335` ("engine.rs llm_history"), `:355` ("cli.rs, doctor.rs, engine.rs and pty.rs"), `:555` ("inline.rs and cli.rs") | | repointed (7.7, 7.9; RC8) |

## Behaviour that must not change

### Local ghost (7.3, 7.6)
- **P7-T1 Gate** (GHO-GT1). `engine.rs:1189-1194`: `!self.settings.bool(INLINE_ENABLED) || req.buffer.is_empty() ||
  cursor != req.buffer.len()` → `None`. `cursor` is the engine's clamped cursor (`req.cursor.min(req.buffer.len())`,
  `engine.rs:529`), compared in **bytes**. A use case that takes the request's raw cursor, or a char count, changes
  when the ghost shows. `SuggestLocalGhostTextUseCase` reads `result.cursor()`, which must be that clamped cursor for
  every result kind **[K2]**.
- **P7-T2 Dispatch** (GHO-GT2, D8). `engine.rs:1197-1201`: `Ok(Source::Specs) => specs()`, `Ok(Source::History) |
  Ok(Source::Profile(_)) => history()`, `_ => history().or_else(specs)`. LLM-profile users get local **history only**;
  `HistoryAndSpecs` and an invalid or missing profile (`Err`) get history, then specs. D8 makes the `_` arm
  `Ok(GhostTextSource::HistoryAndSpecs) | Err(_)`. Dropping the `Err` fallback silently removes ghost text for a broken
  `inline.source`.
- **P7-T3 Laziness** (GHO-GT3). `history` and `specs` are closures (`engine.rs:1195-1196`): Specs never reads the
  history; `or_else` reads specs only when history found nothing. Keep them lazy.
- **P7-T4 `history_ghost`** (GHO-GT4). `inline.rs:131-139`: strict `e.command().len() > buffer.len() &&
  e.command().starts_with(buffer)` (bytes, case-sensitive); the newest `exit_code() == Some(0)` first, then the newest
  of any kind (`None` included); **no** privacy filter (private commands do ghost from history). Input: the whole
  in-memory history (`history.recent()`, at most 5000, the load snapshot plus this process's appends, never re-read).
- **P7-T5 `spec_ghost`** (GHO-GT5). `inline.rs:144-155`: the first item in **rank order** (`find_map`);
  `strip_suffix(' ')` strips ONE space (`"a  "` → `"a "`); `cursor_back <= text.len() - stripped.len()` (bytes); reads
  `kind` (not `original_kind`), skips `AutoExecute` and `History`; needs `delete == 0`, `prefix_match` and a non-empty
  text. The input is the reply's final ranked suggestions in the same order.
- **P7-T6 Ghost on every reply** (GHO-GT6, CON-T19, P6-Q15). `reply()` computes it for Parse **and** Generators
  (`engine.rs:1159`; `pump` replies through `reply`, 1069-1073); error replies compute it with `items = None` for "no
  current command" (542) and `NoSuggestions` (561); the mid-word guard passes `false` (537). The pty keeps an old
  ghost when a Generators reply has none (`pty.rs:881`: `if c.phase == Phase::Parse || c.ghost.is_some()`), so a missing
  recompute on Generators replies hides every spec ghost that comes from generator results. Phase 6 moved this to
  `send_reply` for every result; 7.6 keeps that call site and only swaps the body.
- **P7-T7 Flag-false ≡ `None`** (GHO-GT7, CON-T19). The mid-word guard (`engine.rs:530-538`) fires only when
  `cursor < buffer.len()`, where P7-T1 returns `None`; `spec_ghost(&[])` is `None` like `items = None`. So computing the
  ghost for every result from the result's own buffer, cursor and suggestions gives the same output. Keep this proof
  in the 7.6 commit body.
- **P7-T8 Settings snapshot** (GHO-GT8). The local ghost and the LLM handler read the **engine's** settings (replaced
  only by the Settings message, `engine.rs:476-482`; post-6 `session.settings()` **[K8]**), never the file. A use case
  that loads settings from a repository per request changes timing and adds I/O.

### LLM request (7.4-7.7)
- **P7-T9 Source check first** (GHO-GT9). `engine.rs:1249-1252`: anything but `Ok(Source::Profile(_))` sends
  `Msg::Ghost { req, text: None }` at once on the engine thread (the pty clears `s.llm` with it, `pty.rs:684-688`). There
  is **no** `inline.enabled` check here (the pty checks before sending, `pty.rs:902-908`): do not add one.
- **P7-T10 Cache hit** (GHO-GT10). `engine.rs:1253-1259`: `self.ghost_cache.lock().ok().and_then(|c|
  c.lookup(&buffer))` → reply at once on the engine thread, no thread. A poisoned lock is a silent miss.
- **P7-T11 Snapshot at request time** (GHO-GT11). `engine.rs:1260-1261`: `llm_history(&self.settings,
  &self.history.recent())`, then `self.env().clone()`, both on the engine thread, after the cache miss, before the spawn.
  `count = settings.i64(INLINE_HISTORY_COUNT).max(0) as usize` (`engine.rs:269`): a float `10.0` reads as the default
  49 (`Settings::i64`, `settings.rs:37-42`), a negative as 0.
- **P7-T12 Worker** (GHO-GT12). `engine.rs:1263-1274`: `std::thread::Builder::new().name("llm".into())`; the worker
  runs `llm_complete`, then `if let (Some(t), Ok(mut c)) = (&text, cache.lock()) { c.insert(&[format!("{buffer}{t}")]);
  }` (the lock is taken even for `None`), then `let _ = tx.send(Msg::Ghost { req, text })`: insert **before** send,
  poisoned lock = skip, send errors ignored. Spawn failure: `logger::warn(&format!("engine: llm worker: {e}"))` and
  **no** reply.
- **P7-T13 Insert-after-clear race** (GHO-GT13, D1). The engine thread clears the cache on `CommandFinished`/
  `CommandEnded` (`engine.rs:463-465`; order record → clear → `reset_slots` → `cur = None`, 459-468) while a worker
  started earlier may insert afterwards. Keep it; never clear on `Settings` or `Session` (today it is not, 476-488).
- **P7-T14 `GhostTextCache`** (GHO-GT13b). `inline.rs:169-185`: `insert` gives priority 0.0 to index 0 **within one
  call**, 1.0 to the rest (the engine always inserts one element, so every entry is 0.0); `lookup` = strict
  `full.len() > buffer.len() && starts_with`, `min_by(total_cmp)` returns the **first (oldest)** minimum; no dedupe;
  the cap of 50 applies after the loop (`drain(..len.saturating_sub(50))`, 175), under the `ponytail:` comment (174).
- **P7-T15 Prompt** (GHO-GT14, M6). `llm_complete` 212: `history_prompt(history, buffer)?` returns `None` (no HTTP, no
  log) for a private buffer or a buffer alone over 10 240 bytes. `history_prompt` (280-307): `is_private` on the buffer,
  then each history line (`to_lowercase` Unicode, substring of `PRIVATE_WORDS` including `"authorization:"` and
  `"bearer "`); cost `c.len() + 8` per line, total `sum - 1`; drop the oldest while `> 10_240`; `skip + 1 == len` →
  `None`; numbering restarts at 1 with `format!("{:>5}  {c}", i + 1)`; the cost comment (290) stays. M6: the `{:>5}`
  width holds only because `HISTORY_IN_MEMORY = 5000` (`history_repository.rs:6`). `llm_history` (190-200):
  `rev().filter(usable).take(count)`, then reverse; `usable = exit_code().is_some() && !is_private(command)`.
- **P7-T16 User text** (GHO-GT15). `inline.rs:213-218`: `"OS: {}\nShell: {}\nCWD: {}\n\n{prompt}"` with
  `std::env::consts::OS` (215), `shell.name()`, `cwd.display()`. The system text is `SYSTEM` (252-255).
- **P7-T17 API key** (GHO-GT16). `inline.rs:219-224`: `env.get(&profile.api_key_env).filter(|k|
  !k.is_empty()).cloned().or_else(|| std::env::var(&profile.api_key_env).ok()).unwrap_or_default()`. The map is the
  engine's effective env (session env if non-empty, else the wrapper env); the fallback reads the **process** env on
  the **worker** thread; an empty process value is used as `""`. Empty key → no auth header.
- **P7-T18 HTTP** (GHO-GT17). `inline.rs:226-246`: a **new** `ureq` agent per request with
  `timeout_global(Some(Duration::from_secs(5)))` (227); headers added in order; `send_json(&body).and_then(|mut r|
  r.body_mut().read_json())`; ureq 3 treats a non-2xx as `Err`. Only this failure logs:
  `logger::warn(&format!("inline: profile '{}' request to {url} failed: {e}", profile.name))` (240). A pooled agent or a
  typed error that the driver logs changes behaviour or the log.
- **P7-T19 Silent drops** (GHO-GT18, D5). `parse_response` `None`, `clean` `None` and `validate` false are silent
  (247-249). A `Result` pipeline whose caller logs every `Err` adds log lines.
- **P7-T20 `build_request`** (GHO-GT19, M7). `inline.rs:310-346`: `base_url.trim_end_matches('/')` strips **all**
  trailing slashes (315); OpenAI: `{base}/chat/completions`, header `("authorization", "Bearer {key}")` only for a
  non-empty key, effort ignored; Anthropic: `{base}/v1/messages`, `x-api-key` (if key) then `("anthropic-version",
  "2026-10-21")` always (333), `output_config: {effort}` whenever `effort` is `Some` (also `Some("")`). Bodies are `json!`
  objects; `serde_json` has no `preserve_order` (`Cargo.toml`), so keys go on the wire **sorted** (`request_bodies` pins
  the OpenAI string). A `#[derive(Serialize)]` struct would send declaration order. M7: no `temperature`, `top_p` or
  `top_k` (asserted at 690-699); keep the whole-body assertions.
- **P7-T21 `parse_response`** (GHO-GT20). `inline.rs:349-367`: OpenAI `choices[0].message.content.as_str()` (null →
  `None`, `finish_reason` not read); Anthropic `stop_reason == "refusal"` → `None`, non-array content → `None`, all
  `type == "text"` blocks **concatenated**, others skipped; an empty array → `Some("")` (then `clean` → `None`).
- **P7-T22 `clean`** (GHO-GT21). `inline.rs:371-397`, fixed order: the first line (`text.lines()`, so CRLF passes) that
  is non-empty after trim and not a fence → the quotes `` ` `` `"` `'` tried in order on the **trimmed** line, the inner
  text must not contain the quote, else the **untrimmed** line (leading spaces survive) → a history number stripped
  only when digits are followed by exactly two spaces → the buffer prefix stripped once (only when the buffer is
  non-empty) → `trim_end` → any `char::is_control` (C0, `\t`, DEL, C1) rejects the whole completion.
- **P7-T23 `validate`** (GHO-GT22). `inline.rs:400-418`: `split_words(full.trim())` `None` (open quote) → true; not
  exactly 2 words or first ≠ `"cd"` → true; `expand` `None` → true; `path.is_absolute() && !path.is_dir()` → false (414);
  else `cwd.join(path).canonicalize().is_ok_and(|p| p.is_dir())` (417). Keep **both** fs calls in this order (`cd -`,
  `cd ~user` and a missing relative dir fail; a symlinked or Windows path may differ if collapsed into one call).
- **P7-T24 `split_words`** (GHO-GT23). `inline.rs:421-460`: `let escapes = !cfg!(windows)` (422) is compile-time, not
  from the `Shell`; in double quotes a backslash escapes only `"` `\` `$` `` ` ``; `''` and a trailing lone `\` create an
  empty word (`get_or_insert_default`); Unicode `is_whitespace`. Never dedupe with completion's tokenizer.
- **P7-T25 `expand`** (GHO-GT24). `inline.rs:463-504`: only `~`, `~/` (and `~\` when `cfg!(windows)`, 466) expand; home
  = env map `HOME`, then `USERPROFILE`, then `user_home_from_env().ok()` lossy (471-475: process env, USERPROFILE before
  HOME on Windows), read **only** for a `~` target without `HOME`/`USERPROFILE` in the map; `$`+non-identifier keeps a
  literal `$`; `${}` stays literal; an unclosed `${` → `None`; variables only from the map. Never dedupe with
  completion's `shell_expand`.

### Profiles and source (7.2)
- **P7-T26 `parse_profile` order and texts** (GHO-GT25, M8). `inline.rs:80-123`: reserved name first (81-83,
  `'{name}' is a reserved source name`) → `"not a JSON object"` (84) → provider (the generic `"{k} must be a string"`
  (88), value `"unknown provider '{p}' (expected openai or anthropic)"`, missing `"missing provider"`) → model (type,
  then empty → `"missing model"`) → `max_tokens` (107-113: `as_u64()` in `1..=u32::MAX`, `"max_tokens must be a
  positive integer"`; 32.0, 1e3, -1, 0 rejected) → the struct literal (114-122) evaluates `s("base_url")?`,
  `s("api_key_env")?`, `s("effort")?` in field order. JSON null = absent for every field; `""` for `base_url`/
  `api_key_env` is **kept** (`unwrap_or_else` only on `None`). Defaults: openai `https://api.openai.com/v1`,
  `OPENAI_API_KEY`, 64; anthropic `https://api.anthropic.com`, `ANTHROPIC_API_KEY`, 1024. `from_setting` keeps the
  literal and this left-to-right order. Golden never sets a valid profile: only `inline-diff.sh` covers these texts.
- **P7-T27 `profiles()`** (GHO-GT26). `inline.rs:55-62`: `with_prefix("inline.profiles.")` over the **flat** dotted map
  (`inline.profiles.a.b` is profile `"a.b"`, `inline.profiles.` is `""`); re-sorted by byte order.
- **P7-T28 `source()`** (GHO-GT27). `inline.rs:65-77`: `settings.string(INLINE_SOURCE).unwrap_or_default()` (a
  non-string reads as the catalog `"history+specs"`, `settings_catalog.rs:555-560`); texts `"profile '{name}' not
  found"`, `"profile '{name}' is invalid: {e}"`. Reason texts are read three ways: `render_sources` prints them
  (`cli.rs:337, 358`), doctor discards them (`Err(_)`, `doctor.rs:621`), and `inline_show_customizations` feeds
  `Err("model missing")` (`cli.rs:703`), a text `parse_profile` never produces. So they stay `String` (R5).

### Ghost text in the pty (7.1; legacy until Phase 9)
- **P7-T29 `ghost_rest`** (GHO-GT28). `pty.rs:208-211`: `g.strip_prefix(buffer.strip_prefix(base)?)?`; `None` when the
  rest is empty or holds any `char::is_control`. Never validate control characters in `GhostText::new` instead (not
  provably equivalent).
- **P7-T30 `next_word`** (GHO-GT29). `pty.rs:198-204`: leading whitespace plus the next non-whitespace run, byte
  slicing.
- **P7-T31 `ghost_key` stays** (GHO-GT30, CON17). `pty.rs:175-196` takes `crate::keys::{Key, KeyCode, Mods}` (`pty.rs:3`):
  exact-modifier matches (`Right`/`End` plain → whole; `Ctrl-f/F/e/E` → whole; `Alt-f` → word; `Ctrl-Right` → word).
  ghost_text → dropdown is forbidden; Phase 9's `key_router` takes it.
- **P7-T32 `Msg::Ghost` arm** (GHO-GT31, M2). `pty.rs:682-690`: accepted only if `req == s.req && req == s.llm_req`;
  `s.llm = text.zip(s.sent.as_ref().map(|x| x.0.clone())).map(|(t, b)| (b, t))`: keyed by the **current** `s.sent`,
  `None` clears through the `zip`; then `s.redraw()` with **no** `release_held` (unlike `on_completion`, 899) **[K4]**.
  No pty test feeds `Msg::Ghost` at `ff2db5a`; 7.0 adds `should_show_the_llm_ghost_text_of_the_current_request_behind_the_local_one` (with P7-T33's local-before-LLM order).
- **P7-T33 `ghost_text` guards** (M1). `pty.rs:912-929`, in order: `inline.enabled`, `tracker.preexec`, `alt()`, `cur?`,
  an empty buffer, a cursor not at the end, `shell_ghost`; then `basis()?` when `typed`; then
  `[&self.ghost, &self.llm].into_iter().flatten().find_map(..)`: the **local ghost wins** over the LLM one.
- **P7-T34 `on_completion` keeps an old ghost** (GHO-GT6). `pty.rs:881-883`: `if c.phase == Phase::Parse ||
  c.ghost.is_some() { self.ghost = c.ghost.clone().map(|g| (c.buffer.clone(), g)); }`. Only the constructor changes
  **[K4]**. The accept (1091-1098) forwards `g` whole or `next_word(&g)`, then clears both ghosts.
- **P7-T35 `llm_enabled`** (CON-T26). `pty.rs:902-908`: `settings.bool(INLINE_ENABLED) && matches!(source(..),
  Ok(Source::Profile(_)))`; the same predicate over the resolver after 7.2. Guard
  `llm_ghost_requested_for_commands_without_spec` (`pty.rs:1995`).

### The `inline` command (7.8, 7.9; golden-covered in part)
- **P7-T36 Strict load** (GHO-GT32, CON-T21). Every arm loads with the strict `JsonFileSettingsRepository::load`
  (`cli.rs:210, 218, 234, 264`), never `load_or_defaults`. Golden `inline status` on `{oops`:
  `error: $SANDBOX/home/.tabsmith/settings.json is corrupted: key must be a string at line 1 column 2`, exit 1
  (`golden.txt:6157-6160`); no home: `error: could not determine the home directory (set HOME or TABSMITH_HOME)`
  (6250-6253); `TABSMITH_SETTINGS_PATH` alone works (8889-8892, D3). The repository resolves its path per call, so one
  built in bootstrap stays lazy (`json_file_settings_repository.rs:13-19`).
- **P7-T37 Colour and texts** (GHO-GT33, CON-T22). `color = stdout_tty()` sampled once at the top of `inline`
  (`cli.rs:205`); `Inline is {bold(enabled|disabled)}` (211-214); `Inline enabled`/`Inline disabled` in SGR 35
  (238-249); `Customization {bold(name)} selected` (287); `Customization unset` unbolded (277); errors `Customization not
  found` (284) and `No customization selected` (294, 317).
- **P7-T38 `set-customization`** (GHO-GT34). `cli.rs:263-289`: **one** load before the prompt; the save after it is
  over the **loaded** settings (a concurrent edit during the prompt is overwritten, as today); only parse-valid
  profiles are offered and selectable (267-270); a name matching an invalid profile → `Customization not found`, before
  any save; interactive None → save `without(INLINE_SOURCE)`, print `Customization unset`.
- **P7-T39 `choose_source`** (GHO-GT35). `cli.rs:291-319`: the **stdin** tty check first, before any output (293-295);
  rows `{i}) {n} - {d}` (builtins, then `"{provider} {model}"`), then `{N+1}) None`; `print!("Select a customization: ")`
  + explicit `flush()?` (310-311); `read_line(..)?` (313); `0`, out of range, empty or EOF → `No customization
  selected`; io errors of flush/read_line propagate with their own text.
- **P7-T40 `render_sources`** (GHO-GT36). `cli.rs:321-367`: plain blocks `{bold(n)}{mark}\n{d}\n\n` (mark outside the
  bold); an invalid profile `{bold(n)} (invalid: {reason})\n\n` never gets `(selected)` in plain but carries `selected`
  in JSON; `PROFILE_EXAMPLE` only when there are **no** profiles at all (invalid ones count); JSON via `json!` (sorted
  keys; golden `inline show-customizations --format json` is byte-checked), `+ "\n"`, printed with `print!`; valid
  profile rows have `provider`, `model`, `base_url`, `api_key_env`, no `max_tokens`/`effort`.
- **P7-T41 Error chains** (GHO-GT37, CON-T21). Today the arms `?` a `SettingsError` or an `io::Error` into anyhow;
  `main` logs `{err:#}` (`main.rs:43`) and `print_error` prints `error: {err:#}` (`bootstrap/cli.rs:186-188`). A typed
  error wrapping them must print the same chain: Display = the wrapped error's Display, `source()` = the wrapped
  error's `source()` (the `SettingsError` precedent, `settings_error.rs:15-33`). No `downcast`, `.is::<`, `root_cause` or
  `.chain()` exists in `src`.
- **P7-T42 clap** (GHO-GT38, CON-T23, D4). `InlineCmd` (`cli.rs:43-60`): `Enable, Disable, Status, #[command(alias =
  "set-source")] SetCustomization { name: Option<String> }, #[command(alias = "show-sources")] ShowCustomizations {
  #[arg(long, short, value_enum, default_value_t)] format: OutputFormat }`, each with its doc comment;
  `bootstrap/cli.rs:47-49` `/// Inline shell completions` `#[command(subcommand)] Inline(..)`. Golden `inline --help`
  (`golden.txt:66-79`), `inline set-customization --help` (296-306), `completion bash/zsh/fish` (1445-1590, 4984, 5141,
  5867, 6039). Only `pub` → `pub(crate)` may change.
- **P7-T43 A non-bool `inline.enabled` reads as enabled** (M9). `inline status` uses `Settings::bool`, which reads a
  non-bool as the catalog default `true` (`settings.rs:21-27`, `settings_catalog.rs:177`). `GetInlineStatusUseCase`
  keeps the lenient getter. Only `inline-diff.sh` covers it.
- **P7-T44 Imports the move leaves unused** (CON-T24; confirmed at `ff2db5a`). At 7.9: `crate::inline::..` (`cli.rs:3`),
  `Settings` (`cli.rs:4`, used only at 252), `INLINE_ENABLED`/`INLINE_SOURCE` (only the inline section), `IsTerminal`
  (`cli.rs:26`, only 293). Left in, they are new warnings over a full budget. `SettingValue` (155), `bail!` (112, 153),
  `Write`, `json!` (diagnostic), `bold`/`sgr`/`stdout_tty` (theme, key tester), `OutputFormat` (64) and
  `settings_repository()` (theme, 129) stay used. `cli.rs:1`'s `//!` names "inline" (CON16).

### Doctor (7.2; legacy until Phase 10)
- **P7-T45 `inline_outcome`** (GHO-GT39, CON-T25). `doctor.rs:613-630`: disabled → `Warn("Inline is disabled, to
  re-enable run: tabsmith inline enable")` (617); `source` Err → `Inline source '{}' is not a valid profile` with
  `settings.string(INLINE_SOURCE).unwrap_or_default()` (622); `Ok(Profile(p)) if !is_local(&p.base_url) &&
  !env_set(&p.api_key_env)` → `{api_key_env} is not set` (626); the `Ok(_)` arm stays (Phase 10 makes it explicit).
  `is_local` = `://localhost`, `://127.0.0.1`, `://[::1]` (632-636). Kind `Soft` when disabled (528-532). Golden shows
  only `✔ Inline` (`golden.txt:6127`, 6184). Only the import paths and `p.base_url`/`p.api_key_env` → accessors change.

### Cross-cutting
- **P7-T46 cfg** (GHO-GT40, RC3). The slice holds no `#[cfg(..)]` attribute, only the two `cfg!(windows)` expressions
  (`inline.rs:422, 466`), compiled on every target. The touched legacy files already carry cfg attributes, none in a
  ghost or inline part: `pty.rs` 126, 128, 1317, 1436, 1776, 2504; `cli.rs` 608, 788, 830; `doctor.rs` 481, 655, 723,
  775, 787. The design's grep `cfg(windows)\|…` cannot see `cfg!(windows)` (the `!`). The check per touched file: the
  attribute grep returns the same lines as `git show phase7-start:<file>`, shifted only by the step's edits; and
  `grep -rn 'cfg!(' src/ghost_text src/inline.rs` shows exactly the two expressions (in `inline.rs` until 7.4, then in
  `cd_target_validator.rs`).
- **P7-T47 Logging** (GHO-GT41, D5). Exactly two log lines in the slice: P7-T18's warn (infrastructure,
  `HttpLlmClient`) and P7-T12's warn (bootstrap, the engine server). Nothing in domain or application logs.
- **P7-T48 Threads** (GHO-GT42). Engine thread: settings, history (`Rc`, `RefCell` inside: `!Send`), the local ghost,
  the cache lookup and clear, the source check, the history and env snapshot. `llm` threads: HTTP, clean, validate (fs),
  the process-env key read, the cache insert, the `Msg::Ghost` send. Everything moved into the worker is `Send`; ports
  crossing the boundary are `Arc<dyn Port + Send + Sync>`. `Shell` is `Copy` (`shell.rs:1`).
- **P7-T49 Clippy budget** (GHO-GT43). 3 warnings, all used. An enum holding the LLM job inline trips
  `large_enum_variant` (≈330 bytes vs 24): box it. Unit-struct adapters are built by name, never `::default()`; no
  zero-argument `new` on a double (`new_without_default`). `LlmGhostTextJob::new` has 7 parameters of 7 distinct types:
  at clippy's `too_many_arguments` limit, not over it.
- **P7-T50 `LEGACY_MODULES` keeps `inline`** (CON-T27). `architecture_tests.rs:864-884` feeds legacy names into the
  checker and passes only while they are listed; the ponytail comment (13-14) says names stay. Never "clean up"
  `inline` (or `engine`, `types`, …) from the list.
- **P7-T51 Fitness scope** (D2, CON-T28, CON-T29). Items under `#[cfg(test)]`/`#[cfg(all(test, ..))]` and sibling
  `tests.rs` files keep only the shared-kernel and `mod.rs` rules (`architecture_tests.rs:147-180`), so doubles in port
  files and `serde_json`/`settings_from_json` in domain test modules pass. An inner `#![cfg(test)]` is not recognised: a
  test-only *file* is read as production. A domain file may name no `application` path, not even its own module's
  (`architecture_tests.rs:313-316`). Infrastructure may use the shared kernel's infrastructure (`OutputFormat`,
  `terminal_style`, `logger`, `user_home_from_env`). Fitness rules 6-7 check that `mod.rs` and `main.rs` hold only
  attributes, child declarations (and `fn main`); alphabetical order is a convention, not checked (RC6).
- **P7-T52 Golden does not cover the profile paths** (CON 2.5, RC1). Golden writes `inline.llm.profiles`
  (`golden.sh:55`), never a valid `inline.profiles.<name>`, and selects only `history` (:82); no golden command reaches
  the engine (`_ pty` runs only with `--help`, :31); `llm_complete` has no automated test. Golden's inline coverage is
  **13 captures**: `inline --help` (the loop at :29), `inline set-customization --help` (:40), 8 commands at :76-83, and
  3 error/env runs of `inline status` (`{oops` :110, no home :132, `TABSMITH_SETTINGS_PATH` :135); `doctor --all` runs 3
  times (:101, :111, :134); `completion bash/zsh/fish` at :95-97. Everything else is `inline-diff.sh`'s (R1, section
  "Output-equivalence guard").
- **P7-T53 Test lock per phase.** The gate's MISSING check compares with `ef807c1` only, so a deleted test that
  Phases 1-6 added would pass it. Every step runs `python3 scripts/hexagonal-refactor/testlock.py . phase7-start
  --diff` too: MISSING 0, DUPLICATE 0 (testlock keys tests by fn name; a moved test left in both files, or a new name
  that already exists, hides one), and CHANGED exactly this phase's five restructurings, each in its step. testlock's
  loose form reduces `a::b::c` to its last segment (`testlock.py:118-120`), so `Provider::` → `LlmProvider::`,
  `GhostCache::default()` → `GhostTextCache::default()`, `Source::Profile` → `GhostTextSource::Profile` and
  `crate::inline::spec_ghost` → `spec_ghost` are path-only. Helpers outside `#[test]` fns are not locked.
- **P7-T54 A flaky gate test.** `integrations_zsh_powerlevel10k_prompt_wrapped` (shell_integration's
  `embedded_shell_scripts` tests) is pty-timing flaky under the gate's parallel load. When it is the gate's only
  failure, rerun it alone with `cargo test --locked
  shell_integration::infrastructure::adapters::embedded_shell_scripts::tests::integrations_zsh_powerlevel10k_prompt_wrapped
  -- --exact` (it must print `1 passed`), then run the full gate again. Never edit, ignore or skip it.
- **P7-T55 History is append-only.** Never amend, rebase, reset, `filter-branch` or force-push a commit; fix forward.
  Subjects are at most 72 characters; a longer one is reported, not rewritten.
- **P7-T56 Stale comments** (RC8). `settings_catalog.rs:335` names "engine.rs llm_history", `:355` "cli.rs, doctor.rs,
  engine.rs and pty.rs" (readers of `inline.enabled`), `:555` "inline.rs and cli.rs" (readers of `inline.source`).
  Phase 5 set the precedent of repointing such comments (`ff2db5a`). 7.7 repoints :335 to
  `request_llm_ghost_text_use_case.rs`; 7.9 rewrites each of :355 and :555 to name the files that read the key after
  it: :555 "`ghost_text_source_resolver.rs` and `doctor.rs`"; :355 `get_inline_status_use_case.rs` (the CLI's reader;
  `inline_command.rs` only prints), `doctor.rs`, `suggest_local_ghost_text_use_case.rs` (the engine's reader since 7.6,
  in place of whatever engine file Phase 6 left there) and `pty.rs`. Comment-only: testlock ignores them.

## Decisions

R1-R24 are the binding design's, with its ids and content; the recheck corrections are folded in and marked (RCn).

### R1 Phase start, guards first, three runs
- `phase7-start` is a local, lightweight tag at the commit that hands Phase 7 over (`docs: hand Phase 7 over with its
  research`), which sits directly on the commit that closes 6.7 and changes only docs (see "Launch"). The testlock
  baseline is therefore Phase 6's end.
- Step **7.0** adds the guards before any inline code moves: `scripts/hexagonal-refactor/inline-diff.sh`
  (`chore(scripts)`, P6-Q30, CON15), two engine-server characterization tests of the `Llm` arm and one pty
  characterization test of the `Msg::Ghost` arm (P7-T32, P7-T33) (`test(ghost_text)`).
- Runs: **1** guards + the domain (7.0-7.4); **2** ports, adapters and the engine switch, `inline.rs` gone (7.5-7.7);
  **3** the inline command and the close (7.8-7.10).

Facts: P7-T52. Rejected: no 7.0 (CON15: the script must exist before code moves); the CLI before the engine (the
engine switch is the riskier change and wants the domain and adapters settled; the CLI depends only on 7.2).

### R2 Ids
Every brief cites prefixed ids (P6-Q31): `P7-T…`, `GHO-GT14`, `CON-T21`, `P6-Q15`, `M2`, `RC3`. Phase 6's rechecks are
`P6-R…`; this phase's decisions are `R…`, its rechecks `K…`. Resolves X15 for Phase 7.

### R3 `GhostText` and the pty (GHO1, GHO2, CON17, M1, M2)
- `domain/value_objects/ghost_text.rs`: `GhostText { computed_for: String, text: String }` (private fields, `Clone,
  Debug, PartialEq, Eq`):
  - `new(computed_for: String, text: String) -> Self`;
  - `rest_after(&self, buffer: &str) -> Option<String>`: `ghost_rest`'s body verbatim
    (`self.text.strip_prefix(buffer.strip_prefix(&self.computed_for)?)?`, then the empty/`char::is_control` guard,
    P7-T29);
  - `next_word(rest: &str) -> &str`: an associated fn, `next_word`'s body verbatim (P7-T30).
- `ghost_key` stays in `pty.rs` (P7-T31).
- `pty.rs`: fields `ghost`, `llm` become `Option<GhostText>`; the four sites change their constructor/reads only:
  - the `Msg::Ghost` arm: `.map(|(t, b)| GhostText::new(b, t))` on the **same** `text.zip(s.sent…)` (P7-T32: keyed by
    the current `s.sent`, `None` clears through the `zip`, then `redraw()` with **no** `release_held`);
  - `on_completion`: `GhostText::new(<result buffer>, <ghost>)` under the unchanged `phase == Parse ||
    ghost.is_some()` condition **[K4]**;
  - `ghost_text(typed)`: guards untouched (P7-T33), then `[&self.ghost, &self.llm].into_iter().flatten().find_map(|g|
    g.rest_after(&buffer))` (local before LLM);
  - accept: `GhostText::next_word(&g)`.
- `next_word` and `ghost_rest` are deleted from `pty.rs`. `CompletionResult.ghost` and `Msg::Ghost.text` stay
  `Option<String>` (completion may not name ghost_text; `Msg` is Phase 9's).

Rejected: moving `ghost_key` (forbidden edge); a ghost_text-owned key enum (a new type with no glossary row); keeping
the tuples and building a `GhostText` per draw (a VO with no production owner); validating control characters in
`new` (P7-T29).

### R4 `LlmProvider` and `LlmProfile` (GHO4, GHO17, M8)
- `llm_provider.rs`: `LlmProvider { OpenAi, Anthropic }` (`Clone, Copy, Debug, PartialEq, Eq`) + `name(self) ->
  &'static str` (= `cli::provider_name` verbatim, `cli.rs:256-261`).
- `llm_profile.rs`: `LlmProfile` (7 private fields, `Clone, Debug, PartialEq`), with:
  - `from_setting(name: &str, value: &SettingValue) -> Result<LlmProfile, String>`: `parse_profile`'s body
    **verbatim**, including the reserved-name check over `BUILTIN_SOURCES`, every text (P7-T26) and the struct literal
    **in field order**, so `s("base_url")?`, `s("api_key_env")?`, `s("effort")?` still evaluate left to right after
    `max_tokens`;
  - accessors `name()`, `provider()`, `model()`, `base_url()`, `api_key_env()`, `max_tokens()`, `effort() ->
    Option<&str>`;
  - `#[cfg(test)] new(parts: LlmProfileParts) -> Self` with `#[cfg(test)] pub(crate) struct LlmProfileParts` (the seven
    named fields, same file; the P6-Q2 "Parts" precedent).
- Production builds profiles only through `from_setting` (D7).

Facts: three test literals build a `Profile` (`inline.rs:527-537, 879-887`, `cli.rs:687-697`); four of the seven fields
are `String`s, so a positional `new` with `base_url`/`api_key_env` swapped compiles. Rejected: GHO4-a's 7-argument
`new`; a production `new`; GHO4-b's `with_*` builders (the expected value in `profiles_and_source` would lean on the
defaults it tests).

### R5 `GhostTextSource` and the resolver; reasons stay text (GHO3-a, GHO17)
- `ghost_text_source.rs`: `GhostTextSource { History, Specs, HistoryAndSpecs, Profile(LlmProfile) }` (`Clone, Debug,
  PartialEq`) + `BUILTIN_SOURCES` (same order and texts).
- `domain/services/ghost_text_source_resolver.rs`: private `PROFILES_PREFIX`, `profiles(&Settings)`,
  `source(&Settings)` and `selected_source(&Settings) -> &str` (from `cli.rs:252-254`), bodies verbatim (P7-T27,
  P7-T28).
- `profiles` returns `Vec<(String, Result<LlmProfile, String>)>`, `source` returns `Result<GhostTextSource, String>`.

Rejected: GHO3-b typed reasons (`.to_string()` in about 9 locked assertions; `PartialEq` impossible next to
`SettingsError`, which derives only `Debug`, `settings_error.rs:5-13`).

### R6 Cache and local ghost services; the moved completion test (C22, X14, GHO16)
- `domain/entities/ghost_text_cache.rs`: `GhostTextCache` (← `GhostCache`, `Default`), `clear`/`insert`/`lookup`
  verbatim with the `ponytail:` comment (P7-T14).
- `domain/services/history_ghost_text.rs`: `history_ghost` verbatim (P7-T4).
- `domain/services/spec_ghost_text.rs`: `spec_ghost(&[Suggestion])` as Phase 6 left it (P7-T5) **[K3]**.
- `empty_search_term_is_no_prefix_match` moves into `spec_ghost_text.rs`'s tests and **imports** `opts`/`ctx` from
  `crate::completion::domain::test_fixtures` (P6-Q16) and the completion services it calls; no copies **[K5]**.

Rejected: GHO16/CON18 copies of `opts`/`ctx`.

### R7 `LlmPrompt` and the prompt builder (GHO10, M6)
- `llm_prompt.rs`: `LlmPrompt { system: String, user: String }` (`Clone, Debug, PartialEq`), `new(system, user)`,
  `system()`, `user()`.
- `domain/services/llm_prompt_builder.rs`: `SYSTEM`, `PROMPT_MAX_BYTES`, `PRIVATE_WORDS`, private `is_private`,
  `usable`, `history_prompt` (verbatim, its cost comment kept; M6), `llm_history(entries, count)`, and the new
  composition `llm_prompt(history, buffer, cwd: &Path, shell: Shell) -> Option<LlmPrompt>` = `llm_complete` lines
  212-218 verbatim (`history_prompt(..)?`, then the P7-T16 header), `system` = `SYSTEM`.
- `build_request(profile, prompt: &LlmPrompt, key)` reads `prompt.system()` and `prompt.user()` where it read `SYSTEM`
  and `user`. `request_bodies` becomes CHANGED once (7.4): its five `"U"` arguments (`inline.rs:664, 671, 675, 688,
  692`) become `&prompt("U")` through a test helper `prompt(user) = LlmPrompt::new(SYSTEM.into(), user.into())`; every
  assertion and value is kept.

Rejected: GHO10-b (adapters read `SYSTEM` while `LlmPrompt.system` sits unused) and a user-only `LlmPrompt`.

### R8 The cd check: pure domain target, probe-backed validation (GHO9)
- `domain/services/cd_target_validator.rs`: `cd_target(full: &str, env: &HashMap<String, String>, fallback_home: impl
  FnOnce() -> Option<String>) -> Option<String>`: the `cd` path to check, or `None` (accept). Private `split_words` and
  `expand` verbatim (P7-T24, P7-T25, both `cfg!(windows)` expressions); `expand`'s `user_home_from_env()` fallback
  becomes `.or_else(fallback_home)`, still read only for a `~` target without `HOME`/`USERPROFILE` in the map.
- `validate(full, cwd, env, directories: &dyn DirectoryProbe) -> bool` keeps P7-T23's order: `cd_target(.., ||
  directories.home_directory())` → `None` ⇒ `true`; `path.is_absolute() && !directories.is_dir(path)` ⇒ `false`; else
  `directories.resolves_to_directory(&cwd.join(path))`. In 7.4 `inline::validate` calls `cd_target(.., ||
  user_home_from_env().ok().map(..))` and keeps its two fs calls; from 7.5 it takes the probe; in 7.7 it moves privately
  into `application/llm_ghost_text_job.rs`, with `cd_validation`.
- `cd_validation` becomes CHANGED once (7.5): `validate(c, &d, &env)` (`inline.rs:739`) → `validate(c, &d, &env,
  &FsDirectoryProbe)` (the real adapter, D2).

Rejected: `DirectoryProbe` declared in the domain (GHO9-b); `validate` whole in an application service (GHO9-c);
closures for the two fs calls (the test would re-implement the adapter).

### R9 One HTTP adapter for both providers (TC2, P6-Q20, P6-O7, GHO7)
- `infrastructure/adapters/http_llm_client.rs`: unit struct `HttpLlmClient` implements `LlmClient`. Its `complete` is
  `llm_complete` lines 219-247 up to `parse_response` **verbatim**: key lookup (P7-T17, on the calling thread), a new
  `ureq` agent with the 5 s global timeout, the headers in order, `send_json` + `read_json`, the warn
  `inline: profile '{}' request to {url} failed: {e}` with `profile.name()` (P7-T18), then
  `parse_response(profile.provider(), &resp)`.
- Private `build_request` and `parse_response` keep **both** provider arms in one fn each, `json!` bodies (P7-T20,
  P7-T21).
- `request_bodies` and `response_parsing` live whole in this file (TC2; D6 untouched).
- Plan section 4's `openai_llm_client.rs`/`anthropic_llm_client.rs` are not created (deviation 6).

Rejected: GHO7-a (two adapters + a provider `match` in the use case: a routing rule in application, and tests split
inside one module, which P6-Q20 forbids); GHO7-b a dispatcher over two adapters; the name `UreqLlmClient`.

### R10 Ports and their doubles (plan section 1, P6-Q17)
Each port file holds the trait and its `#[cfg(test)]` double, written test-first with the port:

| Port (`application/ports/`) | Trait | Double |
|---|---|---|
| `llm_client.rs` | `complete(&self, profile: &LlmProfile, prompt: &LlmPrompt, env: &HashMap<String, String>) -> Option<String>` (`None`: no reply worth cleaning; the adapter logged any transport failure) | `ScriptedLlmClient::answering(reply: Option<&str>)` (same reply to every prompt) + `prompts() -> Vec<LlmPrompt>` (`Mutex`, so `Send + Sync`) |
| `directory_probe.rs` | `is_dir(&self, &Path) -> bool`, `resolves_to_directory(&self, &Path) -> bool`, `home_directory(&self) -> Option<String>` | `InMemoryDirectoryProbe::new(directories: &[&str], home: Option<&str>)` |
| `source_chooser.rs` (7.8) | `choose(&self, profiles: &[(String, LlmProfile)]) -> Result<Option<String>, GhostTextError>` (`Ok(None)` = "None" picked) | `FixedSourceChooser::choosing(choice: Option<&str>)` + `offered() -> Vec<String>` |

- Adapters: `HttpLlmClient` (R9); `FsDirectoryProbe` (unit): `Path::is_dir`, `canonicalize().is_ok_and(|p|
  p.is_dir())`, `user_home_from_env().ok().map(|p| p.to_string_lossy().into_owned())`, exactly today's three calls
  (`inline.rs:414, 417, 472-474`); `StdinSourceChooser` (unit, 7.9): `choose_source` verbatim (P7-T39),
  `bail!("No customization selected")` → `Err(GhostTextError::NoCustomizationSelected)`, io errors →
  `GhostTextError::SourceChooser(e)`.
- Unit structs are built by name, never `::default()`; no zero-argument `new` on a double (P7-T49).
- Ports crossing the `llm` thread are `Arc<dyn Port + Send + Sync>` (P7-T48).

Rejected: an `EnvironmentVariables` port (GHO8-c): every process-env read stays in infrastructure, once, where it
happens today.

### R11 `SuggestLocalGhostTextUseCase` on every reply (GHO5, P6-Q15, CON13)
- `SuggestLocalGhostTextUseCase { history: Rc<dyn HistoryRepository> }`, `execute(&self, settings: &Settings, result:
  &CompletionResult) -> Option<String>`:
  - the gate verbatim on `result.buffer()` and `result.cursor()` (the clamped cursor, bytes; P7-T1) **[K2]**;
  - lazy closures `history = || history_ghost(&self.history.recent(), result.buffer())` and `specs = ||
    spec_ghost(result.suggestions())` (P7-T3);
  - D8 arms: `Ok(Specs) => specs()`, `Ok(History) | Ok(Profile(_)) => history()`, `Ok(HistoryAndSpecs) | Err(_) =>
    history().or_else(specs)` (P7-T2).
- `EngineServer::send_reply` calls it for **every** result before `Msg::Completion`, with `session.settings()` (the
  engine snapshot, P7-T8) **[K8]**.

Facts: P6-Q15's proof (P7-T7) holds unchanged. Rejected: GHO's `(settings, buffer, cursor, Option<&[Suggestion]>)`
(P6 §7.1 suggested it; `send_reply` always passes `Some`; O7); a completion-declared port (GHO5-b).

### R12 `RequestLlmGhostTextUseCase` across threads (GHO6)
- `RequestLlmGhostTextUseCase { history: Rc<dyn HistoryRepository>, cache: Arc<Mutex<GhostTextCache>>, client:
  Arc<dyn LlmClient + Send + Sync>, directories: Arc<dyn DirectoryProbe + Send + Sync> }`, `execute(&self, settings:
  &Settings, env: &HashMap<String, String>, request: LlmGhostTextRequest) -> LlmGhostTextAnswer`, in today's order:
  1. `let Ok(GhostTextSource::Profile(profile)) = source(settings) else { return Immediate(None) }` (no
     `inline.enabled` check, P7-T9);
  2. `self.cache.lock().ok().and_then(|c| c.lookup(&request.buffer))` → `Immediate(Some(text))` (poisoned = miss,
     P7-T10);
  3. `llm_history(settings, &self.history.recent())`, then `env.clone()` (on the engine thread, after the miss,
     P7-T11);
  4. `Deferred(Box::new(LlmGhostTextJob::new(profile, request, history, env, client, directories, cache)))`.
- `application/llm_ghost_text_request.rs`: DTO `LlmGhostTextRequest { buffer: String, cwd: PathBuf, shell: Shell }`
  (`pub(crate)` fields; `EngineMessage::Llm` minus `req`). The env is passed by reference so it is cloned only on a
  miss, as today.
- `application/llm_ghost_text_answer.rs`: `LlmGhostTextAnswer { Immediate(Option<String>),
  Deferred(Box<LlmGhostTextJob>) }` (boxed, P7-T49).
- `application/llm_ghost_text_job.rs`: `LlmGhostTextJob` (`Send`, private fields, `new` with 7 parameters of 7 distinct
  types), `run(self: Box<Self>) -> Option<String>`: `llm_prompt(..)?` → `client.complete(..)?` → `clean(.., buffer)?` →
  `validate(full, cwd, env, directories)` → `if let (Some(t), Ok(mut c)) = (&text, self.cache.lock()) {
  c.insert(&[format!("{buffer}{t}")]); }` (the lock is taken even for `None`) → returns the text. Private `validate`
  (R8) lives here.
- `bootstrap/engine_server.rs`, `Llm` arm: `Immediate(text)` → `Msg::Ghost { req, text }` at once on the engine thread;
  `Deferred(job)` → `std::thread::Builder::new().name("llm".into()).spawn(move || { let text = job.run(); let _ =
  replies.send(Msg::Ghost { req, text }); })`; on spawn failure `logger::warn(&format!("engine: llm worker: {e}"))` and
  **no** reply (P7-T12). Insert-before-send holds because `run` inserts and returns before the send.

Rejected: GHO6-b (the use case spawns and takes a callback: a thread and a log in application); GHO6-c (a `Send` use
case: impossible with the `Rc` history); computing the prompt on the engine thread (a timing change).

### R13 One shared `Arc<Mutex<GhostTextCache>>`; the clear moves with the LLM request (GHO-GT13)
- `GhostTextModule::new` builds one `Arc<Mutex<GhostTextCache>>` and hands it to `RequestLlmGhostTextUseCase` and
  `ClearGhostTextCacheUseCase { cache }` (`execute(&self)`: `if let Ok(mut c) = self.cache.lock() { c.clear() }`).
- Both arrive in **7.7**, together. In 7.6 only the local ghost switches; the legacy `clear_ghost_text_cache` and
  `request_llm_ghost_text` keep sharing the server's own field until 7.7 deletes it.
- The insert-after-clear race stays (P7-T13); no clear on `Settings` or `Session`.

Facts: the worker inserts after `execute` has returned, so the cache cannot be a `&mut` driver-owned aggregate (plan
section 1); the engine already shares one `Arc` between the clear and the worker. Rejected: switching the clear in 7.6
(it would clear a cache the legacy handler no longer reads); a driver-owned cache passed into each `execute`.

### R14 The `historyCount` wrapper and its test (GHO15, U3, C23, TC10)
The private `llm_history(&Settings, &[HistoryEntry]) -> Vec<HistoryEntry>` (`settings.i64(INLINE_HISTORY_COUNT).max(0)
as usize`, then `llm_prompt_builder::llm_history` by module path) moves from `engine_server.rs` into
`request_llm_ghost_text_use_case.rs` in 7.7, and `llm_history_honours_history_count` moves with it **path-only** (same
name, same signature). In 7.4 the server's wrapper already calls the builder's `llm_history`.

### R15 Two module structs; the settings repository comes from bootstrap (GHO12, plan Phase 7 step 3)
- `infrastructure/ghost_text_module.rs`: `GhostTextModule { suggest_local_ghost_text, request_llm_ghost_text,
  clear_ghost_text_cache }` (`pub(crate)` fields), `GhostTextModule::new(history, client, directories)` (tests wire
  doubles), `build_ghost_text_module(history: Rc<dyn HistoryRepository>) -> GhostTextModule` = `new(history,
  Arc::new(HttpLlmClient), Arc::new(FsDirectoryProbe))`. No I/O.
  - 7.6 creates it with `suggest_local_ghost_text` only; 7.7 adds the other two and `new`.
  - `spawn` builds it right after `build_history_module`, over `Rc::clone(&history_module.history)` (the history that
    `RecordCommandUseCase` appends to; `HistoryModule` shares one `Rc`, `history_module.rs:13-24`) **[K7]**.
    `EngineServer` swaps `ghost_cache` for `ghost_text: GhostTextModule`.
- `infrastructure/inline_command_module.rs`: `InlineCommandModule { set_inline_enabled, get_inline_status,
  list_ghost_text_sources, select_ghost_text_source }`, `build_inline_command_module(settings: Rc<dyn
  SettingsRepository>)` (builds `StdinSourceChooser`).
- `bootstrap/cli.rs` `Inline` arm: `run_inline_command(c, &build_inline_command_module(Rc::new(
  JsonFileSettingsRepository::new(data_directory_from_env()))))`, the exact repository `cli::settings_repository()`
  builds (lazy, D3, P7-T36) **[K15]**.

Rejected: one module with all seven use cases (the CLI would build a history, which loads and may compact
`history.jsonl`; GHO12-b); bootstrap building the engine use cases itself (GHO12-c).

### R16 `GhostTextError` (C24, P6-Q29, GHO3)
`domain/ghost_text_error.rs`: `GhostTextError { Settings(SettingsError), CustomizationNotFound,
NoCustomizationSelected, SourceChooser(std::io::Error) }`, `Debug`, hand-written:
- `Display`: `Settings(e)`/`SourceChooser(e)` write `{e}`; the other two write `Customization not found` / `No
  customization selected`;
- `Error::source`: `Settings(e) => e.source()`, `SourceChooser(e) => e.source()`, else `None` (the `SettingsError`
  precedent: Display already **is** the wrapped error, so the chain continues with its cause);
- `From<SettingsError>`. It is `Send + Sync` (anyhow).

So `format!("{:#}", anyhow::Error::from(GhostTextError::Settings(e)))` equals `format!("{:#}",
anyhow::Error::from(e))`, and the `bail!` texts print as before (P7-T36, P7-T41; golden `golden.txt:6159`, 6252).
Rejected: CON14's bare `SettingsError` (it cannot carry the two refusals or the chooser's io errors).

### R17 The inline CLI use cases (GHO13, GHO14, M9)
Each holds `Rc<dyn SettingsRepository>` (+ `Rc<dyn SourceChooser>` for select) and returns `Result<_,
GhostTextError>`; the handler prints after:

| Use case | `execute` | Body (from `cli.rs`) |
|---|---|---|
| `SetInlineEnabledUseCase` | `(enabled: bool) -> Result<(), _>` | load, `save(&settings.with(INLINE_ENABLED, Bool(enabled)))` |
| `GetInlineStatusUseCase` | `() -> Result<bool, _>` | `load()?.bool(INLINE_ENABLED)`: the lenient getter, a non-bool reads as enabled (P7-T43) |
| `ListGhostTextSourcesUseCase` | `() -> Result<GhostTextSources, _>` | load once; `selected_source`, `profiles` |
| `SelectGhostTextSourceUseCase` | `(name: Option<String>) -> Result<Option<String>, _>` | `set_customization` minus its prints: **one** load before the chooser, valid profiles only (offered and selectable), `None` chosen → save `without(INLINE_SOURCE)` and `Ok(None)`, unknown → `CustomizationNotFound` before any save, else save and `Ok(Some(name))` (P7-T38) |

- `application/ghost_text_sources.rs`: DTO `GhostTextSources { selected: String, profiles: Vec<(String,
  Result<LlmProfile, String>)> }` (`pub(crate)` fields), so `render_sources(selected, profiles, format, color)` keeps
  its signature and `inline_show_customizations` its input (deviation 13).
- `infrastructure/cli/inline_command.rs`: `InlineCmd` (clap, D4: variants, doc comments, aliases, `#[arg]`s verbatim;
  `pub` → `pub(crate)`), `PROFILE_EXAMPLE`, `run_inline_command(cmd, &InlineCommandModule) -> anyhow::Result<i32>`
  (`color = stdout_tty()` once, first; the same `println!`/`print!` texts, P7-T37), `render_sources` verbatim
  (`cli.rs:321-367`, RC7) with accessor reads.

Rejected: splitting select into list + save (two loads: a concurrent edit during the prompt would survive, P7-T38).

### R18 D6 splits only across modules (TC1/TC2, P6-Q20)
The only Phase 7 split is `ghost_keys_and_words` (`pty.rs:1691-1710`: the `k`, `ctrl`, `alt` setup, 5 `ghost_key`
asserts, 3 `next_word` asserts): the `next_word` asserts keep the name in `ghost_text.rs`; the setup and the five
`ghost_key` asserts become `ghost_keys_and_words_keys` in `pty.rs`. `request_bodies` and `response_parsing` stay whole
(R9).

### R19 Legacy code leaves with its last item; no shim crosses a step's end
| Legacy code | Leaves in | How |
|---|---|---|
| `pty.rs` `next_word`, `ghost_rest` | 7.1 | into `GhostText` |
| `inline.rs` items | 7.2-7.7 | each with its importers switched in the same commit (GHO18); `inline.rs` never re-exports |
| `engine_server.rs` legacy ghost block + `ghost_cache` | 7.6 (local), 7.7 (rest) | replaced by `GhostTextModule` calls |
| `inline.rs` + `mod inline;` | 7.7 | deleted once `llm_complete`/`validate`/`build_request`/`parse_response` are gone |
| `cli.rs` inline section, `InlineCmd`, `profile()` helper, `inline_show_customizations` | 7.9 | moved; unused imports dropped (P7-T44); `cli.rs`'s `//!` stops naming `inline` (CON16); `settings_repository()` stays for `theme` |

`"inline"` stays in `LEGACY_MODULES` (P7-T50). 7.10 deletes nothing; it asserts.

### R20 Module root and main.rs (RC6)
`src/ghost_text/mod.rs` declares `domain` (7.1), `application` and `infrastructure` (7.5); every `mod.rs` holds only
child declarations (fitness rule 6), in alphabetical order by convention (not checked). `main.rs` gains `mod
ghost_text;` in 7.1 and loses `mod inline;` (`main.rs:13`) in 7.7; its declarations (`main.rs:5-27`) are alphabetical by
convention, and rule 7 checks only that it holds attributes, `mod` lines and `fn main`.

### R21 New-test ownership (TC11)
Every new name in "Test moves" has one owner step; each step runs `grep -rn "fn <name>(" src` (empty) before adding
it **[K13]**.

### R22 Registration (7.10)
`"ghost_text"` joins `REGISTERED_MODULES`. Until then every step registers it **locally** (P6-Q33 method) once
`src/ghost_text/` exists (the checker panics on a missing folder): add it, `cargo test --locked architecture` reports
nothing, revert before committing (`git diff src/architecture_tests.rs` empty). Reviewers run the same check on a copy
(`git archive <rev> | tar -x -C <dir>` under `target/hexagonal-refactor/review-tmp/`).

### R23 Phase 6 traps carried (P6-Q32)
M1, M2 → 7.1 (R3; P7-T32, P7-T33); M6 → 7.4 (R7; P7-T15); M7 → 7.4/7.5 (R9: whole-body assertions; P7-T20); M8 → 7.2
(R4; P7-T26); M9 → 7.8 (R17; P7-T43) and `inline-diff.sh`. (P6-Q32 names "7.7" for M8 and M9 under the research's
step numbering; this numbering wins.)

### R24 Per-step rules (gate, lock, evidence)
Every step:
- **gate**: `scripts/hexagonal-refactor/gate.sh` → `GATE PASS` before each commit (P7-T54 for the flaky test);
- **test lock**: `python3 scripts/hexagonal-refactor/testlock.py . ef807c1 --diff` and `… . phase7-start --diff`:
  MISSING 0, DUPLICATE 0, every CHANGED in the step's list ("Test moves") (P7-T53);
- **inline-diff** (from 7.0): `cargo build --locked`, then the diff in "Output-equivalence guard" prints nothing;
- **moves**: by script or `git mv`; a block diff against `git show phase7-start:<file>` shows only paths, visibility,
  glossary names, `self.`/accessor reads and the edits the step lists;
- **fitness**: local registration of `ghost_text` (R22);
- **cfg** (RC3): per touched file the attribute grep `grep -n 'cfg(windows)\|cfg(unix)\|cfg(not(\|cfg(target_os\|cfg(all('`
  returns the same lines as at `phase7-start` (shifted only by the step's edits), and `grep -rn 'cfg!(' src/ghost_text
  src/inline.rs` shows exactly the two `cfg!(windows)` expressions (P7-T46); Windows branches are checked by hand;
- **clippy budget 3**: prune imports as `inline.rs`/`cli.rs` shrink (P7-T44, P7-T49);
- **commits**: `refactor(ghost_text): <why>`, `test(ghost_text): …`, `chore(scripts): …`, at most 72 characters, the
  trailer line, `git commit -F`; new seams RED → GREEN → commit → REFACTOR → commit, `should_…` names, AAA, no mocks;
  never rewrite history (P7-T55);
- **engine steps (7.6, 7.7)**: the engine-server suite 20× (`cargo test --locked bootstrap::engine_server::tests --
  --test-threads=1`), the two 7.0 tests and the three local-ghost tests of 6.0a 50× each by full path (confirm the path
  with `cargo test --locked -- --list`), and `cargo test --locked cli::tests::debug_shell_survives_closed_terminal --
  --exact` alone;
- **per phase**: `cargo test --locked --release -- --ignored all_bundle_modules` prints `1 passed` at the end of each run
  (7.4, 7.7, 7.10); `integrations-diff.sh` and `precmd-diff.sh` stay empty against the baseline binary at the steps that
  touch `doctor.rs`, `pty.rs` or `cli.rs` (7.1, 7.2, 7.9) and at 7.10.

### Engine-server dispatch after 7.7 (`bootstrap/engine_server.rs`)

| `EngineMessage` | Calls, in order | Reply |
|---|---|---|
| `Complete(r)` | `CompleteCommandLineUseCase` → `send_reply` (→ `SuggestLocalGhostTextUseCase`) | `Msg::Completion` unless interrupted |
| idle, generators running | `ProgressGeneratorsUseCase` → `send_reply` (→ `SuggestLocalGhostTextUseCase`) | `Msg::Completion` if `Some` |
| `CommandFinished(e)` | `record_command.execute(e)` → `ClearGhostTextCacheUseCase` → `ResetGeneratorsUseCase` | none |
| `CommandEnded` | `ClearGhostTextCacheUseCase` → `ResetGeneratorsUseCase` | none |
| `Llm { req, buffer, cwd, shell }` | `RequestLlmGhostTextUseCase` (engine thread); `Deferred` → thread `llm` runs the job | `Msg::Ghost { req, text }` (none on spawn failure) |
| others | unchanged (P6 §3.3) | |

### Module map

Layers: **D** domain, **A** application, **I** infrastructure. `pub(crate)` at most; domain fields private.

| File (under `src/ghost_text/`) | Content | From (`ff2db5a` / post-6) | Layer | Step |
|---|---|---|---|---|
| `mod.rs`, `domain/mod.rs`, `domain/value_objects/mod.rs` | child declarations | new | | 7.1 |
| `domain/value_objects/ghost_text.rs` | `GhostText` (R3) | `pty.rs:197-211` | D | 7.1 |
| `domain/value_objects/llm_provider.rs` | `LlmProvider` + `name` | `inline.rs:18-22`; `cli.rs:256-261` | D | 7.2 |
| `domain/value_objects/llm_profile.rs` | `LlmProfile`, `from_setting`, accessors, `#[cfg(test)]` `new` + `LlmProfileParts` | `inline.rs:24-34, 79-123` | D | 7.2 |
| `domain/value_objects/ghost_text_source.rs` | `GhostTextSource`, `BUILTIN_SOURCES` | `inline.rs:36-50` | D | 7.2 |
| `domain/services/{mod.rs, ghost_text_source_resolver.rs}` | `PROFILES_PREFIX`, `profiles`, `source`, `selected_source` | `inline.rs:52-77`; `cli.rs:252-254` | D | 7.2 |
| `domain/entities/{mod.rs, ghost_text_cache.rs}` | `GhostTextCache` | `inline.rs:157-186` | D | 7.3 |
| `domain/services/history_ghost_text.rs` | `history_ghost` | `inline.rs:125-140` | D | 7.3 |
| `domain/services/spec_ghost_text.rs` | `spec_ghost` | `inline.rs:142-155` | D | 7.3 |
| `domain/value_objects/llm_prompt.rs` | `LlmPrompt` | new (GHO10) | D | 7.4 |
| `domain/services/llm_prompt_builder.rs` | `SYSTEM`, `PROMPT_MAX_BYTES`, `PRIVATE_WORDS`, `is_private`, `usable`, `history_prompt`, `llm_history`, `llm_prompt` | `inline.rs:188-200, 212-218, 252-307` | D | 7.4 |
| `domain/services/llm_completion_cleaner.rs` | `clean` | `inline.rs:369-397` | D | 7.4 |
| `domain/services/cd_target_validator.rs` | `cd_target`; private `split_words`, `expand` | `inline.rs:399-412` (target part), 420-504 | D | 7.4 |
| `domain/ghost_text_error.rs` | `GhostTextError` (R16) | `cli.rs` `bail!`s and `?`s | D | 7.8 |
| `application/{mod.rs, ports/mod.rs}` | child declarations | new | | 7.5 |
| `application/ports/llm_client.rs` | `LlmClient` + `ScriptedLlmClient` | new | A | 7.5 |
| `application/ports/directory_probe.rs` | `DirectoryProbe` + `InMemoryDirectoryProbe` | new | A | 7.5 |
| `application/suggest_local_ghost_text_use_case.rs` | R11 | legacy `local_ghost_text` | A | 7.6 |
| `application/clear_ghost_text_cache_use_case.rs` | R13 | legacy `clear_ghost_text_cache` | A | 7.7 |
| `application/request_llm_ghost_text_use_case.rs` | R12 + private `llm_history` (R14) | legacy `request_llm_ghost_text`, `llm_history` | A | 7.7 |
| `application/llm_ghost_text_request.rs` | DTO (R12) | `EngineMessage::Llm` fields | A | 7.7 |
| `application/llm_ghost_text_answer.rs` | `LlmGhostTextAnswer` | new | A | 7.7 |
| `application/llm_ghost_text_job.rs` | `LlmGhostTextJob` + private `validate` | `llm_complete` 212-218, 247-249; the worker closure; `inline::validate` | A | 7.7 |
| `application/ports/source_chooser.rs` | `SourceChooser` + `FixedSourceChooser` | `cli.rs:292` signature (RC7) | A | 7.8 |
| `application/{set_inline_enabled, get_inline_status, list_ghost_text_sources, select_ghost_text_source}_use_case.rs` | R17 | `cli.rs:204-289` minus prints | A | 7.8 |
| `application/ghost_text_sources.rs` | DTO (R17) | `render_sources` args | A | 7.8 |
| `infrastructure/{mod.rs, adapters/mod.rs}` | child declarations | new | | 7.5 |
| `infrastructure/adapters/http_llm_client.rs` | `HttpLlmClient`; private `build_request`, `parse_response` | `inline.rs:219-247, 309-367` | I | 7.5 |
| `infrastructure/adapters/fs_directory_probe.rs` | `FsDirectoryProbe` | `inline.rs:414, 417, 472-474` | I | 7.5 |
| `infrastructure/ghost_text_module.rs` | R15 | engine fields | I | 7.6 (+7.7) |
| `infrastructure/adapters/stdin_source_chooser.rs` | `StdinSourceChooser` | `cli.rs:291-319` (RC7) | I | 7.9 |
| `infrastructure/inline_command_module.rs` | R15 | new | I | 7.9 |
| `infrastructure/cli/{mod.rs, inline_command.rs}` | R17 | `cli.rs:42-59, 202-250, 263-289` (prints), 321-367 (RC7) | I | 7.9 |

Allowed edges used: ghost_text → shared (`Shell`, `logger`, `terminal_style`, `OutputFormat`, `user_home_from_env`),
settings (`Settings`, `SettingValue`, `SettingsRepository`, `SettingsError`, catalog keys), history (`HistoryEntry`,
`HistoryRepository`), specs (`SuggestionKind`), completion (`Suggestion`, `CompletionResult`). `ureq`/`logger` only in
`http_llm_client.rs`; `serde_json` only in `http_llm_client.rs` and `infrastructure/cli/inline_command.rs` (RC4);
`anyhow`, `clap` and output macros only in infrastructure.

Outside `src/ghost_text/`:

| File | Change | Step |
|---|---|---|
| `pty.rs` (legacy) | R3; `llm_enabled` → resolver `source` + `GhostTextSource::Profile(_)` (P7-T35) | 7.1, 7.2 |
| `doctor.rs` (legacy) | `inline_outcome`: resolver `source`, `GhostTextSource`, `p.base_url()`, `p.api_key_env()`; texts and its `Ok(_)` arm unchanged (P7-T45) | 7.2 |
| `cli.rs` (legacy) | 7.2 imports, accessors, `LlmProvider::name`, `selected_source`; 7.9 the inline section leaves (P7-T44) | 7.2, 7.9 |
| `bootstrap/engine_server.rs` | 7.0 two tests (+ the watcher collects `Msg::Ghost`); 7.2-7.4 legacy block paths; 7.6 `send_reply` → use case; 7.7 `Llm`, `CommandFinished`/`CommandEnded` arms → use cases, the block, `ghost_cache` and the `INLINE_*`/`Arc`/`Mutex` imports gone, `spawn` builds `GhostTextModule` | 7.0-7.7 |
| `bootstrap/cli.rs` | `Inline(InlineCmd)` from `inline_command`; the arm per R15 | 7.9 |
| `completion/domain/services/suggestion_filter.rs` | loses `empty_search_term_is_no_prefix_match` | 7.3 |
| `main.rs` | `+ mod ghost_text;` (7.1), `- mod inline;` (7.7) | 7.1, 7.7 |
| `settings/domain/services/settings_catalog.rs` | test doc comments repointed (P7-T56) | 7.7, 7.9 |
| `architecture_tests.rs` | `"ghost_text"` in `REGISTERED_MODULES` | 7.10 |
| `scripts/hexagonal-refactor/inline-diff.sh` | new ("Output-equivalence guard") | 7.0 |

### Output-equivalence guard

Per step, once 7.0 has committed the script:
```
cargo build --locked
diff <(scripts/hexagonal-refactor/inline-diff.sh target/hexagonal-refactor/baseline-tree/target/debug/tabsmith) \
     <(scripts/hexagonal-refactor/inline-diff.sh target/debug/tabsmith)        # prints nothing
scripts/hexagonal-refactor/gate.sh                                              # GATE PASS (golden included)
python3 scripts/hexagonal-refactor/testlock.py . ef807c1 --diff
python3 scripts/hexagonal-refactor/testlock.py . phase7-start --diff
```
The baseline binary is `target/hexagonal-refactor/baseline-tree/target/debug/tabsmith` (`af1b304`, the same `src/` as
`ef807c1`). 7.0 also proves the script deterministic: run it twice on the baseline binary and diff (empty), then diff
the baseline against the Phase 6 end build (empty), and show that the `prun 4 file` scenario selects profile `a` (row
4) and saves it (RC2) before committing.

`scripts/hexagonal-refactor/inline-diff.sh` (7.0 writes exactly this; it already holds RC2's fix in the pty driver:
with stdout in a file the prompt never reaches the pty, so the answer is queued on the tty right after `Popen`; adjust
only a scenario that cannot run, and say so):
```bash
#!/bin/bash
# Usage: inline-diff.sh <tabsmith binary>
# Prints a scrubbed transcript of what the golden capture never shows of `tabsmith inline` and doctor's Inline
# check: valid and invalid `inline.profiles.<name>` (every reason), a selected profile, set-customization by name and
# through the numbered chooser on a pty, a non-bool `inline.enabled`, and every inline command over a corrupted
# settings.json, without a home and with only TABSMITH_SETTINGS_PATH. Phase 7 diffs it at every step:
#   diff <(scripts/hexagonal-refactor/inline-diff.sh target/hexagonal-refactor/baseline-tree/target/debug/tabsmith) \
#        <(scripts/hexagonal-refactor/inline-diff.sh target/debug/tabsmith)
# macOS. Each group starts from a fresh sandbox; commands run with a cleared env (`env -i`) and stdin from /dev/null,
# except the pty group.
BIN="$(cd "$(dirname "$1")" && pwd -P)/$(basename "$1")"
BINDIR="$(dirname "$BIN")"
SANDBOXES=()
cleanup() { for s in "${SANDBOXES[@]}"; do rm -rf "$s"; done; }
trap cleanup EXIT

fresh() {
  local t; t=$(mktemp -d) || exit 1
  SB=$(cd "$t" && pwd -P) || exit 1
  SANDBOXES+=("$SB"); mkdir -p "$SB/home/.tabsmith"; cd "$SB" || exit 1
  echo; echo "======== $1"
}
scrub() { sed -e "s#$SB#\$SANDBOX#g" -e "s#$BIN#\$BIN#g" -e "s#$BINDIR#\$BINDIR#g"; }
show() { scrub <"$1"; if [ -s "$1" ] && [ -n "$(tail -c 1 "$1")" ]; then echo; echo "(no newline at end)"; fi; }
put() { printf '%s' "$1" >"$SB/home/.tabsmith/settings.json"; }   # settings.json holds exactly these bytes
saved() { echo "--- settings.json"
  if [ -e "$SB/home/.tabsmith/settings.json" ]; then show "$SB/home/.tabsmith/settings.json"; else echo "(none)"; fi; }
# run [VAR=value ...] -- <args...>: one command, stdin from /dev/null
run() {
  local envs=(); while [ "$1" != "--" ]; do envs+=("$1"); shift; done; shift
  echo "### ${envs[*]} $*" | scrub
  env -i PATH="$PATH" HOME="$SB/home" TABSMITH_HOME="$SB/home/.tabsmith" USERPROFILE="$SB/home" "${envs[@]}" \
    "$BIN" "$@" </dev/null >"$SB/out" 2>"$SB/err"
  local code=$?
  show "$SB/out"; echo "--- stderr"; show "$SB/err"; echo "--- exit $code"
}
# doctor [VAR=value ...]: the Inline lines of `doctor --all` and its exit code (other checks are not this phase's)
doctor() {
  echo "### $* doctor --all (Inline lines)" | scrub
  env -i PATH="$PATH" HOME="$SB/home" TABSMITH_HOME="$SB/home/.tabsmith" USERPROFILE="$SB/home" "$@" \
    "$BIN" doctor --all </dev/null >"$SB/out" 2>&1
  local code=$?
  grep -E 'Inline|is not set|not a valid profile' "$SB/out" | scrub; echo "--- exit $code"
}
# The pty driver: argv = <answer> <stdout file or -> <stderr file> <command...>. stdin is the pty slave (ECHO off),
# stdout the slave unless a file is given. <answer> (EMPTY an empty line, EOF a Ctrl-D) is written once
# `Select a customization: ` was read from the pty, or right after the start when stdout is a file (the prompt then
# goes to the file; the tty queues the answer until read_line). Prints the pty bytes (\r\n -> \n) and exits with the
# child's code.
PTY='
import os, select, subprocess, sys, termios, time
answer, out_path, err_path, cmd = sys.argv[1], sys.argv[2], sys.argv[3], sys.argv[4:]
pending = {"EMPTY": b"\n", "EOF": b"\x04"}.get(answer, answer.encode() + b"\n")
master, slave = os.openpty()
attrs = termios.tcgetattr(slave)
attrs[3] &= ~termios.ECHO
termios.tcsetattr(slave, termios.TCSANOW, attrs)
out = slave if out_path == "-" else open(out_path, "wb")
err = open(err_path, "wb")
child = subprocess.Popen(cmd, stdin=slave, stdout=out, stderr=err)
os.close(slave)
if out_path != "-":
    os.write(master, pending)
    pending = None
data, deadline = b"", time.time() + 30
while True:
    ready = select.select([master], [], [], 0.05)[0]
    chunk = b""
    if ready:
        try:
            chunk = os.read(master, 65536)
        except OSError:
            chunk = b""
    if chunk:
        data += chunk
        if pending is not None and b"Select a customization: " in data:
            os.write(master, pending)
            pending = None
        continue
    if child.poll() is not None:
        break
    if time.time() > deadline:
        child.kill()
        child.wait()
        data += b"\n(killed: timed out)\n"
        break
sys.stdout.buffer.write(data.replace(b"\r\n", b"\n"))
sys.stdout.flush()
code = child.returncode
sys.exit(code if code >= 0 else 128 - code)
'
# prun <answer> <pty|file> [VAR=value ...] -- <args...>: one command with stdin on a pty (stdout on it, or in a file)
prun() {
  local answer=$1 where=$2; shift 2
  local envs=(); while [ "$1" != "--" ]; do envs+=("$1"); shift; done; shift
  echo "### pty(answer=$answer, stdout=$where) ${envs[*]} $*" | scrub
  local o=-; [ "$where" = file ] && o="$SB/out"
  python3 -c "$PTY" "$answer" "$o" "$SB/err" env -i PATH="$PATH" HOME="$SB/home" \
    TABSMITH_HOME="$SB/home/.tabsmith" USERPROFILE="$SB/home" "${envs[@]}" "$BIN" "$@" >"$SB/pty"
  local code=$?
  echo "--- pty"; show "$SB/pty"
  [ "$where" = file ] && { echo "--- stdout"; show "$SB/out"; }
  echo "--- stderr"; show "$SB/err"; echo "--- exit $code"
}

VALID='"inline.profiles.a":{"provider":"openai","model":"gpt"},"inline.profiles.b":{"provider":"anthropic","model":"claude","effort":"low","base_url":"http://localhost:8080/","api_key_env":"","max_tokens":5,"extra":1},"inline.profiles.c":{"provider":"gemini","model":"x"}'
REASONS='"inline.profiles.history":{"provider":"openai","model":"x"},"inline.profiles.str":"openai","inline.profiles.noprov":{"model":"x"},"inline.profiles.numprov":{"provider":1,"model":"x"},"inline.profiles.nomodel":{"provider":"openai"},"inline.profiles.empty":{"provider":"openai","model":""},"inline.profiles.nummodel":{"provider":"openai","model":2},"inline.profiles.zero":{"provider":"openai","model":"x","max_tokens":0},"inline.profiles.float":{"provider":"openai","model":"x","max_tokens":1.5},"inline.profiles.big":{"provider":"openai","model":"x","max_tokens":4294967296},"inline.profiles.strtok":{"provider":"openai","model":"x","max_tokens":"5"},"inline.profiles.url":{"provider":"openai","model":"x","base_url":1},"inline.profiles.key":{"provider":"openai","model":"x","api_key_env":true},"inline.profiles.eff":{"provider":"openai","model":"x","effort":3},"inline.profiles.nulls":{"provider":"openai","model":"x","base_url":null,"max_tokens":null,"effort":null},"inline.profiles.a.b":{"provider":"openai","model":"dot"},"inline.profiles.":{"provider":"openai","model":"e"}'
CMDS=("status" "enable" "disable" "show-customizations" "show-customizations --format json"
      "set-customization history" "set-customization")

fresh "show-customizations"
for s in "{\"inline.source\":\"b\",$VALID,$REASONS}" "{\"inline.source\":\"c\",$VALID}" '{"inline.source":"nope"}' \
         '{"inline.profiles.c":{"provider":"gemini","model":"x"}}' '{"inline.source":5}' '{}'; do
  put "$s"; run -- inline show-customizations; run -- inline show-sources --format json
done

fresh "set-customization by name"
for name in a b c nope history specs history+specs ''; do
  put "{\"inline.source\":\"a\",\"autocomplete.height\":5,$VALID}"; run -- inline set-customization "$name"; saved
done
put "{$VALID}"; run -- inline set-source b; saved
put "{\"inline.source\":\"a\",$VALID}"; run -- inline set-customization; saved

fresh "the chooser on a pty"
for answer in 1 4 5 6 0 7 x EMPTY EOF; do
  put "{\"inline.source\":\"a\",\"autocomplete.height\":5,$VALID}"; prun "$answer" pty -- inline set-customization; saved
done
put "{$VALID}"; prun 4 file -- inline set-customization; saved
put '{}'; prun 2 pty -- inline set-source; saved

fresh "status, enable, disable"
run -- inline status; run -- inline disable; saved; run -- inline status; run -- inline enable; saved
for v in true false '"yes"' 0 null '{}'; do put "{\"inline.enabled\":$v}"; run -- inline status; done
put '{"inline.enabled":"yes","autocomplete.height":5}'; run -- inline disable; saved

fresh "corrupted settings.json"
for c in "${CMDS[@]}"; do put '{oops'; run -- inline $c; saved; done

fresh "no home"
for c in "${CMDS[@]}"; do run HOME= TABSMITH_HOME= USERPROFILE= -- inline $c; done

fresh "TABSMITH_SETTINGS_PATH only"
for c in "${CMDS[@]}"; do
  run HOME= TABSMITH_HOME= USERPROFILE= "TABSMITH_SETTINGS_PATH=$SB/s.json" -- inline $c
done
echo "--- s.json"; [ -e "$SB/s.json" ] && show "$SB/s.json" || echo "(none)"

fresh "doctor's Inline check"
put '{"inline.enabled":false}'; doctor
put '{"inline.source":"nope"}'; doctor
put "{\"inline.source\":\"c\",$VALID}"; doctor
put "{\"inline.source\":\"a\",$VALID}"; doctor; doctor OPENAI_API_KEY=; doctor OPENAI_API_KEY=k
put "{\"inline.source\":\"b\",$VALID}"; doctor
for u in 'http://127.0.0.1:1/v1' 'http://[::1]:1/v1' 'https://example.invalid/v1'; do
  put "{\"inline.source\":\"l\",\"inline.profiles.l\":{\"provider\":\"openai\",\"model\":\"m\",\"base_url\":\"$u\"}}"
  doctor
done
put '{"inline.enabled":"yes","inline.source":"nope"}'; doctor
```
The script uses only the CLI surface at `ff2db5a`: the aliases `set-source`/`show-sources` (`cli.rs:51, 54`), `--format`
(`-f`, 56), the prompt `Select a customization: ` (310); for `VALID` the rows are 3 builtins + profiles `a`, `b` (`c` is
invalid), so `None` is 6; doctor's texts (`doctor.rs:617, 622, 626`); kind `Soft` when disabled (528-532).

Engine-side guards: 6.0a's three ghost tests and the two 7.0 `Llm` tests (20× as a suite at 7.6 and 7.7, 50× each by
full path); the e2e suite of 7.7 (`ScriptedLlmClient`) and the module test (one shared cache);
`debug_shell_survives_closed_terminal` alone at 7.6, 7.7, 7.10 (`cli.rs:830-832`, `#[cfg(unix)]`, the only automated
run of the real engine spawn, CON-T20); block diffs of `HttpLlmClient::complete` against `llm_complete` 219-247 and of
`LlmGhostTextJob::run` against the worker closure (only `self.` and port calls may differ).

Manual smoke (the user runs it after 7.10, with the baseline binary and the new binary; CON19, O8). With
`TABSMITH_HOME=$(mktemp -d)` and `cargo run -- debug shell zsh`:
1. Plan section 8 item 4: `echo hi`, then `ec` shows `ho hi`, → accepts; Alt-f accepts one word.
2. `tabsmith settings inline.source '"specs"'`: `git ch` ghosts a subcommand; `'"history"'`: only history ghosts.
3. The LLM against a local fake server:
   ```
   python3 -c 'import http.server,json
   class H(http.server.BaseHTTPRequestHandler):
       def do_POST(self):
           self.rfile.read(int(self.headers["Content-Length"])); b=json.dumps({"choices":[{"message":{"content":"tus"}}]}).encode()
           self.send_response(200); self.send_header("Content-Type","application/json"); self.send_header("Content-Length",str(len(b))); self.end_headers(); self.wfile.write(b)
   http.server.HTTPServer(("127.0.0.1",8765),H).serve_forever()'
   ```
   `tabsmith settings inline.profiles.fake '{"provider":"openai","model":"m","base_url":"http://127.0.0.1:8765/v1","api_key_env":"NONE"}'`,
   `tabsmith settings inline.source '"fake"'`; in the debug shell type `zzz sta` → after the debounce the ghost `tus`
   appears; stop the server, run with `-vv`, type again: `pty.log` shows the same `inline: profile 'fake' request to
   http://127.0.0.1:8765/v1/chat/completions failed: …` line for both binaries.

### Deviations from the plan's letter (record them in plan section 9 at 7.10)
1. **Guards.** `inline-diff.sh`, two engine-server `Llm` tests and one pty `Msg::Ghost` test (7.0) precede any move; the local tag `phase7-start`
   is the baseline: 5 tests CHANGED against it, each a restructuring.
2. **`ghost_key` stays in `pty.rs`** (it takes dropdown key types); `ghost_keys_and_words` is D6-split
   (`ghost_keys_and_words_keys` in pty). Phase 9 moves it to `key_router`.
3. **The pty holds `Option<GhostText>`** already; `CompletionResult.ghost` and `Msg::Ghost.text` stay text.
4. **`LlmProfile`** is built by `from_setting` in production; `new(LlmProfileParts)` is test-only. The profile and
   source reasons stay `String` texts.
5. **`LlmPrompt`** carries `SYSTEM`; `build_request` reads it (one CHANGED test).
6. **One adapter `HttpLlmClient`** serves both providers: section 4's `openai_llm_client.rs`/`anthropic_llm_client.rs`
   are not created; `request_bodies`/`response_parsing` stay whole.
7. **The cd check** is a pure domain `cd_target` plus a probe-backed `validate` in the LLM job; `DirectoryProbe` also
   yields the fallback home. Section 4's "cd_target_validator ← validate/split_words/expand (dir probe injected)" is
   read as: the pure half in the domain, the probe in application.
8. **The LLM request** returns `Immediate` or a `Deferred` job; the `llm` thread, its warn and the `Msg::Ghost` sends
   stay in the engine server.
9. **The ghost text cache** is one `Arc<Mutex<GhostTextCache>>` the module hands to two use cases (it crosses the
   `llm` thread), not a driver-owned aggregate.
10. **`SuggestLocalGhostTextUseCase`** takes `&CompletionResult` and runs for every reply (Phase 6 amended the table).
11. **Two module structs:** `GhostTextModule` (engine thread, from the history) and `InlineCommandModule` (CLI, from an
    injected `Rc<dyn SettingsRepository>`).
12. **`GhostTextError`** wraps `SettingsError` and the chooser's io error transparently.
13. **`GhostTextSources`** holds `LlmProfile` results (a DTO exception, like `SettingValue`).
14. **Placements:** `selected_source` in the resolver, `provider_name` as `LlmProvider::name`, the `historyCount`
    wrapper private in the request use case, `empty_search_term_is_no_prefix_match` in `spec_ghost_text.rs`.
15. **Files beyond section 4:** `llm_ghost_text_request.rs`, `llm_ghost_text_answer.rs`, `llm_ghost_text_job.rs`,
    `inline_command_module.rs`, `http_llm_client.rs`; section 4's two client files are not created.
16. **`inline`** stays in `LEGACY_MODULES`.
17. **Eleven steps in three runs** (`7.0` … `7.10`) instead of the plan's four: the plan's step 1 is 7.1-7.4, step 2 is
    7.5 (+ the stdin chooser in 7.9), step 3 is 7.6-7.9, step 4 is 7.6-7.7 (the engine switch and `inline.rs`
    deleted in 7.7, not at the end).
18. **Stale comments repointed** in `settings_catalog.rs` (RC8), as Phase 5 did.

Plan text (7.10, in place **[K10]**): section 4 ghost_text layout as built; section 4 event table `Llm` row →
"`ghost_text::RequestLlmGhostText` on the engine thread; a `Deferred` answer runs on an `llm` worker thread"; section 5
Phase 7 marked done with its research link; section 8 "Manual smoke" gains "after Phase 7: item 4 + the LLM fake-server
check".

### Hand-off notes for later phases
- **Phase 8 (dropdown).** `Key`/`KeyCode`/`Mods` move; `pty.rs`'s `ghost_key` and `ghost_keys_and_words_keys` follow by
  path only. Nothing in ghost_text names dropdown; the ghost reaches render as `&str` (`FrameInput.ghost`).
- **Phase 9 (terminal_session).**
  - `key_router` takes `ghost_key` and `ghost_keys_and_words_keys` (the "accept ghost" `KeyRoute`; whole vs next word
    through `GhostText::next_word`).
  - `TerminalSession` holds the local and LLM `Option<GhostText>`; `Session::ghost_text`'s guards (P7-T33) and the
    local-before-LLM order move with it.
  - `ApplyLlmGhostTextUseCase` replaces the `Msg::Ghost` arm with P7-T32 intact (`req == s.req && req == s.llm_req`,
    keyed by the current `s.sent` through the `zip`, `redraw` without `release_held`); 7.0's `should_show_the_llm_ghost_text_of_the_current_request_behind_the_local_one` pins it.
  - `llm_enabled` = `settings.bool(INLINE_ENABLED) && matches!(source(..), Ok(GhostTextSource::Profile(_)))`; the LLM
    debounce stays in `HandleTimerTick`; the request goes through `CompletionEngine::request_llm_ghost_text`.
- **Phase 10 (diagnostics).** `inline_outcome`/`is_local` read `ghost_text_source_resolver::source` and
  `LlmProfile::{base_url(), api_key_env()}`; when they move into a domain service, the `Ok(_)` arm over
  `GhostTextSource` gets explicit arms (D8).
- **Phase 11.** `LEGACY_MODULES` keeps `inline`. `types::Msg::Ghost` dies with `Msg` in Phase 9.

## Glossary additions (proposed for `docs/ubiquitous-language.md`, ghost_text section; 7.10 writes them)

| Term | Kind | Meaning | Replaces |
|---|---|---|---|
| `GhostText` (edit) | VO | Gray continuation computed for one buffer: what is still to show after the typed buffer (none when empty or holding a control character), and the next word Alt-f accepts. Which key accepts it is key routing (`ghost_key`, in the pty until Phase 9). | `(String, String)` tuples + `pty::{next_word, ghost_rest}` |
| `LlmProfile` / `LlmProvider` (edit) | VO | Provider (`openai`, `anthropic`), model, base URL, API key env var, max tokens, effort; read from an `inline.profiles.<name>` setting, or a reason text when invalid. | `Profile` + `parse_profile`, `Provider`, `cli::provider_name` |
| `LlmPrompt` (edit) | VO | System + user text sent to the provider: the OS, shell and cwd above the privacy-filtered, numbered history. | `SYSTEM` + `history_prompt` output |
| `GhostTextError` | Domain error | Settings (the settings error, printed as itself), customization not found, no customization selected, source chooser (the io error, printed as itself). | the `bail!`s and `?`s of `cli::inline` |
| `GhostTextSourceResolver` | Domain service | The profiles (sorted by name, each a profile or its reason), the selected source and its name. | `inline::{profiles, source}`, `cli::selected_source` |
| `CdTargetValidator` | Domain service | The directory a `cd` completion would change into, if any (quotes, `~` and `$VAR` expanded from the env). | the target half of `inline::validate`, `split_words`, `expand` |
| `LlmPromptBuilder` | Domain service | Which history lines the LLM may see (finished, not private, the last `historyCount`) and the prompt built from them (10 240-byte budget, none for a private buffer). | `llm_history`, `is_private`, `usable`, `history_prompt` |
| `LlmCompletionCleaner` | Domain service | The completion text kept from a provider reply (first usable line, unquoted, number and buffer stripped; none with a control character). | `inline::clean` |
| `LlmClient` | Port | Sends an `LlmPrompt` to a profile's provider; the raw completion text or none (transport failures are logged by the adapter). | the HTTP half of `llm_complete` |
| `DirectoryProbe` | Port | Is a path a directory; does it resolve to one; the home directory fallback. | `Path::is_dir`, `canonicalize`, `user_home_from_env` in `validate`/`expand` |
| `SourceChooser` | Port | Asks the user to pick one of the valid profiles, or none. | `cli::choose_source` |
| `GhostTextSources` | DTO | The selected source and every profile or its reason, for `inline show-customizations`. | `render_sources`' arguments |
| `LlmGhostTextRequest` | DTO | Buffer, cwd and shell of an LLM ghost-text request. | `EngineMsg::Llm` fields |
| `LlmGhostTextAnswer` | Application type | An LLM ghost text known at once (or none), or a job to run off the engine thread. | the two early sends and the spawn of `engine::llm` |
| `LlmGhostTextJob` | Application type | The worker half of an LLM request: prompt, provider call, clean, cd check, cache insert. | the `llm` worker closure + `llm_complete` |

Rows to edit: the "prompt" word row gains the `inline set-customization` chooser prompt (`Select a customization: `,
`StdinSourceChooser`; no domain type). `HttpLlmClient` is named in prose ("`LlmClient` … `HttpLlmClient` implements it for
both providers"). No row: `FsDirectoryProbe`, `StdinSourceChooser`, `ScriptedLlmClient`, `InMemoryDirectoryProbe`,
`FixedSourceChooser`, `GhostTextModule`, `InlineCommandModule`, `LlmProfileParts`.

## Test moves

"path" means testlock's loose form shows no change (P7-T53); "CHANGED" means a justified restructuring: the same
scenario and the same assertions. Find tests by name; the line numbers mix `#[test]` lines (`inline.rs`) and fn lines
(`pty.rs`, `cli.rs`) (RC7).

| Test | Now (`ff2db5a` / post-6) | Destination | Step | Kind |
|---|---|---|---|---|
| `cache_lookup_vector` | `inline.rs:539` | `domain/entities/ghost_text_cache.rs` | 7.3 | path (`GhostCache::default()` → `GhostTextCache::default()`; `c.entries` is private, same file) |
| `cache_insert_priorities` | `inline.rs:567` | same | 7.3 | path |
| `prompt_numbering_and_filters` | `inline.rs:577` | `domain/services/llm_prompt_builder.rs` | 7.4 | path (`entry()` copied, `HistoryEntryPrimitives`) |
| `prompt_truncation` | `inline.rs:594` | same | 7.4 | path |
| `llm_history_snapshot` | `inline.rs:616` | same | 7.4 | path |
| `cleaning` | `inline.rs:630` | `domain/services/llm_completion_cleaner.rs` | 7.4 | path |
| `request_bodies` | `inline.rs:662` | `infrastructure/adapters/http_llm_client.rs` | 7.4 CHANGED in place; 7.5 moves | CHANGED: the five `"U"` → `&prompt("U")` (R7); helper `profile(provider, effort)` → `LlmProfileParts` (7.2; helpers are not locked), `"http://x/"` keeps its slash |
| `response_parsing` | `inline.rs:702` | same | 7.5 | path (`Provider::` → `LlmProvider::`) |
| `cd_validation` | `inline.rs:726` | `application/llm_ghost_text_job.rs` | 7.5 CHANGED in place; 7.7 moves | CHANGED: `validate(c, &d, &env)` → `validate(c, &d, &env, &FsDirectoryProbe)` |
| `history_ghost_rules` | `inline.rs:774` | `domain/services/history_ghost_text.rs` | 7.3 | path (`entry()` copied) |
| `spec_ghost_rules` | `inline.rs:823` | `domain/services/spec_ghost_text.rs` | 7.3 | path (CHANGED by 6.2b against `ef807c1`, not against `phase7-start`; helper `item()` moves) **[K9]** |
| `profiles_and_source` | `inline.rs:855` | `domain/services/ghost_text_source_resolver.rs` | 7.2 | CHANGED: the `Profile { .. }` literal (879-887) → `LlmProfile::new(LlmProfileParts { .. })` (same names and values); `local.base_url.as_str()` etc. (892-894, 913) → accessors; helper `settings(json!)` kept (D2) |
| `ghost_keys_and_words` | `pty.rs:1692` | `domain/value_objects/ghost_text.rs` | 7.1 | split (D6, R18): the 3 `next_word` asserts as `GhostText::next_word` keep the name |
| `ghost_keys_and_words_keys` (new half) | — | `pty.rs` | 7.1 | the `k`/`ctrl`/`alt` setup + the 5 `ghost_key` asserts |
| `ghost_with_control_chars_is_never_shown_or_accepted` | `pty.rs:1713` | `domain/value_objects/ghost_text.rs` | 7.1 | CHANGED: `ghost_rest(buffer, base, g)` → `GhostText::new(base.into(), g.into()).rest_after(buffer)`, all seven argument triples (1716-1727) kept |
| `llm_ghost_requested_for_commands_without_spec` | `pty.rs:1995` | stays | — | none (production `llm_enabled` switches) |
| `inline_show_customizations` | `cli.rs:700` | `infrastructure/cli/inline_command.rs` | 7.9 | path (`render_sources` keeps its signature, so `Err("model missing".into())` stays; helper `profile(name)` → `LlmProfileParts` in 7.2) |
| `llm_history_honours_history_count` | `engine.rs:1335` (post-6: `engine_server.rs`) | `application/request_llm_ghost_text_use_case.rs` | 7.7 | path (the wrapper keeps name and signature) |
| `empty_search_term_is_no_prefix_match` | `suggest.rs:1603` (post-6: `suggestion_filter.rs`) | `domain/services/spec_ghost_text.rs` | 7.3 | path (`crate::inline::spec_ghost` → `spec_ghost`; `opts`/`ctx` imported from completion's `test_fixtures`) **[K5]** |
| the twelve 6.0a engine-server tests (three pin the local ghost) | `engine_server.rs` | stay | — | none; their helper builds the server with `GhostTextModule` from 7.6 (helpers are not locked) **[K6]** |

Helpers that move: `entry()` (copied into the prompt-builder and history-ghost test modules), `settings(Value)` (the
resolver tests), `profile(provider, effort)` (the HTTP client tests), `profile(name)` (the inline command tests),
`item()` (the spec ghost tests).

New tests (one owner step each; `grep -rn "fn <name>(" src` empty before adding it, R21; test-first, `should_…`, AAA, no
mocks):

| Step | File | Tests |
|---|---|---|
| 7.0 | `bootstrap/engine_server.rs` | `should_answer_an_llm_request_at_once_with_no_ghost_text_for_a_source_that_is_not_a_profile`, `should_answer_an_llm_request_with_no_ghost_text_when_the_provider_cannot_be_reached` (`base_url` `http://127.0.0.1:9/v1`, the closed port of `llm_ghost_requested_for_commands_without_spec`, `pty.rs:1997`; the only automated run of the `llm` thread → HTTP → reply path; a refused localhost connection, bounded by the harness's 10 s watcher) **[K6]** |
| 7.0 | `pty.rs` | `should_show_the_llm_ghost_text_of_the_current_request_behind_the_local_one` (P7-T32, P7-T33: the only test that feeds `Msg::Ghost`; `session("{}")`, `prompt("git sta")`, `s.llm_req = s.req`, then through `Session::step`: the current request's `Some("tus")` shows `tus`, another request's reply is ignored, `None` clears, and a local `reply(.., Some("sh"))` wins; asserts only `ghost_text(false)`, so 7.1 leaves it byte-identical) |
| 7.1 | `pty.rs` | `ghost_keys_and_words_keys` (the split half) |
| 7.4 | `llm_prompt_builder.rs` | `should_send_the_os_shell_and_cwd_above_the_numbered_history` (P7-T16; `system` is `SYSTEM`) |
| 7.4 | `cd_target_validator.rs` | `should_read_the_fallback_home_only_for_a_tilde_target_the_env_does_not_resolve` (P7-T25 laziness: a counting closure) |
| 7.5 | `llm_client.rs` | `should_answer_every_prompt_with_its_scripted_reply`, `should_remember_each_prompt_it_was_sent` |
| 7.5 | `directory_probe.rs` | `should_know_only_the_directories_it_was_given`, `should_have_no_home_unless_given_one` |
| 7.6 | `suggest_local_ghost_text_use_case.rs` (characterization of P7-T1…T3; must pass by moving `local_ghost_text`'s body with the D8 arms) | `should_suggest_the_rest_of_the_newest_successful_history_command`, `should_fall_back_to_the_first_spec_suggestion_when_no_history_command_extends_the_buffer`, `should_suggest_only_specs_for_the_specs_source`, `should_suggest_only_history_for_an_llm_profile_source`, `should_fall_back_from_history_to_specs_for_a_source_that_names_no_valid_profile`, `should_suggest_nothing_while_inline_is_disabled`, `should_suggest_nothing_when_the_cursor_is_not_at_the_end` |
| 7.7 | `clear_ghost_text_cache_use_case.rs` | `should_forget_every_cached_completion` |
| 7.7 | `request_llm_ghost_text_use_case.rs` (plan Phase 7 step 3: the first end-to-end test of `llm_complete`, over `ScriptedLlmClient`; the test thread runs the job) | `should_suggest_the_cleaned_completion_the_profile_returns` (reply `git status` for `git sta` → `tus`), `should_answer_from_the_cache_without_asking_again`, `should_answer_nothing_at_once_for_a_source_that_is_not_a_profile`, `should_not_ask_about_a_private_buffer`, `should_drop_a_cd_into_a_directory_that_does_not_exist`, `should_send_the_os_shell_cwd_and_numbered_history`, `should_not_cache_a_reply_that_suggests_nothing` |
| 7.7 | `ghost_text_module.rs` | `should_clear_the_completions_the_llm_request_cached` (one shared cache, R13) |
| 7.8 | `ghost_text_error.rs` | `should_print_a_settings_failure_exactly_as_the_settings_error` (anyhow `{:#}` with and without the wrapper, a storage error with a cause), `should_print_a_chooser_failure_exactly_as_its_io_error`, `should_print_an_unknown_customization_as_the_cli_did` |
| 7.8 | `source_chooser.rs` | `should_choose_what_it_was_given`, `should_remember_the_profiles_it_was_offered` |
| 7.8 | `set_inline_enabled_use_case.rs` | `should_save_the_inline_switch`, `should_not_save_the_inline_switch_over_settings_that_cannot_be_loaded` |
| 7.8 | `get_inline_status_use_case.rs` | `should_read_inline_as_enabled_when_unset`, `should_read_a_non_boolean_inline_switch_as_enabled` (P7-T43) |
| 7.8 | `list_ghost_text_sources_use_case.rs` | `should_list_the_selected_source_and_each_profile_or_its_reason` |
| 7.8 | `select_ghost_text_source_use_case.rs` | `should_select_a_builtin_source_by_name`, `should_select_a_valid_profile_by_name`, `should_refuse_a_source_that_is_neither_builtin_nor_a_valid_profile`, `should_unset_the_source_when_none_is_chosen`, `should_offer_the_chooser_only_the_valid_profiles`, `should_not_save_a_source_over_settings_that_cannot_be_loaded` |

Use-case tests build settings with `settings_from_json(json!({..}).as_object().unwrap().clone())` (the signature takes a
`Map<String, Value>`, `setting_value_json.rs:7`; as `inline.rs:523-525` and `engine.rs:1343-1349` do) or a local
`settings(Value)` helper (RC5); `Suggestion`s and `CompletionResult`s through Phase 6's `…Parts` **[K2]**; the CLI use
cases over `InMemorySettingsRepository`/`UnreadableSettingsRepository` (with `save_attempted()`,
`settings_repository.rs:17-71`); and they share doubles through `Arc`/`Rc` clones to inspect them. No fixtures file.

**Totals against `phase7-start`:** 5 CHANGED (`ghost_keys_and_words`, `ghost_with_control_chars_is_never_shown_or_
accepted`, `profiles_and_source`, `request_bodies`, `cd_validation`), each a restructuring; MISSING 0; DUPLICATE 0;
**42 NEW** (the table above, the split half included). The pass count only grows.

Guards that must stay green without edits: pty `insertions_build_on_keys_not_yet_echoed` (1956: `l` + Right accepts only
the rest of the ghost), `keys_after_an_unmodelled_key_wait_for_its_echo` (2183),
`slow_echo_keeps_the_insertion_in_the_basis` (2215), `llm_ghost_requested_for_commands_without_spec` (1995), and from
7.0 on `should_show_the_llm_ghost_text_of_the_current_request_behind_the_local_one`; doctor
`doctor_group_soft_fix_abort` (`doctor.rs:817`); the settings catalog's `inline.*` default tests
(`settings_catalog.rs:335-360, 555-560`); `debug_shell_survives_closed_terminal`; 6.0a's twelve engine-server tests.

## Step briefs

The full prompts are in `phase-7-ghost-text.args.json` (ids below). Each implementer reads `agent-brief.md`, then this
file and the source maps named in its prompt, and loads the skills `xp-tdd`, `testing-standard`, `coding-standards`
and `hexagonal-architecture`. Every step ends with:
- the full gate green and everything committed (R24);
- `testlock.py . ef807c1 --diff` and `testlock.py . phase7-start --diff`: MISSING 0, DUPLICATE 0, exactly the step's
  CHANGED tests, each justified;
- from 7.0, the `inline-diff.sh` diff empty;
- once `src/ghost_text/` exists and until 7.10, the local fitness run with `ghost_text` registered, reverted before
  each commit (R22);
- the cfg check (RC3), the clippy budget, `should_…` names grepped first (R21);
- no rewritten history, subjects of at most 72 characters, and a lone `integrations_zsh_powerlevel10k_prompt_wrapped`
  failure rerun alone before the gate is run again (P7-T54).

Each step compiles on its own: no shim crosses a step's end (R19), and every legacy importer of an item switches in
the step that moves it.

### Run 1: guards and the domain (7.0-7.4)
Ends with the ghost_text domain complete except `GhostTextError`; `inline.rs` holding only `llm_complete`, `validate`,
`build_request`, `parse_response` and the tests `request_bodies`, `response_parsing`, `cd_validation`;
`all_bundle_modules` `1 passed`.

| Id | Title | Plan step | What it does |
|---|---|---|---|
| 7.0 | Guards before the move | — (section 8) | `chore(scripts)`: `inline-diff.sh` exactly as above, determinism and baseline-vs-Phase-6 diffs empty, the `prun 4 file` transcript selects `a` (RC2); `test(ghost_text)`: the two `Llm` tests through the 6.0a harness (its watcher also collects `Msg::Ghost`; the 12 bodies untouched) **[K6]** and the pty's `Msg::Ghost` test (P7-T32, P7-T33); engine-server suite 20× with no production change (R1) |
| 7.1 | `GhostText` from the pty | 1 | module root, `mod ghost_text;`, `ghost_text.rs`; pty fields and the four sites (P7-T29…T34, M1, M2); `next_word`, `ghost_rest` deleted; the D6 split and the CHANGED control-char test (R3, R18, R20) |
| 7.2 | Profiles and sources | 1 | `llm_provider.rs`, `llm_profile.rs`, `ghost_text_source.rs`, `ghost_text_source_resolver.rs`; importers in the same commit: `inline.rs` (accessors in `build_request`/`parse_response`/`llm_complete`, test helper `profile()` → `LlmProfileParts`), `cli.rs` (imports, `render_sources`, `choose_source`, `set_customization`, `selected_source`, `provider_name` → `name()`, helper `profile(name)`), `doctor.rs`, `pty.rs` `llm_enabled`, the server's legacy block; M8 (R4, R5; P7-T26…T28, T35, T45) |
| 7.3 | The cache and the local ghost services | 1 | `ghost_text_cache.rs`, `history_ghost_text.rs`, `spec_ghost_text.rs`; the server's legacy block uses them and `Arc<Mutex<GhostTextCache>>`; `empty_search_term_is_no_prefix_match` moves (R6; P7-T4, T5, T14) |
| 7.4 | Prompt, cleaner, cd target | 1 | `llm_prompt.rs`, `llm_prompt_builder.rs`, `llm_completion_cleaner.rs`, `cd_target_validator.rs`; `inline::llm_complete` calls `llm_prompt` and `clean`, `build_request(profile, &prompt, key)`; `inline::validate` calls `cd_target(.., \|\| user_home_from_env().ok().map(..))` and keeps its two fs calls; the server's `llm_history` wrapper calls the builder's; M6, M7 (R7, R8; P7-T15, T16, T20, T22-T25) |

### Run 2: ports, adapters and the engine switch (7.5-7.7)
Ends with the engine server dispatching to ghost_text use cases; `inline.rs` and `mod inline;` gone;
`all_bundle_modules` `1 passed`.

| Id | Title | Plan step | What it does |
|---|---|---|---|
| 7.5 | Ports with their doubles, the HTTP client, the fs probe | 2 | TDD `llm_client.rs` + `ScriptedLlmClient`, `directory_probe.rs` + `InMemoryDirectoryProbe`; move `http_llm_client.rs` (warn text byte-identical: `grep -rn 'request to {url} failed' src`) and `fs_directory_probe.rs`; `inline::llm_complete` = `llm_prompt` → `HttpLlmClient.complete` → `clean` → `validate(.., &FsDirectoryProbe)`; `inline::validate(full, cwd, env, directories)` (R8, R9, R10; P7-T17…T21, T23) |
| 7.6 | The local ghost through its use case | 3, 4 | TDD (characterization) `suggest_local_ghost_text_use_case.rs`; `ghost_text_module.rs` with it; `spawn` builds the module after the history module; `send_reply` calls the use case; `local_ghost_text` deleted (the block keeps the LLM half and the clear); P7-T7's proof in the commit body (R11, R15) **[K1, K2, K7, K8]** |
| 7.7 | The LLM request and the cache clear (alone) | 3, 4 | TDD e2e over `ScriptedLlmClient`, `InMemoryDirectoryProbe`, `InMemoryHistoryRepository`; GREEN by moving: the legacy handler's engine half → `execute`, `llm_complete`'s rest + the worker insert → `LlmGhostTextJob::run`, `inline::validate` + `cd_validation` → the job file, `llm_history` + its test → the use case file; `GhostTextModule::new`; server: `Llm` arm per R12, the clear in both command arms, the legacy block, `ghost_cache` and the `INLINE_*` imports deleted; `inline.rs` + `mod inline;` deleted; `settings_catalog.rs:335` repointed; `grep -rn 'crate::inline\|mod inline' src` empty, per-fn block diffs (R12-R14, R19; P7-T9…T13) **[K7, K16]** |

### Run 3: the inline command and the close (7.8-7.10)
Ends with `cli.rs` free of `inline`; `ghost_text` registered; Phase 7 recorded.

| Id | Title | Plan step | What it does |
|---|---|---|---|
| 7.8 | Inline use cases (no production caller yet) | 3 | TDD `ghost_text_error.rs`, `source_chooser.rs` + `FixedSourceChooser`, the four use cases (over `InMemorySettingsRepository`/`UnreadableSettingsRepository`), `ghost_text_sources.rs`; M9 (R16, R17, R10; P7-T36, T38, T41, T43) |
| 7.9 | The inline command moves | 2, 3 | `stdin_source_chooser.rs`, `inline_command_module.rs`, `infrastructure/cli/{mod.rs, inline_command.rs}`; `bootstrap/cli.rs`; `cli.rs` loses the section, `InlineCmd`, `PROFILE_EXAMPLE`, the helper and the test, drops `Settings`, `INLINE_ENABLED`, `INLINE_SOURCE`, `IsTerminal` (and any other import clippy reports unused), `//!` rewritten; `settings_catalog.rs:355, 555` repointed (R15, R17, R19; P7-T36…T44, T56) **[K14, K15]** |
| 7.10 | Register `ghost_text` and record the phase | 4 | R22; the greps below; plan section 9 "Phase 7 notes" (the deviation list), section 4 layout and event table, section 5 status, section 8 smoke line; glossary rows; README and handoff status; `cargo test --locked --release -- --ignored all_bundle_modules`; `cargo check --locked --no-default-features --all-targets`; the orchestrator asks the user for the manual smoke **[K10]** |

Per-step guards and test-lock expectations ("CHANGED" = changed for the first time against `phase7-start`):

| Id | Guards (on top of every test) | New tests | CHANGED |
|---|---|---|---|
| 7.0 | engine-server suite 20× with no production change; inline-diff baseline vs baseline and vs the current build | the two `Llm` tests; `should_show_the_llm_ghost_text_of_the_current_request_behind_the_local_one` | none |
| 7.1 | `insertions_build_on_keys_not_yet_echoed`, `keys_after_an_unmodelled_key_wait_for_its_echo`, `slow_echo_keeps_the_insertion_in_the_basis`, `llm_ghost_requested_for_commands_without_spec`, 7.0's `should_show_the_llm_ghost_text_of_the_current_request_behind_the_local_one` (the `Msg::Ghost` arm, local before LLM), all pty tests; precmd-diff | `ghost_keys_and_words_keys` | `ghost_keys_and_words` (split), `ghost_with_control_chars_is_never_shown_or_accepted` |
| 7.2 | golden: the 13 `inline` captures and the 3 `doctor --all` captures (RC1); inline-diff; integrations-diff and precmd-diff; `inline_show_customizations`, `request_bodies`, `response_parsing` (helpers only), `llm_ghost_requested_for_commands_without_spec` | none | `profiles_and_source` |
| 7.3 | the 3 local-ghost tests of 6.0a, `spec_ghost_rules`, `history_ghost_rules` | none | none |
| 7.4 | `request_bodies`, `cd_validation` (body unchanged), `llm_history_honours_history_count`; `all_bundle_modules` | `should_send_the_os_shell_and_cwd_above_the_numbered_history`, `should_read_the_fallback_home_only_for_a_tilde_target_the_env_does_not_resolve` | `request_bodies` |
| 7.5 | `request_bodies`, `response_parsing` (whole, path), the 7.0 provider test | the 4 port-double tests | `cd_validation` |
| 7.6 | the 3 local-ghost tests of 6.0a (50× by full path), engine-server suite 20×, `debug_shell_survives_closed_terminal` alone | the 7 local-ghost tests | none |
| 7.7 | the two 7.0 tests (50× by full path), engine-server suite 20×, `debug_shell_survives_closed_terminal` alone, `llm_ghost_requested_for_commands_without_spec`; `all_bundle_modules` | clear 1 + request 7 + module 1 | none (`cd_validation`, `llm_history_honours_history_count` move path-only) |
| 7.8 | inline-diff, golden | the 16 CLI tests | none |
| 7.9 | golden: the 13 `inline` captures (help included), `completion bash/zsh/fish` (`golden.sh:95-97`) (RC1); inline-diff (the pty chooser group); integrations-diff and precmd-diff | none | none (`inline_show_customizations` moves path-only) |
| 7.10 | full gate; testlock vs `ef807c1` and `phase7-start`; inline-diff, integrations-diff, precmd-diff; `debug_shell_survives_closed_terminal` alone; `all_bundle_modules` | none | none |

**7.10 greps** (empty outside `#[cfg(test)]` unless said; RC4):
- `grep -rnE 'crate::(types|engine|parser|suggest|pty|inline|menu|render|icons|doctor|cli|tracker|keys|shell_parser|js|spec)\b' src/ghost_text`;
- `grep -rnE 'crate::(shell_integration|dropdown|terminal_session|diagnostics|bootstrap)\b' src/ghost_text`;
- `grep -rn 'use crate::{' src/ghost_text` reviewed by hand; `grep -rn 'pub(crate) use' src/ghost_text src/pty.rs src/cli.rs src/doctor.rs` empty (no shim);
- `grep -rnE '\b(anyhow|clap)\b' src/ghost_text`: only `infrastructure/cli` (and `ghost_text_error.rs` tests);
- `grep -rnE '\bureq\b|logger' src/ghost_text`: only `infrastructure/adapters/http_llm_client.rs`;
- `grep -rnE '\bserde_json\b|json!' src/ghost_text`: only `http_llm_client.rs`, `infrastructure/cli/inline_command.rs` (`render_sources` builds rows with `json!` and passes `OutputFormat::render(.., json: impl FnOnce() -> Value)`) and test modules;
- `grep -rnE '\b(e?print(ln)?|dbg)!' src/ghost_text`: only `infrastructure/cli` and `stdin_source_chooser.rs`;
- `grep -rn 'crate::inline\|GhostCache\b\|fn ghost_rest\|fn next_word(g: &str)' src`: only `GhostText::next_word` (the
  completion tokenizer's `fn next_word(&self, i: usize)`, moved verbatim from `shell_parser.rs:242` by Phase 6, is not
  matched);
- `grep -rn 'cfg!(' src/ghost_text`: the two `cfg!(windows)` in `cd_target_validator.rs`.

## Open questions (for the plan owner; the default applies if nobody answers)

1. **O1 One `HttpLlmClient` or the plan's two client files?** *Default: one (R9); a recorded deviation.*
2. **O2 `LlmPrompt` carries `SYSTEM` (1 CHANGED test) or only the user text (path-only, `system` unused)?** *Default:
   it carries it (R7).*
3. **O3 Two module structs or one?** *Default: two (R15).*
4. **O4 A `GhostTextError` wrapper or a bare `SettingsError`?** *Default: the wrapper (R16, P6-Q29).*
5. **O5 Keep the closed-port engine test** (network-stack dependent, up to about 5 s on a filtering host)? *Default:
   keep it: it is the only automated run of the `llm` thread; drop it only if CI shows it flaky.*
6. **O6 Move the pty's ghost tuples to `GhostText` now or in Phase 9?** *Default: now (plan Phase 7 step 1; R3).*
7. **O7 `SuggestLocalGhostTextUseCase` takes `&CompletionResult` or `(buffer, cursor, suggestions)`?** *Default:
   `&CompletionResult` (R11); switch to the three values if K2 makes tests build results awkwardly.*
8. **O8 Ask the user for the manual smoke after 7.7 as well as after 7.10?** *Default: once, after 7.10.*
9. **O9 Should `ScriptedLlmClient` also record the env it was given?** *Default: no: the key lookup stays verbatim in
   the adapter.*

## Launch

From the repo root, with the gate green at the end of Phase 6:
1. Re-check the DEFERRED items ("Re-check at `phase7-start`", K1-K15 and RC8) against the end of Phase 6 and correct
   this file, the step prompts and `target/hexagonal-refactor/prep7/review-notes.md` where they differ. Re-run the K13
   grep for all 42 names.
2. Commit the prep docs: this file and `docs/hexagonal-refactor/phase-7-ghost-text.args.json` (its `reviewNotes` =
   `target/hexagonal-refactor/prep7/review-notes.md`), as `docs: hand Phase 7 over with its research` (as `6f38da1` did
   for Phase 4). The step prompts point there, and 7.0's step 0 stops on a dirty `git status`.
3. Tag that commit: `git tag phase7-start` (local, lightweight, never pushed). It is the testlock baseline of every
   step (`testlock.py . phase7-start --diff`); it changes only docs on top of the end of Phase 6. `startRev` is that
   commit.
4. Run the workflow once per run, each time with the args file's `phase`, `reviewNotes` and the steps of that run only:
   run 1 = `7.0`, `7.1`, `7.2`, `7.3`, `7.4`; run 2 = `7.5`, `7.6`, `7.7`; run 3 = `7.8`, `7.9`, `7.10`. Before runs 2
   and 3 the orchestrator writes a binding "SINCE" preamble into each step prompt of the run (what the earlier runs and
   their reviews left: layout, signatures as built, gate/testlock/inline-diff numbers, accepted deviations; as
   `target/hexagonal-refactor/phase5-run4-since.md` did), and a "SCOPE OF THIS RUN" preamble for the reviewers (judge
   only this run's steps; do not demand later runs' work; the state the run starts from).

```js
Workflow({
  scriptPath: "docs/hexagonal-refactor/phase-workflow.js",
  args: /* phase-7-ghost-text.args.json with startRev = phase7-start (run 1) or the previous run's end (runs 2-3),
           and steps = the run's steps, each prompt prefixed with that run's SINCE preamble */
})
```

---

## Re-check results at `058bebd` (binding; they win over every K row, brief and trap above)

Four read-only auditors compared K1-K16, RC8 and every code citation in the 11 step briefs with the real end of Phase 6
(`058bebd`; `src/` is identical to `7f48161`). No claim blocks a step. Every line number cited above is an `ff2db5a`
value and has shifted: locate every site by symbol or content. The corrections below are binding. Where a brief says
otherwise, this section wins. The full audit is `target/hexagonal-refactor/prep7/recheck-p6.json`.

### State at `phase7-start` (K12, K11)
- Baseline: **943 passed, 0 failed, 1 ignored**, clippy **3** warnings (budget 3, all used), golden identical, testlock
  against `ef807c1`: MISSING 0, DUPLICATE 0, CHANGED 150. The final CHANGED count against `ef807c1` is 150 plus
  the new CHANGED tests of this phase (the briefs expect 5), unless one of the five was already CHANGED.
- `REGISTERED_MODULES` = completion, history, settings, shared, shell_integration, specs. `LEGACY_MODULES` holds `"inline"`.
- Commit trailer for this phase: `Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>` (the model that writes the
  commit; earlier phases carry the Opus trailer, and the briefs' "Opus 5.5" text is superseded).
- Code baseline until the tag exists: `058bebd`. `phase7-start` is created by the hand-over commit, which changes docs only.

### The engine server's tests (K6; briefs 7.0, 7.6, 7.7)
- The engine-server tests are NOT inline. `src/bootstrap/engine_server.rs` ends with `#[cfg(test)] mod tests;` and the
  14 tests, the harness and the helpers live in **`src/bootstrap/engine_server/tests.rs`** (`use super::*;`). The module
  path is still `bootstrap::engine_server::tests::<name>`. Read every "engine_server.rs's tests" as that file. 7.0's evidence
  is `git diff phase7-start -- src` touching only `src/bootstrap/engine_server/tests.rs` and the test module of `src/pty.rs`.
  `llm_history_honours_history_count` is `tests.rs:20` and reaches `llm_history` through `use super::*`.
- Harness signatures: `serve(settings: Settings, history: Vec<HistoryEntry>, first: Vec<EngineMessage>, grace: Duration,
  react: impl FnMut(&[Reply]) -> Then + Send + 'static) -> Vec<Reply>`; `enum Then { Wait, Deliver(Vec<EngineMessage>),
  Stop }`; `stop_when(done) -> impl FnMut(&[Reply]) -> Then`; `SERVE_LIMIT` 10 s; `apply_settings(Settings)` queues an
  `EngineMessage::Settings`. Settings are the FIRST ARGUMENT of `serve`. History is `InMemoryHistoryRepository::new(history)`
  (no TempDir, no `h.jsonl`; only `engine_over(&TempDir)` uses `JsonlHistoryRepository`). There is no harness tmp dir: take `cwd` from
  `TempDir::new("engine-llm")` or `spec_dir(tag, arg)`.
- **The watcher does NOT collect `Msg::Ghost`**: it drops it at `Ok(_) => {} // LLM ghost text` and in the final drain, and `react`
  fires only after a `Msg::Completion`. 7.0 must factor the watcher so a sibling helper also collects `Msg::Ghost` as
  `(u64, Option<String>)` with its own stop predicate, keeping `serve`'s signature and the 12 existing bodies byte-identical.
  `EngineMessage::Llm { req: u64, buffer: String, cwd: PathBuf, shell: Shell }` is in `bootstrap/engine_message.rs`;
  `Msg::Ghost { req: u64, text: Option<String> }`.

### `EngineServer` construction (briefs 7.6, 7.7)
- There is no `EngineServer { .. }` literal outside `EngineServer::new(replies, session, completion, history_module)`
  (`engine_server.rs`), which destructures `HistoryModule { history, record_command }` and builds `ghost_cache` itself. It has
  exactly three callers: `spawn`, and `tests.rs` `engine_over` and `serve`.
- 7.6: add a `ghost_text: GhostTextModule` parameter to `new`, store it in the new field, and at all three call sites
  compute `let ghost_text = build_ghost_text_module(Rc::clone(&history_module.history));` BEFORE `history_module` is moved into `new`.
  In `send_reply` replace only the right-hand side of `let ghost = self.local_ghost_text(&result);` with
  `let ghost = self.ghost_text.suggest_local_ghost_text.execute(self.session.settings(), &result);`. The
  `Msg::Completion(Box::new(result.with_ghost(ghost)))` send stays as it is.
- 7.7: `new` drops the `ghost_cache` initialiser and, once nothing reads it, the `history` field; destructure with
  `let HistoryModule { record_command, .. } = history_module;`. Test helpers change only their `EngineServer::new(..)` arguments.
- Imports: `tests.rs` has no imports of its own for these names and relies on `use super::*`. When 7.6 removes
  `INLINE_ENABLED` and 7.7 removes `HistoryEntry`, `Arc`, `Shell` (and `PathBuf` if unused) from `engine_server.rs`, add explicit
  `use` lines for them at the top of `engine_server/tests.rs` (imports are not locked; the 12 bodies stay identical). In 7.7, after
  `llm_history_honours_history_count` moves out, remove the then-unused `serde_json::{json, Value}` and `settings_from_json`
  imports from `tests.rs`, or the clippy budget of 3 is exceeded.
- 7.7 also rewords the `//!` header of `engine_server.rs` ("...and the legacy ghost text block" becomes "...and the ghost_text use
  cases"), so `grep -rn 'legacy ghost text\|ghost_cache\|GhostCache\b\|fn llm_complete' src` prints nothing.

### Building a `CompletionResult` in tests (K2, K3, K5, K9; briefs 7.3, 7.6)
- No `Default`, no `CompletionResultParts` default and no setter for items, buffer, cursor or ghost exist. A result with
  suggestions is built by writing all 16 `CompletionResultParts` fields (`req` 1, `phase: CompletionPhase::Parse`, `error` false,
  `buffer`, `cursor`, `tokens`, `command_text`, `command_index` 0, `arg_key` 0, `current_arg: None`, `search_term`,
  `triggered_generators` empty, `items`, `loading` false, `ghost: None`, `fuzzy` false) into `CompletionResult::new(..)`, and each row
  by `Suggestion::new(SuggestionParts { 15 fields })` with `Insertion::new(InsertionParts { delete, text, cursor_back, execute })`
  (`SuggestionKind::Subcommand`, `prefix_match` true). Write a local `result(buffer, cursor, suggestions)` helper and a
  `suggestion(text)` helper in the use-case test module. A mid-word result is the `cursor` field of the literal. An error reply is
  `CompletionResult::error(req, buffer, cursor)`. There is no `PartialEq` on `CompletionResult`: assert through
  `ghost()`, `buffer()`, `cursor()`, `suggestions()`. Do not add production setters. Do not reuse `crate::menu::tests::comp` from
  ghost_text (a legacy dependency). O7's fallback (three values instead of `&CompletionResult`) is not needed.
- K9: when 7.3 removes `item()` and `spec_ghost_rules` from `inline.rs`, delete the then-unused imports at `inline.rs:11-12`
  (`Suggestion`, `SuggestionKind`) and the test imports at `:509-510` (`Insertion, InsertionParts`, `SuggestionParts`); the moved test
  module in `spec_ghost_text.rs` needs `Insertion`, `InsertionParts` and `SuggestionParts` (`Suggestion`, `SuggestionKind` come from `use super::*`).

### Smaller corrections
- **P7-T16 (7.4).** The user text is the header `OS: {}\nShell: {}\nCWD: {}` (OS = `std::env::consts::OS`, `shell.name()`,
  `cwd.display()`), then a blank line, then the numbered history prompt: the header is ABOVE the history. "History first" in the trap
  means evaluation order only (`history_prompt(..)?` runs first, and a `None` returns early). `llm_prompt` keeps
  `format!("OS: {}\nShell: {}\nCWD: {}\n\n{prompt}", ..)` verbatim.
- **Unused imports (7.5).** `#[cfg(test)]` goes on `use std::sync::Mutex;` in `application/ports/llm_client.rs` and on
  `use std::path::PathBuf;` in `application/ports/directory_probe.rs` (keep a plain `use std::path::Path;` for the trait). The
  clippy budget is 3 with all 3 used.
- **Plan comments.** `settings_catalog.rs:335` reads `engine_server.rs llm_history`, `:355` reads `cli.rs, doctor.rs, engine_server.rs
  and pty.rs`, `:555` reads `inline.rs and cli.rs`. No line says `engine.rs`. 7.7 repoints `:335` to
  `request_llm_ghost_text_use_case.rs llm_history`. 7.9 repoints `:355` (`engine_server.rs` becomes
  `suggest_local_ghost_text_use_case.rs`, `cli.rs` becomes `get_inline_status_use_case.rs`) and `:555` (`ghost_text_source_resolver.rs`
  and `doctor.rs`). 7.10's grep is `grep -rn 'inline\.rs\|engine\.rs\|engine_server\.rs\|cli\.rs' src/settings/domain/services/settings_catalog.rs`
  and shows no line naming a file that no longer holds the read.
- **7.10 plan edits.** Besides the `Llm` event-table row, layout, section 5, section 8 and the Phase 7 notes, edit in place plan section 4's
  bootstrap block (drop "the legacy ghost block until Phase 7" from the `engine_server.rs` line) and the event-table lead-in "Until
  Phase 7 replaces them, the ghost_text:: steps are the legacy ghost block of engine_server.rs (...)" (delete or reword). README
  splits the single `7-11 | not started` row into a Phase 7 row plus `8-11 | not started`. Leave the historical Phase 5 and 6 text
  (the Phase 6 step "calls the legacy inline.rs until Phase 7" and the Phase 6 notes) unchanged.
- **Hand grep, every step and 7.10.** After every commit run `grep -rnE 'completion::application::(command_line_parser|generator_runner|test_fixtures)' src --exclude-dir=completion`.
  It must print nothing outside `#[cfg(test)]` code (it is empty at `058bebd`). Fitness rule 3 cannot see completion's application
  services. No ghost_text step needs any `completion::application` path.
- **M-traps.** M7 goes to 7.5, M8 is carried by 7.2 (the profile reason texts) and M9 by 7.8 (the lenient bool in the status), not 7.7.
  7.10's plan notes record that.
- **cfg lines at `058bebd`** (compare by attribute text and count against `git show phase7-start:<file>`, shifted only by the step's own
  edits): `pty.rs` 135, 137, 1317, 1436, 1777, 2505; `cli.rs` 609, 789, 831 (and `cfg!` at 397, 431, 566); `doctor.rs` 481, 655,
  723, 775, 787.
- **Other.** The pty's LLM request is the port call `self.engine.request_llm_ghost_text(self.req, buffer, self.cwd(), self.shell())`,
  and 7.1 leaves it alone. The type is `EngineMessage`, not `EngineMsg`. `doctor.rs` and the inline section of `cli.rs` are identical to
  `ff2db5a`. `inline.rs` differs only in `spec_ghost`'s `&[Suggestion]` accessor reads and the test imports and `item()` helper. The
  ghost cache field is `ghost_cache: Arc<Mutex<GhostCache>>` with `GhostCache` imported from `crate::inline` (7.3 changes the import
  and the type name). The tokenizer's `next_word` is `command_line_tokenizer.rs:217`; the grep for `fn next_word(g: &str)` does not match it.
  If a step edits `engine_inbox.rs` or `engine_message.rs`, run the two inbox tests 50 times by full path with `-- --exact`.
  7.2's check also runs `grep -rnE 'inline::(\{[^}]*(Profile|Provider|Source|BUILTIN_SOURCES)|source\(|profiles\()' src` and
  `grep -n 'Source::' src/bootstrap/engine_server.rs` (only `GhostTextSource::` variants).
