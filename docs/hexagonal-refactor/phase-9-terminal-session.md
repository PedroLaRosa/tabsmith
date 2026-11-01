# Phase 9 (terminal_session): preparation

Status: **prepared, not started** (HEAD `5efb4b2` on `refactor/hexagonal-dropdown-terminal-session`; the local lightweight tag `phase9-start` is at `858b7b5`, the last commit that touched `src/`: `git diff phase9-start HEAD -- src` is empty). It was prepared
against the end of Phase 8 (`da2f950`, 1091 passed, 0 failed, 1 ignored) plus its post-close docs and two `chore(scripts)` commits (`b41f752` runs the gate on Linux with `stat`/`sed` shims, `5efb4b2` compile-checks the `cfg(windows)` code in the gate). Gate at `5efb4b2` on this
container (Linux, clippy 1.97): build warnings 0, **1091 passed, 0 failed, 1 ignored** (`all_bundle_modules`), clippy **4** (`BASELINE_CLIPPY=4`: `completion_result_builder.rs:55`, `pty.rs:1801`, `embedded_shell_scripts/tests.rs:574`, `file_dotfile_repository/tests.rs:261`), golden identical
(131 commands), `cargo check --locked --all-targets --target x86_64-pc-windows-gnu` ok, testlock against `ef807c1` MISSING 0. The decisions below take precedence over the plan's letter where they differ; Phase 9 writes its own notes into the plan's section 9 at 9.7.

**How it was researched.** Six read-only researchers read `5efb4b2` and wrote notes under `target/hexagonal-refactor/p9/` (git-ignored; this file is the merge and wins where they differ):
- `recon-tracker-domain.md` (+ `recon-tracker-domain-prototype/`: the domain files, `legacy-rewire.patch`, `differential_test.rs`; 38 tracker tests and a 36 000-batch differential test pass on a copy of HEAD);
- `recon-tracker-infra.md` (`OutFilter`, `extract`, `tag`, `mark_line_start`, the `TerminalScreen` port, `FrameInput`);
- `recon-session-domain.md` (+ `recon-session-domain-prototype/`: the aggregate, entities and services with 90 domain tests, compiled on a copy of HEAD);
- `recon-use-cases.md` (the `Session` methods as services and use cases, the harness, the call-order table);
- `recon-driver-cli.md` (`run`, `Session::start`, the reader, the probe, `sys`, `cli.rs` commands; Appendix A is `pty-diff.py`);
- `recon-governance.md` (+ `recon-governance-prototype/`: `characterisation_*_tests.rs.txt`, `session-diff.py`, `fitness-mod.sh`, `p9-greps.sh`, `edges.py`; fitness experiments E1-E3 and probes V1-V7 on copies under `target/hexagonal-refactor/review-tmp/`).

A synthesizer merged them: traps **P9-T1...T101** and **P9-N1...N6**, decisions **R1-R46**, rechecks **K1-K16**, **30 deviations**, test moves for every `#[test]` of `pty.rs`, `tracker.rs` and the pty parts of `cli.rs`, the guard design, hand-off notes and **42 steps (9.0a ... 9.7) in six runs**. The prototypes are for implementers to LIFT from (they
compiled and passed on copies; the real steps rebuild them test-first, in the named steps). The step list is also returned to the orchestrator as structured data; each run's step prompts are written from the tables below, as Phase 8's `phase-8-dropdown.args.json` was.

Plan reference: [the plan](../hexagonal-refactor-plan.md).
- Section 5, "Phase 9: terminal_session" (seven steps; this file cuts them into 42) and "Phase 10: diagnostics / Phase 11" (the consumers of this phase's leftovers).
- Sections 1 (Rust conventions: `Rc<dyn Port>` use cases, no generics, one type per file, `mod.rs` only declares), 2 (allowed dependencies: `terminal_session -> shared, settings, history, completion, ghost_text, dropdown`; the bridge rows), 4 (`terminal_session` layout and use-case table), 6 (checklist rows
  `pty.rs`, `tracker.rs`, `types.rs`, `cli.rs` debug part), 7 (risks: the byte-offset interleaving, one write per batch, panic tiers), 8 (verification, manual smoke items 1-9) and 9 (D1-D9 and the Phase 1-8 notes).

Glossary: [`docs/ubiquitous-language.md`](../ubiquitous-language.md), section terminal_session. Source maps: [`pty`](source-map/pty.md), [`tracker`](source-map/tracker.md), [`cli`](source-map/cli.md). Their numbers are at `af1b304`: much moved since, so **locate every site by symbol; this file cites `5efb4b2`**
(the traps cite `P` = `src/pty.rs`, `T` = `src/tracker.rs`, `C` = `src/cli.rs`, `R` = `overlay_renderer.rs`).

**Ids in this file.** Traps **P9-T1...** (origin in brackets: `td` tracker-domain, `ti` tracker-infra, `sd` session-domain, `uc` use-cases, `dr` driver-cli, `gv` governance, `sm` source map) and noticed-not-fixed **P9-N1-N6**; decisions **R1-R46**; rechecks **K1-K16**; deviations **1-30**; steps **9.0a-9.7**;
open questions **O1-O18**; acceptance greps by the labels of `phase9-greps.sh` (A1-E1) plus **F1-F5**. Phases 4-8 ids are prefixed (`P8-T25`, `P7-Q15`, Phase 8 `R3`). Never cite a bare `T`, `R` or `G`.

**Line numbers.** Every line below is `5efb4b2`. After the first step commits, every site is found by symbol.

## What Phases 0-8 leave for Phase 9

Assumed, and verified at `5efb4b2` unless a K row says otherwise:
- **Legacy files.** `src/pty.rs` (2537 lines: the session, the driver, `sys`, 36 tests), `src/tracker.rs` (2210: the splitter, the output filter, the tracker, the scrape, 38 tests), `src/cli.rs` (510: `DiagnosticArgs`, `DebugCmd`, `diagnostic*`, the key tester, the debug shell, `debug_logs`, 4 tests), `src/types.rs` (29: `Msg`, `now_ms`, `now_secs`, `Model`).
  `main.rs` declares `mod cli; mod doctor; mod pty; mod tracker; mod types;` besides the registered modules. They are flat files: the fitness test does not read them, and they may import infrastructure of any module.
- **Registration.** `REGISTERED_MODULES` = completion, dropdown, ghost_text, history, settings, shared, shell_integration, specs (`architecture_tests.rs:9-10`); `LEGACY_MODULES` lists `cli doctor engine icons inline integrations js keys menu parser pty render shell_parser spec suggest tracker types` and stays as it is (R38). Registering `terminal_session` at HEAD
  produces zero violations (55 architecture tests, run on a copy of HEAD; R3).
- **`terminal_session` today** (unregistered): `application/{ports/{mod.rs, completion_engine.rs}, report_session_context_use_case.rs}`; `domain/{entities/session_context.rs, repositories/session_context_repository.rs, value_objects/{key_event.rs, session_id.rs}, terminal_session_error.rs}`;
  `infrastructure/{adapters/{file_session_context_repository, icon_assets, kitty_graphics, overlay_renderer, stdin_key_decoder}.rs, cli/pre_cmd_command.rs, terminal_session_module.rs}`. There is no `domain/services/`, no `infrastructure/pty/`, no `application/test_fixtures.rs`.
  `CompletionEngine` (`complete`, `command_finished`, `command_ended`, `record_insertion`, `request_llm_ghost_text`, `apply_settings`, `apply_session_context`, `toggle_fuzzy_search`, `shut_down`; all `&self`, fire and forget) and its `#[cfg(test)]` `RecordingCompletionEngine`/`CompletionEngineCall` (calls forwarded to an
  `mpsc::Sender`) live in `ports/completion_engine.rs`; `build_terminal_session_module(&SharedDependencies)` builds only `ReportSessionContextUseCase`.
- **From Phase 8.** `dropdown`: `Key`, `KeyAction`, `KeyBinding`, `KeyCode`, `Keymap`, `Modifiers`, `Menu`, `MenuEffect`, `Theme`, `ThemeRepository`, `resolve_theme`, `Rgb`, `GraphicsSupport`, `DropdownModule::themes`. `OverlayRenderer` (`overlay_renderer.rs`, 1147 lines: `FrameInput` at 28-39, `use crate::types::Model;` at 17,
  `menu.loading(Instant::now())` at 307, a 551-line inline test module of 10 tests with hash scenarios); `StdinKeyDecoder` (`stdin_key_decoder.rs`, 408 lines, pure std, 5 tests, `Default`, `decode`); `KeyEvent` (domain VO); `kitty_graphics::detect()`; `icon_assets`. `pty.rs` imports them as its Phase 8 hand-off lists.
- **From Phases 2-7.** `Settings` (`bool/f64/string/i64`, catalog defaults), `SettingsRepository` (`load`, `save`, `last_modified`), `load_or_defaults` (warn `settings: {chain}`, defaults; only `pty.rs` and `doctor.rs` call it), `JsonFileSettingsRepository`; `Clock` (`now_ms`, `now_secs`, `now_instant`), `FixedClock::{new, advance}`, `SystemClock`,
  `SharedDependencies { data_directory, clock }`, `DataDirectory`, `Shell` (shared domain: `from_name`, `Bash/Zsh/Fish/Pwsh`), `logger`, `error_chain::display_chain`, `user_home_from_env`; `CompletionRequest`, `CompletionResult`, `Insertion`, `Suggestion`, `GhostText`/`ghost_text_source_resolver`, `HistoryEntry`; `specs`'
  global `kill_live_children()` (`process_executor.rs:326`); the engine thread of `bootstrap/{engine_server, channel_completion_engine}.rs`, which replies as `Msg::Completion/Ghost` into the wrapper's inbox through `CompletionEngineStarter` (`pty.rs:74`).
- **Test tooling.** `scripts/hexagonal-refactor/{gate.sh, golden.sh, testlock.py, make-baseline.sh, precmd-diff.sh, inline-diff.sh, integrations-diff.sh, theme-diff.sh, compat/}`; `shared/infrastructure/testing/{mod.rs, temp_dir.rs}` (`#[cfg(test)]`); `completion/domain/test_fixtures.rs`; the baseline binary
  `target/hexagonal-refactor/baseline-tree/target/debug/tabsmith`. `docs/git-convention.md` is named by AGENTS.md but does not exist in this checkout: R44 applies.

### Re-check at the start of run 1 (the items marked DEFERRED are re-read against the tree each run really starts from)

| K | Claim | Step | Fact at `5efb4b2` |
|---|---|---|---|
| K1 | The legacy importers of the files that die are exactly: `pty.rs:26` (`tracker::{Event, Extracted, Tracker}`), `:38-40` (`types::{Model, Msg}`), `:1321`, `:1440` (`types::Msg` in the two `sys` modules); `tracker.rs:6` (`types::{now_secs, Model}`); `overlay_renderer.rs:17` (`types::Model`); `cli.rs:3` (`pty::CompletionEngineStarter`), `:257` (`crate::pty::run`); `bootstrap/cli.rs:30` (`{cli, doctor, pty}`, `:148-150`); `bootstrap/{channel_completion_engine.rs:9, engine_server.rs:25, engine_server/tests.rs}` (`Msg`); `doctor.rs:32` (`types::now_secs`, stays) | all | `grep -rnE 'crate::(tracker\|pty\|types\|cli)\b\|\b(pty\|tracker)::' src` (minus `portable_pty::`) shows these and the comment at `architecture_tests.rs:375`. DEFERRED: rerun |
| K2 | 83 tests live in the files that die: `pty.rs` 36, `tracker.rs` 38, `cli.rs` 4 (`key_tester_line`, `diagnostic_toml`, `debug_shell_rc_dir_and_quoting`, `debug_shell_survives_closed_terminal`), `stdin_key_decoder.rs` 5; plus the renderer's 10 (which only move to a sibling file) | all | `grep -c '#\[test\]'` per file; `cargo test --locked -- --list` after a build |
| K3 | Gate at HEAD: 1091 passed, 1 ignored; clippy 4 on Linux; golden identical; windows check ok; `all_bundle_modules` `1 passed` in release; `--no-default-features` checks | 9.0a | `target/hexagonal-refactor/gate-last/{test,clippy}.txt`; run the gate once at the start (it is the baseline of every step) |
| K4 | `phase9-start` is at `858b7b5`; `git diff phase9-start HEAD -- src` is empty; the working tree is clean | 9.0a | `git status --short` empty (the research notes under `target/` are ignored) |
| K5 | The baseline binary exists, `target/debug/tabsmith` builds, and the two new scripts need `python3`, `tmux`, `bash`, `vim` and `git` on PATH | 9.0a | `command -v tmux vim bash python3`; the baseline tree is `make-baseline.sh`'s |
| K6 | Registering `terminal_session` passes at HEAD: `fitness-mod.sh terminal_session` (or the prototype) prints `55 passed`. The scanner treats a test-support file as production code unless each item is `#[cfg(test)]` (probes V1-V4, P9-T97) | 9.0a, 9.4b | rerun on a copy; K12 repeats V1-V4 at 9.4b |
| K7 | `types::Msg` has eight variants (`Stdin`, `Pty`, `PtyEof`, `Resize(u16, u16)`, `ChildExit(i32)`, `Term`, `Completion(Box<CompletionResult>)`, `Ghost { req, text }`); `types::Model = Term<VoidListener>`; `now_secs` has one caller left after Phase 9 (`doctor.rs:32`), `now_ms` none | 9.2a, 9.6a | `types.rs` as read |
| K8 | No Phase 9 new test name exists yet (R32): `grep -rn "fn <name>(" src` is empty for every name of "New tests" (the list is extracted from the table by script at the start of each run; the four unnamed 9.3i names are chosen by that step and grepped the same way) | all | DEFERRED: run for all names at launch and at each run start |
| K9 | The cfg lines that must survive a move (R37): `pty.rs` 83, 139, 148, 150, 424, 669, 714 (`cfg!(test)`, dies), 1319, 1438, 1752, 2506; `tracker.rs` 555; `cli.rs` 235, 278, 380, 422 (the lines 66 and 100 stay with `diagnostics`) | all | `phase9-greps.sh` C1 compares the sorted multiset with `phase9-start` |
| K10 | `debug_shell_survives_closed_terminal` (`cli.rs:424`, unix) is the only automated start-up of the wrapper (probe, renderer, `Keymap`, `Menu`, theme); it names its own libtest path in `NAME` (`cli.rs:427`, D9) and re-execs itself with `--exact` | 9.6c | a stale `NAME` makes the child run 0 tests and the marker assertion fail after ~20 s (P9-T94) |
| K11 | `alacritty_terminal` 0.26: `Handler::set_hyperlink` only writes the cursor template (`term/mod.rs:1874-1877`), so no tracker command moves the cursor (P9-T27); MSRV 1.85 (`Cargo.toml: rust-version`); no new crate is needed (`unicode-width` is the only external crate domain/application may use) | 9.1d, 9.2e | read from the vendored source; `cargo tree` |
| K12 | Rule 3 cannot see a service or use case of another module from outside (the Phase 4-6 ceiling), so F1 pins it by grep; the checker reads `application/test_fixtures.rs` as production code unless every infrastructure `use` has its own `#[cfg(test)]` | 9.4b | V1-V4 of `recon-governance.md`; verified by `fitness-mod.sh --worktree` after 9.4b |
| K13 | Hand greps empty at HEAD: `grep -rnE '^\s*pub(\(crate\))? use ' src` outside `architecture_tests.rs` strings (B6; NOT the looser `'pub(crate) use\|pub use'`, whose second alternative matches the field `pub user: Option<String>` of the legacy tracker, `tracker.rs:523`, until 9.1e deletes it); `grep -rnE 'completion::application::(command_line_parser\|generator_runner\|test_fixtures)' src --exclude-dir=completion` | all | confirmed |
| K14 | Clippy budget 4 on Linux (3 on macOS, all used): every step prunes the imports it leaves unused, adds no `new` with more than 7 parameters (use a `*Parts` struct), no `Default` unit struct, no `match` with duplicated bodies; one of the four (`pty.rs:1801`, `Modes::default()`) dies with the `modes` field at 9.4c | all | `gate-last/clippy.txt` |
| K15 | The golden capture reaches only `_ pty` error paths and `debug shell --help`: the session itself is guarded by the unit tests, the 17 characterisation fixtures, `pty-diff.py`, `session-diff.py` and the manual smoke (guard section) | all | `golden.sh` |
| K16 | The orchestrator's checks at launch: a clean tree, the docs commit (Launch 2), `phase-workflow.js` reads `args.branch`, and 20 serial runs of the pty tests are green at HEAD (the baseline for the flake checks of R45.8) | launch | `cargo test --locked pty:: -- --test-threads=1` x 3 |

## Where the code is today (`5efb4b2`)

| What | Where (lines) | Used by | Goes to (step) |
|---|---|---|---|
| `Piece`, `Splitter` (OSC 697 markers, window names), `SEQ_MAX`, `OSC_MAX`, `TAG_PROMPT` | `tracker.rs:14-161` | `Tracker::feed`, `OutFilter` | `OutputPiece`, `MarkerSplitter` (9.1a) |
| `SuggestionColor`, `rgb_to_256`, `parse_zsh/fish`, `parse_color`, `parse_sgr`, `Flags`, `Cell` matching | `tracker.rs:167-275, 950-989` | `extract`, `on_marker` | `shell_suggestion_color.rs`, `text_emphasis.rs`, `pwsh_prediction_color.rs` (9.1a); the cell matching to `edit_buffer_extractor.rs` (9.2b) |
| `OutFilter` (keyboard-protocol upgrades, scroll region, strings, held UTF-8) | `tracker.rs:283-470` | `Tracker::feed`, `Session::on_output` | `output_filter.rs` (9.2a) |
| `Event`, `Extracted`, the `Tracker` fields, `on_marker`, `new_cmd`, `pre_exec` | `tracker.rs:483-760` | `Session::on_event`, `refresh` | `shell_event.rs`, `edit_buffer.rs`, `shell_marker.rs`, `marker_tracker.rs` (9.1b-9.1e) |
| `Tracker::feed` (the batch loop) | `tracker.rs:589-605` | `Session::on_output` | `ShellOutputFeeder` (9.2f) |
| `extract` (the scrape), `strip_pwsh_prompts` | `tracker.rs:763-945` | `Tracker::pre_exec`, `Session::refresh` | `edit_buffer_extractor.rs` (9.2b) |
| `tag`, `mark_line_start` | `tracker.rs:992-1032` | `new_cmd`, `pre_exec`, `end_prompt` | `cell_tagger.rs` (9.2a) |
| 38 tracker tests, rig `setup/feed/buf` | `tracker.rs:1034-2210` | | "Test moves" (9.1a-9.2f) |
| timer consts, `PROBE*`, `LATE_PROBE`, `PTY_CREDITS`, `EXIT_GRACE` | `pty.rs:55-71` | `Session` | `session_deadlines.rs` (9.3a), `terminal_probe.rs`, `pty_reader.rs` (9.6a), `LATE_PROBE` in the aggregate (9.3g) |
| `CompletionEngineStarter`, `run`, `env_nonempty`, `argv`, `fallback` | `pty.rs:74-157` | `bootstrap/cli.rs:150`, `cli.rs` | `completion_engine_starter.rs` (9.6b), `pty_session.rs` (9.6e), `WrappedShellCommand` (9.3b), `fallback_shell.rs` (9.4a) |
| `insertion_bytes`, `expected_buffer`, `unechoed_after`, `ghost_key`, `is_ctrl` | `pty.rs:161-250` | `on_keys`, `insert`, `refresh` | `insertion_reconciler.rs`, `key_router.rs` (9.3a) |
| `pump`, `parse_probe`, `parse_cpr`, `parse_osc11`, `session_id`, `write_stdout` | `pty.rs:231-331` | `start`, `run` | `pty_reader.rs`, `terminal_probe.rs` (9.6a), `SessionId::from_entropy` (9.1d), `StdoutTerminalWriter` (9.4b) |
| `Session` (46 fields) | `pty.rs:333-397` | | `TerminalSession` + six entities (9.3c-9.3i) |
| `Session::start` | `pty.rs:401-576` | `run` | `session_launcher.rs::launch_session` (9.6e) |
| `run`, `step`, `guard`, `emit`, `next_deadline` | `pty.rs:579-739` | | `session_runner.rs` (9.6d), `TerminalWriter` (9.4b) |
| `alt`, `cwd`, `shell`, `llm_enabled`, `ghost_text`, `basis`, `note_forwarded`, `suppressed`, `drawable` | `pty.rs:741-1000` | | the aggregate (9.3g-9.3i) |
| `on_output`, `on_event`, `refresh`, `on_completion` | `pty.rs:758-902` | | `ProcessShellOutput` (9.5d), `CompletionRequester` (9.5b), `ApplyCompletionResult` (9.5e) |
| `frame`, `redraw`, `take_repaint`, `on_input`, `on_keys`, `insert` | `pty.rs:1002-1191` | | `OverlayPainter` (9.5a), the runner (9.6d), `KeystrokeHandler` (9.5c) |
| `on_resize`, `on_timers`, `poll_files`, `cleanup` | `pty.rs:1193-1317` | | `ResizeTerminal` (9.5f), `HandleTimerTick` (9.5g), `SessionFilePoller` (9.5b), `EndSession` (9.5h) |
| `sys` (unix 1319-1436; windows 1438-1604): `enter`, `read_timeout`, `spawn_input`, `wait`, `wait_pid`, `spawn_resize`, `Modes` | `pty.rs:1319-1604` | `start`, `run` | `platform_unix.rs`, `platform_windows.rs` (9.6b), `ConsoleTerminalModes` (9.4c) |
| 36 pty tests, fixture `session()`, helpers | `pty.rs:1606-2537` | | "Test moves" (9.3a-9.6f) |
| `DiagnosticArgs`, `DebugCmd`, `diagnostic*`, `debug`, `key_line`, `key_tester`, `debug_shell`, `debug_shell_files`, `debug_logs` | `cli.rs:22-320` | `bootstrap/cli.rs:76, 82, 146-148` | the three commands (9.6c); `diagnostic*` and `debug_logs` stay (Phase 10) |
| 4 cli tests | `cli.rs:321-510` | | 9.6c (3), `diagnostic_toml` stays |
| `Msg`, `now_ms`, `now_secs`, `Model` | `types.rs:1-29` | pty, tracker, bootstrap engine, renderer, doctor | `SessionMessage` (9.6a), `headless_model.rs` (9.2a); the clock helpers stay (Phase 11) |
| `FrameInput`, `OverlayRenderer` (+ the 551-line test module) | `overlay_renderer.rs:28-39`, tests inline | `pty.rs`, the screen | `frame_input.rs` (9.2c), `overlay_renderer/tests.rs` (9.2c) |
| `StdinKeyDecoder` + 5 tests | `stdin_key_decoder.rs:1-408` | `pty.rs:32` | `KeyDecoder` entity (9.3b) |

Sizes: `pty.rs` 2537 lines (production 1-1605, tests 1606-2537), `tracker.rs` 2210 (production 1-1033, tests 1034-2210), `cli.rs` 510, `types.rs` 29. Nothing of `pty.rs`, `tracker.rs` and `types::{Msg, Model}` outlives Phase 9; `cli.rs` keeps `diagnostic*`, `debug_logs` and `diagnostic_toml` for Phase 10.
Legacy code leaves with its last item (R38), not in one final step.

## Behaviour that must not change

Grouped by the steps that move the code. "Kept by" says how the design preserves it; "pinned" = a test at `5efb4b2` fails if it breaks, "9.0" = pinned by a characterisation test added before any code moves.
Every cite is `5efb4b2` (`P` = `src/pty.rs`, `T` = `src/tracker.rs`, `C` = `src/cli.rs`, `R` = `overlay_renderer.rs`); the source maps' numbers at `af1b304` are 2 to 50 lines lower. Origins in brackets: `td` tracker-domain, `ti` tracker-infra,
`sd` session-domain, `uc` use-cases, `dr` driver-cli, `gv` governance, `sm` the source map.

### Tracker pipeline and the batch loop (9.1, 9.2)
- **P9-T1 Byte-offset interleaving** [td, ti, sm] (T:589-599). Pieces are handled in stream order; a marker's model effects (`tag`, `mark_line_start`) reach the model BEFORE the next `Bytes` piece is advanced. Kept by: `ShellOutputFeeder` applies each marker's `ScreenCommand`s right after its tracker call, inside the piece loop, never at the end of the batch. 9.0.
- **P9-T2 `WindowName` goes to the filter only** [td, ti] (T:596). Routing it to the model drifts the cursor (`window_name_reaches_the_terminal_but_is_not_printed` asserts column 4). Kept by: its own port method `pass_window_name_through`. Pinned.
- **P9-T3 Markers never reach `out`** (T:597). A `Marker` piece calls no screen output method. Pinned (`markers_stripped_from_output`).
- **P9-T4 `batch_markers` counts EVERY marker** [td, ti] (T:621, before the empty-value return :626 and the lock gate :629). Skipping `Ignored`, locked-out or wrong-sid markers changes when the Windows deferral flushes (:601). Kept by: the application calls `on_marker` for `ShellMarker::Ignored` too; the counter lives in the entity.
- **P9-T5 The end-of-batch flush runs only when the batch had zero markers, at the cursor of the END of that batch** [td] (T:600-604; `windows_deferral_fifo`: "applies the queue in order, at the current cursor"). Kept by: `tracker.end_batch(screen.cursor_column(), now_secs)` evaluated AFTER the piece loop, called for every batch including an empty one (the old `feed` always ran its tail), and flushing regardless of the lock. 9.0.
- **P9-T6 The `PreExec` and `NoHistory` guards test `preexec` BEFORE the flush** [td] (T:649-653). On Windows a batch `.. EndPrompt NewCmd PreExec` with `preexec == true` queues the `NewCmd`, so `PreExec` still sees `preexec == true` and is IGNORED; flushing first would accept it. Kept by: `on_marker` tests the guard first and flushes inside the arm; pinned by the `tracker_defer` guard (9.0b: a chunk `EndPrompt` `NewCmd=<sid>` `PreExec` while a command runs) and by `should_ignore_a_pre_exec_whose_new_cmd_is_still_queued` and `should_not_flush_a_queued_new_cmd_for_no_history_while_a_command_runs` (9.1d, deferral ON); the 9.1e differential test with `defer` on catches both mutations too but is never committed. Suspicious (a PreExec is lost), reported not fixed (P9-N1).
- **P9-T7 `pre_exec` scrapes the model at the `PreExec` byte offset and does NOT filter by `at_prompt` or the alternate screen** [td, sm] (T:714-717 against the filter at P:817), after the deferred markers' tags were applied (:713). Reusing the session's cached `cur` changes recorded history (the transient-prompt tests). Kept by: `EditBufferNeeded(flush effects)` is applied BEFORE `edit_buffer_context()`/`screen.edit_buffer`; no filter on that path; `refresh` keeps its own `at_prompt` filter and alt rule (in the aggregate).
- **P9-T8 Events are dispatched after the WHOLE batch, in emission order** [td, ti] (P:765-767). A flush's events precede `PreExec`; `PreExec` precedes the `NewCommand` of the same batch. Kept by: `MarkerEffects.events` collected into the feeder's return value; `ProcessShellOutput` dispatches them after `feed`.
- **P9-T9 Per-key trimming must not be unified; bare keys are not empty values** [td, sm] (T:637, 646, 656, 672). `NewCmd` compares the UNTRIMMED value with the sid (`val != Some(sid)`, :646); `Dir`'s value is untrimmed (`PathBuf::from(val.unwrap_or(""))`, :656): a bare `Dir` sets `cwd = Some("")`, only `Dir=` is ignored (:626); `PwshPrompts` splits the untrimmed value; every other value is trimmed. A bare `ExitCode`/`PID` overwrites with `None`; a bare `Shell` gives `shell = None` and `shell_name = Some("")`. Kept by: the variant types of `ShellMarker` (R7) and 13 tests.
- **P9-T10 The lock allow-list** [td, sm] (T:629-636): exactly `NewCmd`, `PreExec`, `NoHistory`, `OSCLock`, `OSCUnlock` (and the dead `User`) pass; `StartPrompt`/`EndPrompt` are dropped while locked, not deferred. Kept by: `passes_integration_lock` with explicit arms. Pinned (`lock_and_foreign_session_ids`).
- **P9-T11 `Shell=<unknown>` replaces a known shell by `None`** [td, sm] (T:658-661); `shell_name` keeps the raw text; the session falls back with `tracker.shell().unwrap_or(launched)` (P:754). Never `from_name(v).or(self.shell)`.
- **P9-T12 `new_cmd` order** [td, sm] (T:685-709): `redraw = cycle.filter(!preexec)` BEFORE the increment; commands `[Tag(Cycle(n)), MarkLineStart{redrawn_cycle: redraw}]` in that order (`mark_line_start` reads the template link `tag` just set, :1006); `line_col0 = column == 0` with the RAW `cursor.point.column.0` (:693), not the wrap-adjusted column `extract` uses (:783-790). Kept by: the port doc of `cursor_column` and P9-T27.
- **P9-T13 Exit code, pending and `submitted`** [td, sm] (T:696-709). `exit_code.take()` runs BEFORE the pending lookup (a redraw or a nested shell's first prompt consumes `ExitCode`); `submitted = None` ONLY when a pending entry matched (:700); `rposition(|p| p.pid() == self.pid)` compares `Option<i32>` so `None == None` matches when no PID was ever reported; `drain(k..).rev()` emits the innermost first and every drained entry gets the same exit code; a blank command is `CommandEnded` (`h.command().is_empty()`, :704). Pinned (`nested_shell_prompt_keeps_outer_command_running`, `transient_prompt_before_preexec_keeps_the_submitted_line`).
- **P9-T14 The `HistoryEntry` built at `pre_exec`** [td, sm] (T:728-750): `shell` is the RAW `shell_name.unwrap_or_default()`, `cwd` is `p.display().to_string()` (lossy), `start_time = now_secs`, `hostname`/`end_time`/`exit_code` default; `then_output` (:721-727) trims only ASCII spaces, the match must start the scrape or follow a space and be followed by the end or `\n`, `submitted` is trimmed first and an empty one skipped; `mem::take(&mut no_history)` (:733); `pwsh_list_view = false` at PreExec (:748), not at NewCmd; `pwsh_continuation` defaults to `">> "` (:576). Kept by: `HistoryEntryPrimitives { .., ..Default::default() }` as before and the verbatim `complete_pre_exec`.
- **P9-T15 Two "at prompt" meanings** [td, sd, sm]. `MarkerTracker::at_prompt` (cycle open and not preexec, T:616) is not `EditBuffer::at_prompt` (cursor on the command line, T:500, 843-855); `suppressed` uses the first (P:975), `refresh`'s filter the second (P:817), Enter and `execute` the first (P:1107, 1133). Never merged.
- **P9-T16 Palette semantics** [td, ti, sm] (T:762-772, 211-254, 950-989). A pwsh cell with a reported colour needs ALL its flags (`contains`; an empty set always matches), without one ANY of dim/italic; zsh/fish without a reported colour, and bash, have no suggestion; `cell_col`: `Named(n)` maps to its index only below 16, `Foreground`/`Background`/`Cursor`/`Dim*` become `None` (= the "default" token `Some(None)`); `parse_color`: the fish flag disables decimal and makes `#` optional, `parse_fish` skips the token after `-b/--background`, `parse_zsh`/`parse_fish` return `None` when nothing parsed; `parse_sgr` splits on `[';', ':', '[', 'm']`, drops unparsable tokens, `38;5;n` with n > 255 still consumes 3 parameters, `0` resets, `None` unless colour or emphasis was set. Kept by: verbatim moves (`TextEmphasis::is_within` = `contains`, `is_dim_or_italic` = `intersects`); colour matching now evaluates `cell_col` for both layers even when one list is empty (the old `||` short-circuited): a pure function over a few cells, unobservable.
- **P9-T17 `extract` details** [ti, sm] (T:776-918). The cursor check `(r, c) == (cl, cc)` runs BEFORE the wide-spacer `continue` (:878-886); a pending wrap sets `cc = cols` (:783-790) and is handled after the row loop (:903-905); limits `cl - start < 100` / `cl - top < 100` (:808, 835); the final trim is `keep = e.buffer.trim_end().len().max(e.cursor)`; `strip_pwsh_prompts` runs before it and only for pwsh (:911-916); `EditBuffer.cursor` is a BYTE offset (`e.buffer.len()`, :868) while the at-prompt test counts CELLS (`pwsh_cont.chars().count()`, :842) and the strip shifts by BYTE lengths (`p.len()`, :930-932): the mix stays; `cell.hyperlink()` allocates per call: not hoisted. Kept by: a verbatim move with the old `Extracted` as the private `Scraped`.
- **P9-T18 The `Processor` is one long-lived instance per model; `stop_sync` follows EVERY chunk** [ti, sm] (T:548, 608-613). A synchronized update opened in piece 1 ends after piece 1; the startup CUP (P:447) goes through the same `Processor`. Kept by: `Inner.processor` lives with the screen; `pass_through` and `advance_model` share one private `advance` helper.
- **P9-T19 `OutFilter` details** [ti, sm] (T:386-469). The UTF-8 tail goes BEFORE an escape carry (`tail.append(&mut self.carry); self.carry = tail;`, :461-463); `lf_to_ind` checks `data[i - 1] != b'\r'` against the carry-prefixed data only, so a CR from the previous batch does not count (:402); the filter's private `alt` (:414-421, reset by `ESC c`) is NOT the model's alt screen; DECSTBM applies only when `top < bottom` (:430); the strip list outside the alt screen is `?9001h/l`, `?u` with an empty rest, `=|>|<..u` when numeric, `>..m` when numeric (:436-442); an overlong sequence passes through (`SEQ_MAX`). Kept by: a verbatim move; `lf_to_ind` stays a `pub(crate)` field because two locked tests write it. Pinned (4 tests).
- **P9-T20 `tag` and `mark_line_start`** [ti, sm] (T:992-1032). `tag` uses `vte::ansi::Hyperlink { id: None, uri }`, `mark_line_start` builds a `term::cell::Hyperlink::new(None::<String>, ..)` (two types; the cell type draws a process-global id counter for a `None` id, never compared: `has()` compares `uri()`); `mark_line_start` reads the template link `tag` just set (:1006); cells before the cursor that are ours or untagged become the prompt tag, ours at or after the cursor get the template link, and it continues over WRAPLINE rows only when `from` is `Some` (:1024-1030). Kept by: `cell_tagger::apply` executes the commands in the order received; verbatim bodies.
- **P9-T21 Resize is a triple and compares the MODEL's size** [ti, sm] (P:1194-1212). Zero or a size equal to the MODEL's (not the PTY's) is ignored; the order is PTY master, model, filter, `drop_clear`, then the quiet spell armed LAST. Kept by: `ResizeTerminalUseCase` reads `screen.state().size()`; `TerminalScreen::resize` bundles the three.
- **P9-T22 `set_theme` is unconditional; `take_clear` is taken LATE; ONE renderer per session** [ti, sm] (R:113-116; P:772-773; `sent` R:52 is never cleared). `set_theme` clears the last frame even for an equal theme and runs at two moments (late OSC 11 only when a colour came, P:1028; every settings change, :1295); a panic above `take_clear` leaves the clear cached for `guard`; `frame` also calls `take_clear` when `compose` is `None` (R:136). Kept by: no equality check in the port or adapter (a new test pins it); the screen owns the renderer it was built with for life.
- **P9-T23 Scrollback differs between production (2000) and the tests (10000)** [ti, td] (P:415-419 against `T:1058`, `P:1802`). `extract_prompt_scrolled_by_output` needs scrollback only. Kept by: `new` (2000, const) versus `#[cfg(test)] for_tests`; a test pins 2000.
- **P9-T24 `defer` is a platform decision** [td, sm] (T:555 `cfg!(windows)`; tests force `false`, one forces `true`). Kept by: a constructor parameter; the launcher passes `cfg!(windows)` unchanged; the windows check compiles it.
- **P9-T25 The tracker gets `Shell::from_name(program)` (an `Option`), NOT the fallback shell** [td, sm] (P:424-429): `None` for an unknown program keeps colour-based suggestion detection off. Two shells exist (P9-T50).
- **P9-T26 `RefCell` discipline** [ti]. Port methods borrow once and call no other port method; `FrameInput` and `EditBufferContext` borrow from OTHER objects (aggregate, tracker), never from the screen; a panic inside a borrowed section unwinds the `RefMut` (no poisoning), so `guard` can still `take_clear`. The only way to see `BorrowMutError` is re-entrancy: a code-review item, not a rule.
- **P9-T27 Nothing the tracker does moves the cursor** [td, ti]. `Tag`, `ClearTag` and `MarkLineStart` only write the cursor template and cells (alacritty_terminal 0.26 `term/mod.rs:1874-1877`, T:1013-1022), so reading `cursor_column()` before the commands equals reading it after (T:693). Do not add a command that moves it without revisiting R5.
- **P9-T28 Payloads are lossy UTF-8** (T:144): an invalid `Dir` already carries U+FFFD when it reaches `parse`.

### Session rules (9.3)
- **P9-T29 `refresh` order** [sd, sm] (P:811-874). `unechoed` is reconciled BEFORE `cur` is replaced, against the lock's `expected` else the OLD `cur` (:819-827); then `cur = new`; then the lock (:829-842: inside its deadline and not applied, return with no request; after the deadline and before the cap with `buf == before` (nothing echoed) re-arm and return; otherwise release); then `let Some(e) = &self.cur else { return }` BEFORE the grace.
- **P9-T30 The grace clear is broad** (P:844-847): `_ => self.redraw_until = None` clears the grace for ANY buffer that is not (blank AND inside the grace); untouched when `cur` is `None`. Kept by: the same `match`.
- **P9-T31 A blank line is a request** (P:848-864). Dedup against `sent` happens after the grace; a whitespace-only line still does `req += 1`, sets `sent` and `changed_at`, cancels `llm_at`, error-resets the menu, clears `echo_pending` and returns without `Complete` (a late reply for the old request is then dropped). Kept by: `Option<CompletionQuery>` is `None` but `request_id()` advanced.
- **P9-T32 `on_event`** [sd, sm] (P:781-808). `NewCmd(None)` (a prompt redraw) arms the grace, polls the files and RETURNS before any reset: `unechoed`, `echo_pending`, `ghost`, `llm`, `llm_at`, `lock` stay (the `prompt_redraw_*` tests). The other arms call the engine FIRST (`command_finished`/`command_ended`), then `error_reset`, `sent = None`, `poll_files`, then the shared resets, which do not touch `held`/`held_at`; `PreExec` only error-resets and keeps `sent`. Kept by: `on_shell_event` returns "look at the files" and has explicit arms (D8); the use case's order is engine call, state method, poll. State-before-engine is unobservable (the engine call neither reads nor writes the session); engine-before-poll must hold because `CommandFinished` has to reach the engine before `ApplySessionContext`/`ApplySettings`.
- **P9-T33 Strict `>` in `echo_pending`** (P:880): cleared only by a reply to a LATER request than the unmodelled key's (`EchoHold::answered_by`).
- **P9-T34 Ghost update rule** (P:883): the local ghost is replaced only for a Parse reply or any reply that carries a ghost; a Generators reply without a ghost keeps the old one.
- **P9-T35 LLM debounce** (P:888-895): needs Parse, no local ghost, `cursor == buffer.len()` in BYTES and `llm_enabled` (= `inline.enabled` AND `source` resolves to a Profile); measured from `changed_at`, not from now; `i64(INLINE_DEBOUNCE_MS).max(0) as u64` (the wrong-type fallback stays in `Settings::i64`: `250.0` reads as the default 300). Pinned (`llm_ghost_requested_for_commands_without_spec`).
- **P9-T36 Spinner** (P:896, 1249-1256): armed only when idle; ticks only while `menu.last().is_loading()`; `menu.loading` is judged at a FRESH instant (`tick_spinner(now, judged_at)`).
- **P9-T37 `ghost_text`** (P:914-931): order `[local, llm]`; `typed` uses `basis()?` so `None` propagates; none with inline disabled, a command running, the alternate screen, an empty buffer, a cursor not at the end, or a shell ghost. Kept by: `GhostCandidates::rest_after` keeps local first.
- **P9-T38 `note_forwarded`** (P:946-960): the at-end test reads `cur`, not the basis; ANY key that is not a plain char, or a plain char not at the end, clears `unechoed` and sets `echo_pending = Some(self.req)` (the current `req`, not +1). The same predicate counts `typed` (P:1111-1113). Kept by: `forward_key` + `key_router::plain_char`.
- **P9-T39 `typed` counts only in the Forward arm** (P:1097-1125). The ghost-accept arm and the menu arms `continue` or fall through without counting; `prompt` requires `typed < 2` BEFORE the key (:1061); `continue` skips `redraw = true` in the Forward and AcceptGhost arms, so Ctrl-C/Ctrl-D/Enter set it themselves via `forward_key -> bool`.
- **P9-T40 Enter bookkeeping order** (P:1107-1114): `submitted = basis()` (uses `unechoed`) runs BEFORE `note_forwarded` mutates it, only when the TRACKER's `at_prompt()` holds; the `Write(b"\r")` path (:1133-1135) does the same check. Kept by: `forward_key` order and `wrote_to_shell`.
- **P9-T41 The hold** (P:1078-1088). `acts` counts a ghost key EVEN when an Action arm will win the match, and does NOT count the global show/toggle arm (so `showAutocomplete` is never held); the hold keeps `chunk[ev.bytes().start..]`, sets `held_at = now + INSERT_LOCK`, RESETS the decoder, and `break`s (`fwd`/`redraw` collected so far still go out after the loop); when `held` is non-empty on entry `on_keys` appends and returns BEFORE decoding (:1045-1048): decoder state untouched and the injected-panic hook does not fire. Kept by: `route` returns `Hold` from the same `acts` expression; the handler checks `is_holding_keys()` first.
- **P9-T42 `insert`** (P:1161-1190). Dropped while a command runs or without a buffer (and then `record_insertion` is NOT sent); the basis comes from `menu.last()` first, `cur` second; reconciliation is against `basis()`; `unechoed.clear()`; `submitted = expected` when it executes, BEFORE the lock; the lock after; `before` = the `cur` buffer else `ib`; the record AFTER the lock. Kept by: `insert -> Option<Vec<u8>>` and `if let Some(bytes) = session.insert(..) { .. record }`.
- **P9-T43 Insertion arithmetic is in chars** (P:161-194): `chars().count()` for the extra typed chars, `delete` CHARS popped, a non-boundary cursor falls back to `ib.len()` (:182-186) and `ib[..cursor]` is sliced only after the boundary check. Pinned (the `aé`, `stéé` rows). Moved verbatim.
- **P9-T44 `poll_files`** [sd, sm] (P:1272-1303). The session context first, settings second; the context mtime is stored BEFORE the file is read (an unreadable file is not re-read until it changes); the context reloads only when its mtime is `Some` and different, so a deleted file is no change and keeps the old time; settings reload whenever the `Option<SystemTime>` differs INCLUDING `Some -> None` (a deletion reloads the defaults); then `apply_settings` (settings and keymap), `set_theme(resolve_theme(..))` unconditionally, `engine.apply_settings(clone)` LAST; returns true only for settings. Pinned (the 5 context tests).
- **P9-T45 `take_repaint`** (P:1216-1222) needs no pending resize and not alt; the flag is consumed only when it returns bytes.
- **P9-T46 The exit state machine** [sd, dr] (P:648-651, 665-680, 599-607). `PtyEof` continues only while there is no exit code; `ChildExit` stops at once if EOF was seen, else arms the 200 ms grace (`EXIT_GRACE`); `Term` sets 129 for SIGTERM and SIGHUP alike and stops; the exit code is `unwrap_or(0)`; `exit_at` is checked after every message; `Disconnected` ends the loop. Kept by: `ShellExit` and `HandleShellExitUseCase` return exactly these bools.
- **P9-T47 `suppressed`, `drawable` and `compose` must stay equal** [sd, sm] (P:974-1000 against R:177-186). `FrameInput.suppress` is `suppressed()`, NOT `drawable()` (P:1008): the renderer applies the screen guards itself; `drawable` gates key interception only. The conditions are `columns >= 10`, `lines >= 3`, ALT|ORIGIN|INSERT, `input_needs_wrap`, scroll region. Kept by: `render_guard::allows_drawing` and the parity test `should_draw_a_frame_exactly_when_the_render_guard_allows_drawing` (9.3a); `compose` is not rewired (a byte-hashed Phase 8 file).
- **P9-T48 `alt` is the MODEL's `TermMode::ALT_SCREEN`** (P:741-743), never the filter's private `alt`. `ScreenState.alt_screen` is filled from the model.
- **P9-T49 `suppressed()` reads `cur.is_none()`** (P:978): the cursor off the command line (an inline TUI's rows) hides the menu and keeps its hands off the keys.
- **P9-T50 Two shells** (P:424-429, 753-755): `launched_shell` (`Shell::from_name(program)` else Pwsh on Windows, Bash elsewhere: `cfg!(windows)` stays in the launcher) against the tracker's raw `Option` that `Shell=sh` can REPLACE by `None`; `shell() = tracker.shell().unwrap_or(launched)`. Never merged. Read at request time, also for the LLM request (P:1246).
- **P9-T51 `next_deadline`** (P:721-739): in passthrough only `exit_at` counts; otherwise `poll_at` is always `Some`, so the loop never blocks forever; the lock contributes its DEADLINE, not the cap. It runs OUTSIDE `guard`: it must not panic (pure `min`). Pinned (`panic_turns_the_session_into_a_pass_through` asserts it after `ChildExit`; each of the eight sources one by one by `should_wake_for_each_source_of_a_deadline_on_its_own`, 9.3g).
- **P9-T52 Boundary semantics**: "due" is `d <= now` (P:599, 964, 1226, 1238), "still inside" is strict `<` (:833, 845, 1026), the poll is `now >= poll_at` (:1257). Entities test both sides of each boundary.
- **P9-T53 Panic tiers** [sd, dr, sm] (P:59-70, 702-711). A panic inside `guard` (Stdin, Pty, Resize, Completion, Ghost, `on_timers`, including the late-probe strip) turns the session into a pass-through and the child keeps running; a panic outside (the batching, `next_deadline`, the `ChildExit`/`PtyEof`/`Term` arms, `cleanup`, `launch_session`, the shell resolution) reaches the outer `catch_unwind` and `exec`s a NEW fallback shell. Domain methods must not panic where the legacy did not. Kept by: the runner keeps the single outer `catch_unwind` (`attempt`) and the same `guard` arms; the use cases run inside `guard` exactly where `Session::on_*` ran.
- **P9-T54 The late probe window** [sd, uc, sm] (P:1022-1042). `probe_until` is checked with `Instant::now() < t`, but cleared ONLY when a CPR arrives (an expired window stays `Some`); during the window EVERY CPR- or OSC 11-shaped sequence of user input is consumed; a background reply sets `terminal_bg` and re-themes with the CURRENT settings (`set_theme` unconditional); the strip runs inside `guard`. Pinned (`late_probe_reply_is_not_typed`).
- **P9-T55 `apply_completion` takes the reply by value** (P:876): it reads `buffer()/cursor()/ghost()/is_loading()` before `menu.on_completion(c, ..)` consumes it; a stale reply returns before ANY state change, so the use case must also skip `redraw` and `release_held`.
- **P9-T56 `release_held`** (P:963-972): returns without clearing anything when `held` is empty or when `echo_pending` holds and the hold has not timed out; on release it clears `echo_pending` and `held_at` first, then replays through the FULL `on_keys` (decodes again). Reached from three events (P:778, 901, 1237).
- **P9-T57 `raw` before `on_input`** (P:619): `!menu.intercepting(&settings)` uses `intercepting`, not `drawable`; on a panic the read is forwarded only if `raw`. Kept by: `menu_intercepts()` evaluated before the guard.
- **P9-T58 Progressive mutation** [sd]. `on_keys` may panic mid-loop (the injected hook is the first statement of the per-key loop); the state mutated by earlier events stays mutated, and `step` can still forward the read raw because the PTY write is the last statement. Aggregate transitions are therefore called in the same order, one per event, never batched into a command list.

### Use-case order contracts (9.4, 9.5)
- **P9-T59 One stdout write per batch, `release_held` AFTER it** [uc, gv] (P:773-778). `on_output` builds `clear + body + repaint + frame` and emits ONCE; `release_held` may then emit a bell, a second frame and write to the PTY. Kept by: `ProcessShellOutput` writes once inside `execute` and returns `()` (R21). The plan's "returns one byte buffer, the driver writes once" cannot be followed.
- **P9-T60 `take_clear` is taken LATE** (P:772-773), after `refresh`; never hoisted above the feeder or the refresh; `guard` and `EndSession` take it too. Pinned where a successful batch cannot show it: a panic INSIDE the refresh leaves the clear cached for `guard` (`should_emit_the_cached_clear_then_the_raw_batch_and_return_the_credits_when_the_refresh_panics`, 9.0d; `should_take_the_clear_after_the_refresh` with a panicking engine double, 9.5d).
- **P9-T61 A panic in `release_held` after the emit makes the driver emit the batch AGAIN raw** (P:641-643: `!guard(on_output)` leads to `emit(&b)`). An existing quirk (duplicate output): kept, by leaving `release_held` inside `execute` after the write. Do not "fix" it (P9-N4). Pinned by `should_write_the_batch_again_raw_when_release_held_panics_after_the_frame` (9.0d).
- **P9-T62 The bell is inline** (P:1139): written at the effect, BEFORE the rerequest, the frame and the PTY write.
- **P9-T63 The PTY write is the LAST statement of `on_keys`** (P:1155-1157), so that after a panic above `step` can forward the read raw. Kept by: `shell.write_input(forward)` last in `KeystrokeHandler::handle`.
- **P9-T64 rerequest before redraw before the PTY write** (P:1148-1157): `forget_sent` + `refresh` (which can send `Complete`), then `redraw`, then the write, each ONLY under its HEAD condition (`if rerequest`, `if redraw`, `if !fwd.is_empty()`, pty.rs:1148-1157); `ToggleFuzzy` calls the engine immediately and then rerequests.
- **P9-T65 The ghost reply rule** (P:685-693): accepted only if `req == s.req && req == s.llm_req`; built from the CURRENT `sent` buffer through the `zip`; a `None` text clears the LLM ghost; then `redraw()` with NO `release_held` (unlike `on_completion`). Pinned (`should_show_the_llm_ghost_text_of_the_current_request_behind_the_local_one`).
- **P9-T66 Per-key state is recomputed for EVERY key** (P:1061-1077): `prompt`, `shown`, `intercepting`, `global`, `tab_show` and the ghost zip (the eager `self.ghost_text(true)` argument is pure) because the menu changes between keys. Kept by: `route_key(&key, typed, &screen.state())` per key, `screen.state()` per key.
- **P9-T67 The tick order is fixed and lazy** (P:1224-1269): resize, then redraw grace (refresh), `release_held`, lock due (evaluated AFTER `release_held`, which may create one), LLM (the schedule is dropped even when `sent` is `None`; `llm_req = req` only when it sends), spinner (reschedules from the tick's `now`, judges "loading" at a fresh read), poll (uses the tick's `now`), then repaint+frame in ONE write else a plain redraw. The poll comes BEFORE the repaint and frame because a reload runs `set_theme` and applies the new settings: pinned by `should_poll_the_files_before_drawing_the_repaint_and_the_frame` (9.5g).
- **P9-T68 Repaint and frame go out in one write, else a plain redraw** (P:1261-1268); `take_repaint` drops the cached clear before `frame`.
- **P9-T69 `cleanup` order** (P:1306-1316): ONE write of `clear + \x1b[0m\x1b[?25h`, `engine.shut_down()`, `kill_live_children()`, `modes.restore()`, then removing the session context (only if the id exists). Kept by: `EndSessionUseCase` holds `TerminalModes` (R22). `cleanup` is outside `guard`.
- **P9-T70 `ScreenState` is a snapshot** [ti, sd]: never reused across a `pass_through`, `apply` or `resize`. `ProcessShellOutput` takes `before`, then a new snapshot after the feeder (P:762, 768); `release_held` is reached AFTER the emit, where a snapshot taken before the feed is stale.
- **P9-T71 cwd and shell of a request are read at request time** (P:745-755, 865-873, 1246): `cwd` is the tracker's `Dir` else `std::env::current_dir()` else empty (lazy `or_else`: the syscall happens only when the shell never reported a directory). Capturing once at start would give another answer when the directory is deleted. Kept by: `WorkingDirectory` read per call.
- **P9-T72 Setting defaults and wrong-type fallbacks** [sm] (P:880, 892, 902, 1036-1037). `bool(inline.enabled)`, `i64(inline.debounceMs).max(0)`, `bool(autocomplete.onlyShowOnTab)`, `bool(autocomplete.disable)` use the same keys through the same `Settings` getters (the catalog agrees today; the wrong-type fallback stays in the getters). Application code reads no setting itself.
- **P9-T73 `emit` writes nothing for an empty buffer** [gv] (`write_stdout` skips empty, P:324-326; `redraw` of an identical frame emits an empty buffer) and its `cfg!(test)` branch (P:714) becomes the injected writer. The capturing double extends by nothing for an empty slice. Start-up writes (P:433, 448, 496) bypass `emit` and go through the same writer.
- **P9-T74 Credits** [uc, dr] (P:644-646): one `try_send` per chunk AFTER the handler, also after a panic; the batch is one `Pty` message plus all queued ones. Pinned after a caught panic by `should_emit_the_cached_clear_then_the_raw_batch_and_return_the_credits_when_the_refresh_panics` (9.0d): a runner that returned early from the `Pty` arm after a failed `guard` would skip the `try_send` and starve the reader.
- **P9-T75 Real-clock tests** [sm, sd, uc] (P:2208, 2224, 2168). `slow_echo_keeps_the_insertion_in_the_basis`, `split_prompt_redraw_keeps_the_menu` and `keys_after_an_unmodelled_key_wait_for_its_echo` set a field to `Instant::now()` and then tick: the harness keeps `SystemClock` (R19).
- **P9-T76 Engine replies share the PTY inbox** [sm, dr] (engine_server.rs:180, 193, 200): `Completion(Box<CompletionResult>)` and `Ghost` arrive interleaved with `Pty`, `Stdin`, `Resize` and `ChildExit`. One `Sender<SessionMessage>` keeps ordering and the stash batching.
- **P9-T77 The harness engine records from construction; the id is required** [uc]. The HEAD tests swap `s.engine` after `session()` and before the first `prompt`; construction makes no engine call, so the recorded sequence is the same (`no_history_command_still_ends` filters out `Complete` and expects exactly `[CommandEnded]`). `poll_files` skipped the context when `session_id` was `None`; an in-memory repository with nothing saved reports `None` too.

### Driver, start-up and the CLI (9.4, 9.6)
- **P9-T78 Panic hook** [dr, sm] (P:79): `std::panic::set_hook(.. logger::error("panic: {info}"))` is process-wide, set FIRST, and stays installed after `run` returns (the debug shell's code runs after). Kept by: first statement of `run_pty_session`.
- **P9-T79 Shell resolution and argv** [dr, sm] (P:80-97, 112-130). Explicit, then `TABSMITH_SHELL` (non-empty), then Windows `"pwsh"`, else `$SHELL` (non-empty; Windows never reads it); login only when `TABSMITH_IS_LOGIN_SHELL == "1"`; a first extra argument not starting with `-` replaces the shell even when `shell` is `None`; `--login` only for Bash/Zsh/Fish (by `Shell::from_name`, so paths) and BEFORE the extras. Pinned (`child_argv`). Kept by: `PrepareSessionLaunchUseCase` + `WrappedShellCommand::new`; the argv is computed BEFORE `catch_unwind` and feeds the fallback.
- **P9-T80 The `modes` hand-off** [dr] (P:411-412, 100-102): `*modes = Some(m)` runs right after `sys::enter`, so every later failure still restores raw mode before the fallback prints. Kept by: `modes: &mut Option<Rc<dyn TerminalModes>>` set on the line after `enter()?`.
- **P9-T81 Start-up order** [dr, sm] (P:401-576). tty check (:402), program (:405), first settings load (:408-410, may log a warn line), `enter` (:411), size (:413), session id (:423), probe write and read (:433-443), the late window (:445), the CUP or `\x1b[H\x1b[2J` (:446-449), then everything that can fail before the child exists: truecolor, theme, `detect()` (spawns `tmux display -p`), renderer, `spawn_resize` (registers the signal handlers: a SIGTERM from here on is a `Term` message), engine start (:463, inside `catch_unwind`), `openpty`, `CommandBuilder` (:471-486), `spawn_command`, `drop(pair.slave)` (REQUIRED for EOF), `try_clone_reader`/`take_writer` (still fallible after the child exists), the info log (:491), the title OSC (:495-496, AFTER the spawn), the reader/writer/wait threads (:499-520), `spawn_input` (:521), the typeahead (:522-524), then the session literal with `settings_mtime` read at the END (:542). Kept by: `launch_session` is a line-for-line port; the screen is built after the probe (R12). 9.0 (`pty-diff.py`).
- **P9-T82 The probe** [dr, sm] (P:433-449). `PROBE` goes out through the writer directly (not `emit`); the loop ends ONLY on a CPR or after 300 ms; the late window exists only without a CPR; with a CPR the model gets a synthetic `\x1b[{r};{c}H`, without one `\x1b[H\x1b[2J` clears the real screen; leftover bytes (typeahead) go to the PTY writer AFTER `spawn_input`; a reply cut by the timeout is forwarded as typeahead, only a whole late reply is dropped. Pinned (`probe_replies`: the OSC 11 query comes before the CPR).
- **P9-T83 Child env and cwd** [dr, sm] (P:471-486). cwd = `current_dir()` if `is_dir()`, else the USER home (`user_home_from_env`, not the data dir); `TABSMITH_TERM=<CARGO_PKG_VERSION>`; `TABSMITH_TERM_TMUX` only if `TMUX` is set; `TABSMITH_SESSION_ID`; `TABSMITH_SHELL`/`TABSMITH_IS_LOGIN_SHELL` removed. The fallback sets `TABSMITH_TERM_DISABLED=1` instead (:145-147) and removes the same two.
- **P9-T84 Reader, writer and credits** [dr, sm] (P:231-245, 499-516). `sync_channel(4)` pre-filled with 4 sends; the pump takes a credit BEFORE each read, retries `Interrupted`, maps any read error to EOF (Linux EIO) and ALWAYS sends `PtyEof` last; the 16 KiB buffer. Pinned (`pty_reader_waits_for_credits`).
- **P9-T85 The session id** [dr, sd, sm] (P:312-322): 32 lowercase hex digits `{:016x}{:016x}` of `(nanos as u64, pid ^ nanos.rotate_left(17))`; the shell scripts echo it in `NewCmd={sid}`; `SessionId::new(sid).ok()` of it always succeeds. 9.0 (`should_generate_a_session_id_of_32_lowercase_hex_digits`) and the literal test of 9.1d.
- **P9-T86 The main loop and batching** [dr, sm] (P:579-647). The stash is consumed BEFORE any blocking recv; `Disconnected` ends the loop; after EVERY message and EVERY timeout `exit_at <= now` is checked, then `on_timers` runs unless passthrough; ALL queued `Pty` chunks are concatenated and the first other message is stashed and handled next; in passthrough the RAW batch (markers unstripped) is emitted. 9.0 (`batching`, `passthrough`; the 13 loop tests).
- **P9-T87 Resize in passthrough** [dr] (P:652-661): only the master PTY is resized, not the model; `Msg::Resize` is POSITIONAL `(cols, rows)` while `PtySize { rows, cols }` is named. Kept by: `WrappedShell::resize(columns, rows)`; a new test pins the order.
- **P9-T88 ConPTY** [dr, sm] (P:665-676): the master is dropped on a helper thread because `ClosePseudoConsole` may wait for a reader that waits for credits from the main loop (R23). Pinned (`conpty_is_closed_off_the_main_loop`, Windows CI and the windows check).
- **P9-T89 Field drop order at the end of `run(mut self)`** [dr]: HEAD drops `rx`, `engine`, `writer`, `credits`, `master`, ... in declaration order; dropping `writer` ends the writer thread, dropping `master` closes the PTY. Unobservable on Unix; only ConPTY cares. Kept by: `SessionRunner`'s field order.
- **P9-T90 `PtyWrappedShell` is shared** (`Rc`) between the runner and several use cases; the master lives in a `RefCell` inside it with tiny borrow scopes, so a panic can never leave it borrowed across `guard`.
- **P9-T91 Lenient settings text** [dr, uc] (`load_or_defaults.rs:11-24`): `settings: {display_chain(error)}` at warn level, then `Settings::default()`; logged inside the call; `settings_mtime` is read at the END of `start`, so a change during the probe is reloaded at the first poll. Kept by: `LenientLiveSettings` (the same `warning` text; the settings tests of `load_or_defaults.rs` stay where they are, `doctor.rs` still uses it).
- **P9-T92 Bootstrap import graph** [dr, gv]. terminal_session infrastructure may not name `crate::bootstrap` (rule 3), so the starter is injected; the `debug_shell_survives_closed_terminal` child calls `crate::bootstrap::cli::terminal_session_module()` from `#[cfg(test)]` code (exempt, as the HEAD test already imports `bootstrap::channel_completion_engine` at C:425).
- **P9-T93 The CLI commands** [dr, sm]. `key_tester` (C:201-228): the banner is printed BEFORE raw mode, lines end `\r\n`, it ends ONLY on a read that is exactly `[4]` or on EOF and prints `ctrl-d` in both cases, `disable_raw_mode()?` runs before `res?` so its error wins. `debug_shell` (C:231-262): no argument gives Pwsh on Windows else `$SHELL`'s shell or `Could not determine current shell or shell not supported`; `exe` is `$SHELL` only when it names the same shell; the post script is rendered BEFORE the data root is resolved; `ZDOTDIR` only for zsh, `TABSMITH_DEBUG_SHELL=1` always, with `set_var` before any thread of the run; output through `let _ = writeln!` (never `println!`: the terminal may be gone); the rc dir is removed after `run`, errors ignored; returns `Ok(code)`. `debug_shell_files` (C:266-304): a recursive `DirBuilder` mode 0700 (all created levels), the rc file by plain `fs::write`, a stale `debug-shell-<pid>` dir removed first, the exact argv shapes. Guarded by `debug_shell_survives_closed_terminal` and `debug_shell_rc_dir_and_quoting`.
- **P9-T94 The D9 path** [dr, sm]: `NAME` (C:427) is the libtest path used to re-exec the test with `--exact`; it MUST become the new module path or the child runs 0 tests.
- **P9-T95 cfg and platform code moves verbatim** [dr]. `fallback` (P:148-154), `sys` (P:1319-1604), the windows test (P:2506-2536), `signal_death_exit_code` (P:1752), `debug_shell_files` unix mode (C:278), `cfg!(windows)` at P:83, 139, 424, T:555, C:235. The unix `wait` retries on EINTR, any other error returns 1, a signal returns 128+sig; the SIGWINCH thread reads `crossterm::terminal::size()` inside the signal thread and skips `Err`; a SIGTERM and a SIGHUP both become `Term` (129). Every step that touches one runs the windows check and the cfg multiset grep.

### Governance and the plan's sentences
- **P9-T96 testlock keys by fn name over the whole tree** [gv]: a test moved by copy instead of cut is a DUPLICATE; a renamed one is MISSING plus NEW. New names are unique across the phase (R32); a moved test keeps its name.
- **P9-T97 Early registration and test-support files** [gv] (probes V1-V4): a `#[cfg(test)] use <infrastructure path>;` plus an unannotated body using the short name is clean, the same `use` without the attribute is a violation (`application uses infrastructure`), an inline full path in an unannotated body is a violation, the same inside a `#[cfg(test)] fn` is clean. P8-T49 is closed with its boundary: the scanner counts `(`/`[` only inside a BODYLESS `#[cfg(test)]` item (probe V5: `#[cfg(test)] const OPEN: &[u8] = b"\x1b[";` followed by a production line that breaks a rule is a silent miss; written `\x5b` it is read); none exists at HEAD in terminal_session, pty.rs, tracker.rs or cli.rs (grep B17). Big test modules go to sibling `tests.rs` (exempt wholesale).
- **P9-T98 Rule-3 hits of the legacy code, by experiment** [gv]. `pty.rs` placed verbatim in `infrastructure/pty/` trips rule 3 seven times (P:16 `FileThemeRepository`, :22 `JsonFileSettingsRepository`, :23 `load_or_defaults`, :136 and :1311 `kill_live_children`, :409 and :457 the `new(..)` uses): injected by 9.4a-9.4c. `cli.rs`'s debug shell trips it five times (C:9, 11, 12, 13, 246: `RenderInitScriptUseCase`, `EmbeddedShellScripts`, `OsShellEnvironment`, `init_command`): a bootstrap callback. `tracker.rs`'s infrastructure half is clean. The application layer cannot take anything that mentions `alacritty_terminal`, `portable_pty`, `crossterm`, `libc`, `anyhow`, `logger`, `eprintln`, `crate::types`, `crate::tracker`.
- **P9-T99 The old call order per event** (the table the journal tests reproduce) [gv] (P:758-780, 1044-1159, 876-902, 1224-1270, 1306-1318): `on_output`: re-arm `resize_at`, feed, drain events (engine THEN `poll_files`), `error_reset` on alt exit, `refresh`, ONE emit, `release_held`; `on_keys`: per key `maybe_panic` first, effects in order (`Insert`, `Write`, `Bell` = its own emit, `Rerequest`, `ToggleFuzzy`), AFTER the loop `refresh`, `redraw` (one emit), LAST the single PTY write; `on_completion`: stale drop, ghost, debounce, spinner, `menu.on_completion`, `redraw`, `release_held`; `on_timers`: resize, grace, `release_held`, lock, llm, spinner, poll, ONE emit or a plain redraw; `cleanup`. All emit sites: P:642, 709, 777, 1018, 1139, 1265, 1309. The interleaving of a stdout write and a PTY write INSIDE one event is not observable in the old code (`sink` and the writer channel are two sinks) and production is not touched to make it so; the new `IoJournal` tests pin it and `session-diff.py` checks what the user sees.
- **P9-T100 The debug shell's exec fallback skips cleanup** [gv]. With stdin not a terminal `start` fails and `fallback` `exec`s the shell (P:148-149): `remove_dir_all(&dir)` and "Ending tabsmith debug shell" (C:258-261) never run, so `run/debug-shell-<pid>` stays and no goodbye prints. That is behaviour; `run_debug_shell_command` must not clean up on the error path (`session-diff.py` `failures` records it).
- **P9-T101 Plan sentences that cannot be followed** [uc, gv, sm]: "returns one byte buffer, the driver writes once" (P9-T59); "late probe replies stripped by the driver" read as the driver's guarded handler (P9-T53, P9-T54); "the `#[cfg(test)]` panic injection stays at the driver's call site" (it sits inside the per-key loop, P:1055-1056: it would fire for held chunks and for chunks that decode to nothing; it stays in `KeystrokeHandler`); "`modes.restore` stays in the driver" (R22); "Term goes to EndSession" (R24); "7 pure tests" (after Phase 7 five pure `pty.rs` tests remain: `insertion_bytes_reconcile`, `expected_buffer_after_insertion`, `unechoed_keys_are_consumed_by_their_echo`, `child_argv`, `ghost_keys_and_words_keys`).

### Noticed and NOT fixed (agent brief: report; each is behaviour today)
- **P9-N1** A Windows `PreExec` that arrives in the same batch as its still-queued `NewCmd` is ignored (P9-T6).
- **P9-N2** A bare `Dir` marker sets an empty cwd that `cwd()` does not fall back from, so generators get `""` (T:656, P:745).
- **P9-N3** The end-of-batch deferred flush can starve while every batch carries a marker (T:601).
- **P9-N4** A panic in `release_held` after the emit makes the driver write the batch twice (P9-T61).
- **P9-N5** `try_clone_reader` and `take_writer` can fail after the child exists, orphaning it (P:489-490); a panic in `on_output`'s final frame loses the clear it already took (`ponytail` note at P:701).
- **P9-N6** `settings_mtime` is read at the end of `start` (P:542): a settings change during the probe is only seen at the first poll.

## Decisions

R1-R46 are binding; behaviour first (D1). Each gives the options considered and the one taken. Where two area notes disagreed
the decision says which one won and why. "Area" names the researcher whose note proposed it (`tracker-domain`,
`tracker-infra`, `session-domain`, `use-cases`, `driver-cli`, `governance`).

### R1 Phase start, guards first, three baselines, six runs
- `phase9-start` is the local lightweight tag at `858b7b5` (its `src/` equals `5efb4b2`; the two later commits touch `scripts/`
  only). Steps **9.0a-9.0d** precede every move: the registration and the scripts, a fixture helper and three tracker scenarios,
  fourteen session scenarios, thirteen loop tests and the id-shape test. All pass on the old code (a characterisation test has no
  RED, Phase 8's 8.0b precedent). After 9.0d the orchestrator sets the local tag **`phase9-guards`**: the third `testlock.py`
  baseline and the reference of the frozen-fixture check (`git diff phase9-guards -- tests/fixtures/terminal_session/characterisation`
  is empty at every later step).
- The 42 steps run in six runs (see "Steps"). Run ends are the orchestrator's verification points.
- Rejected: Phase 8's FNV hash tables (opaque on failure: a mismatch must say WHICH step of WHICH event changed); a recording hook
  inside `pty.rs` (a production edit before the move); one giant golden file (cannot say which event broke).

### R2 Step order: tracker first, then session domain, ports, services and use cases, driver (governance, all areas)
Plan order is 1 tracker domain, 2 tracker infrastructure and delete `tracker.rs`, 3 session domain, 4 ports, 5 use cases, 6 driver and
CLI, 7 delete `pty.rs`. Taken, in execution order: guards (9.0), tracker domain (9.1a-d), the legacy `Tracker` delegates (9.1e), tracker
adapters (9.2a-d), the screen switch (9.2e), the feeder and `tracker.rs`'s deletion (9.2f), the independent driver moves (9.6a-b), pure
session services (9.3a-b), session entities and the aggregate (9.3c-i), ports (9.4a-c), the CLI commands (9.6c), services and use cases
(9.5a-h), the runner (9.6d-f), docs (9.7). Why: `ScreenState` must exist before the render guard (9.3a); `SessionMessage` (9.6a) before the
collaborators injection (9.4a); the aggregate before the services; `pty.rs` is free of rule-3 imports only after 9.4a-c, which is what
lets 9.6d `git mv` it into `infrastructure/pty/`.
Rejected: (a) the tracker-infra note's "adapters first against the legacy Tracker, domain after" (its I4 needs `EditBuffer`, `CellTag`,
the colour value objects and a context VO that only the domain steps create, and the legacy `Tracker` would be rewired twice); (b) the
plan's literal order (its step 2 deletes `tracker.rs` before any port exists).

### R3 Register `terminal_session` first (9.0a); no module-edge rule (governance G1, G2)
Registering passes at HEAD with zero violations (55 architecture tests; re-run on a copy of HEAD by the synthesizer), so from 9.0a the gate
rule-checks every Phase 9 commit instead of Phase 8's local registration per step. Consequences, all handled in the steps: legacy
`pty.rs`/`tracker.rs`/`cli.rs` are not read by the checker (they are flat files), so they may import new items; new domain and
application files may import no legacy module (`crate::types`, `crate::pty`, `crate::tracker`, `crate::cli`); every `use` of an
infrastructure path in an application or domain TEST-SUPPORT file carries `#[cfg(test)]` (R31); the test-only `terminal_session -> specs`
and `-> completion` edges (Phase 8 deviation 25) are allowed explicitly: D2 exempts test code, production edges were verified by script
(`terminal_session -> completion, dropdown, history, settings, shared`; no `specs`), and grep B3 pins it. A section 2 edge rule is a Phase 11
item (offered in O6). Rejected: registering at the end (fifteen unchecked commits on the biggest phase); a new `ALLOWED_EDGES` rule inside
Phase 9 (~60 lines and four tests of checker code, with its own cycle, for no behaviour).

### R4 The strangler: `Session` shrinks in place, no dead code, no shim across a step (session-domain D-17, task ordering principle)
Nothing is built dead and swapped later. The aggregate takes `Session`'s state fields (9.3g); then each service or use case takes over the
body of one `Session::on_*` method, and the method becomes a one-line delegate in the same commit (9.5a-h); the tests that act on it move
at the commit where the LAST use case they need exists (R32). Interim composites, each deleted by the named step: the legacy `Tracker`
(`tracker.rs`, 9.1e-9.2f), the aggregate's `pub(crate)` fields (9.3g-9.3i, R16), the `Session` delegates (9.5a-9.5h), and the file `pty.rs`
itself (9.6d). No `pub(crate) use` shim exists at any commit (grep B6). Rejected: use-cases AP21 ("the new code has no production caller until
the driver step; `pty.rs` untouched"): two implementations for five steps, drift visible only at the swap, and the moved tests would then
test code that production does not run.

### R5 Tracker meets screen: option B, a pure `MarkerTracker` driven by the application (tracker-domain 1-3, tracker-infra D8)
`MarkerTracker` takes three inputs it cannot get itself (the parsed `ShellMarker`, the cursor column, the wall-clock seconds) and answers
with a value: `MarkerReaction::{Handled, EditBufferNeeded}(MarkerEffects)`, `MarkerEffects` = the screen commands to apply now, in order, and
the events to hand on after the batch. `PreExec` is the one two-call marker: `on_marker` flushes the deferred markers and returns
`EditBufferNeeded(effects)`; the application applies them, reads `tracker.edit_buffer_context()` and `screen.edit_buffer(&ctx)`, then calls
`complete_pre_exec(scraped, now_secs)`. Valid because nothing the tracker does moves the cursor (`Handler::set_hyperlink` only writes the
cursor template, alacritty_terminal 0.26 `term/mod.rs:1874-1877`; `mark_line_start` only writes cells), so the column read just before a
marker equals the one the old code read after its own `tag` calls (tracker.rs:693).
Rejected: option A, a domain trait `MarkerScreen` and generic methods (a port inside the domain, no trait upcasting under MSRV 1.85, every
domain test needs a fake screen); a scrape closure (the closure would apply the flush commands itself, commands travel on two channels);
a tracker flag "awaiting scrape" (an invalid intermediate state to guard); a `PreExecTicket` token (one more type for one caller; O3).

### R6 `MarkerTracker` API, `end_batch`, time (tracker-domain 2-4, use-cases C-34)
`MarkerTracker::new(session_id: SessionId, shell: Option<Shell>, defer_markers: bool)`; `on_marker(&mut self, marker, cursor_column, now_secs) ->
MarkerReaction`; `complete_pre_exec(&mut self, scraped: Option<&EditBuffer>, now_secs) -> MarkerEffects`; `end_batch(&mut self, cursor_column,
now_secs) -> MarkerEffects`, which owns "flush the deferred queue only if the batch had zero markers, then reset the counter" and is called for
EVERY batch, even one with no piece (the old `feed` always ran its tail). `batch_markers` counts every marker handed to `on_marker`, `Ignored`
and locked-out ones included, so the application never skips an `Ignored` marker. `now_secs` is a `u64` the application reads from the `Clock`
port before EACH tracker call (per marker, per `end_batch`, once more after the scrape for `complete_pre_exec`): the old code read
`now_secs()` per drained entry (tracker.rs:702) and after the scrape (:744); one read per call differs only across a wall-second tick inside
one marker and cannot be seen (recorded, deviation 7). `defer_markers` is a constructor parameter: production passes `cfg!(windows)` from the
launcher (tracker.rs:555 hid it), tests pass `false`, `windows_deferral_fifo` passes `true`.
Rejected: `now_secs: &dyn Fn() -> u64` (exact per drained entry, but an unusual domain signature); one `now` per batch in the feeder (history
`start_time`/`end_time` would move to the batch start, use-cases C-34); `MarkerTracker::new(String)` (zero ripple, but `SessionId` is the glossary
VO and a generated id cannot fail it).

### R7 `ShellMarker` is a total parse; four dead fields and three markers go (tracker-domain 6, 16)
`ShellMarker::parse(payload) -> ShellMarker` never fails; the per-key trimming rule is encoded in the variant types: `NewCmd(Option<String>)`,
`Dir(PathBuf)` and `PwshPrompts{..}` keep the raw value, every other value is trimmed, an empty value (`key=`) is `Ignored`. `ShellPath`, `TTY`
and `User` parse to `Ignored` because the fields they filled (`shell_path`, `tty`, `user`) and `has_seen_prompt` are read by nobody (grep over
`src`, tests included); `User` therefore leaves the lock allow-list, which changes nothing. `passes_integration_lock` lists the five exempt
variants (`NewCmd`, `PreExec`, `NoHistory`, `OscLock`, `OscUnlock`) with explicit arms (D8). The fields die in 9.1e (earlier than the plan's
step 2, because the marker no longer parses them). `docs/SPEC.md` §5.1 (lines 679, 689) still documents them: Phase 11.

### R8 Colour value objects; `EditBufferContext` is the one name (tracker-domain 8-10 over tracker-infra D7)
The domain holds xterm-256 indexes only. `ShellSuggestionColor::matches(fg: Option<u8>, bg: Option<u8>)`; the adapter keeps `cell_col(Color) ->
Option<u8>`. The pwsh pair `(SuggestionColor, Flags)` becomes `PwshPredictionColor { colors, emphasis: TextEmphasis }`, `TextEmphasis { dim, italic }`
replacing alacritty `Flags` (`contains` = `is_within`, `intersects(DIM|ITALIC)` = `is_dim_or_italic`). `EditBufferContext<'a>` (built from
`EditBufferContextParts<'a>`, nine inputs) borrows what `extract` reads of the tracker: it exists only while a cycle is open and no command runs
(`MarkerTracker::edit_buffer_context() -> Option<EditBufferContext<'_>>` is `None` exactly when the old `extract` returned `None`, tracker.rs:777),
so the extractor never sees "no open cycle". `EditBuffer` keeps private fields, `Clone + Debug + Default + PartialEq`, and its `at_prompt` ("cursor on
the command line") stays a different thing from `MarkerTracker::at_prompt` ("cycle open and not preexec"). Won over tracker-infra's `PromptContext` /
`PredictionStyle` / `pwsh_prediction: Option<(&Color, Style)>`: the domain names were prototyped and validated by a differential test; the
glossary gets `EditBufferContext`. `rgb_to_256` stays beside the colours as a pure move (R40).

### R9 Screen commands and cell tags (tracker-domain 12, tracker-infra a.6)
`CellTag { Prompt, Cycle(u64) }` is the single definition of the `tabsmith:p` / `tabsmith:c{n}` URIs (`CellTag::PROMPT_URI` const for the
extractor's per-row compare, `uri()` for the rest); it replaces `TAG_PROMPT` and the four `format!("tabsmith:c{n}")` (tracker.rs:682, 691, 692, 778).
`ScreenCommand { Tag(CellTag), ClearTag, MarkLineStart { redrawn_cycle: Option<u64> } }` (`Copy`). The adapter `cell_tagger::apply(model, command)`
maps them in the order received. Won over tracker-infra's `MarkLineStart { from }`.

### R10 The `TerminalScreen` port: one trait, eleven methods, `&self` with a `RefCell` inside, `Rc<dyn _>` (tracker-infra D1-D6, use-cases AP1)
Methods: `pass_through`, `pass_window_name_through`, `apply`, `cursor_column`, `edit_buffer`, `state`, `resize`, `draw_overlay`, `take_clear`,
`full_repaint`, `set_theme` (signatures in "Signatures"). Why `&self`: section 1 says use cases hold `Rc<dyn Port>`; `RecordingCompletionEngine`
and the new capturing doubles already use interior mutability; the runner's `guard` and `cleanup` need the very instance the use cases hold
(a panic inside a `borrow_mut` releases the `RefMut` while unwinding, a `RefCell` does not poison, so `guard`'s `take_clear` after a caught panic
works as today). Every method takes ONE borrow and calls no other port method (no re-entrancy, so no `BorrowMutError`). `pass_through` APPENDS to
an `out: &mut Vec<u8>` because `OutputFilter::hold_utf8` inspects the tail of the accumulated batch. `resize(columns, lines)` bundles
`term.resize`, `filter.resize` and `renderer.drop_clear` in HEAD order (pty.rs:1208-1211); `drop_clear` and a model-only resize are not port methods.
The startup CUP is the INHERENT `AlacrittyTerminalScreen::advance_model` (same long-lived `Processor`, called once on the concrete screen before
it becomes `Rc<dyn TerminalScreen>`). No `InMemoryTerminalScreen`: plan step 2 says a real object beats a fake, and the real adapter is the double
(`for_tests`).
Rejected: `&mut dyn TerminalScreen` as a parameter of every `execute` and service (the driver-cli default; doubles every signature and the guard
needs a second path to the screen: one-line switch if ever wanted); two traits (`MarkerScreen` + `TerminalScreen`, only useful for option A); a flag on
`pass_through` for window names (a caller could route a window name into the model).

### R11 `ScreenState` is a fresh `Copy` snapshot, created in 9.2d (tracker-infra a.4, session-domain D-3)
Eight fields (`alt_screen`, `origin_mode`, `insert_mode`, `pending_wrap`, `columns`, `lines`, `scroll_region`, `in_string`), built from
`ScreenStateParts`; `size() -> (columns, lines)`; `#[cfg(test)] plain(columns, lines)` (the `Default` has `columns` 0). `alt_screen` is the MODEL's
`TermMode::ALT_SCREEN`, never the filter's private `alt`; `scroll_region` and `in_string` are the FILTER's (they lag by its carry, as at HEAD).
Every entry point of a use case takes a fresh snapshot; none is cached across a `pass_through`, `apply` or `resize`.

### R12 `AlacrittyTerminalScreen`, scrollback, build order (tracker-infra D11, D12, D16; driver-cli D10)
`AlacrittyTerminalScreen::new(columns, lines, renderer, lf_to_ind)` uses `scrolling_history: 2000` (const); `#[cfg(test)] for_tests(columns, lines)` keeps
`Config::default()` (10000), `OverlayRenderer::new(Theme::dark(), false)` and `lf_to_ind` false, exactly the `setup()` and pty-test `session()` environment;
a new test pins 2000 in production. `headless_model.rs` (`type Model = Term<VoidListener>`) replaces `types::Model` for terminal_session at 9.2a.
The screen is built AFTER the probe, where the renderer is built today (pty.rs:458), then `advance_model(CUP)`; the `\x1b[H\x1b[2J` write stays before
`kitty_graphics::detect()` (which spawns `tmux display -p`). Only in-memory work (`Term::new`, `Tracker::new`, the CUP replay) moved after the probe:
unobservable (deviation 19). Rejected: a placeholder theme and `set_theme` later (P8-T25: construction and `set_theme` differ in the renderer's `sent`).

### R13 The adapters stay free functions over the model; `OutputFilter` moves verbatim (tracker-infra D13, D15)
`output_filter.rs` (`OutputFilter`, `lf_to_ind` stays a `pub(crate)` field because two locked tests write it mid-test), `edit_buffer_extractor.rs`
(`extract(model, &EditBufferContext) -> Option<EditBuffer>`; the old `Extracted` becomes its private `Scraped` so the body moves verbatim;
`strip_pwsh_prompts` stays here, its byte-versus-cell mix next to the code that produces the offsets), `cell_tagger.rs` (`apply(model, ScreenCommand)`).
Unit structs would add nothing (precedent `icon_assets.rs`, `kitty_graphics.rs`).

### R14 The batch loop is the application service `ShellOutputFeeder` (tracker-infra D9, use-cases a.4.5; tracker-domain 17)
`Tracker::feed` becomes `ShellOutputFeeder::feed(&self, splitter: &mut MarkerSplitter, tracker: &mut MarkerTracker, batch, out) -> Vec<ShellEvent>`; the
feeder holds `Rc<dyn TerminalScreen>` and `Arc<dyn Clock + Send + Sync>` and reads `clock.now_secs()` before each tracker call (R6). It is a service,
not a use case (so `ProcessShellOutputUseCase` can call it) and not part of the adapter (the adapter must not know the tracker). Its signature does not
mention `TerminalSession`, so the 26 marker tests run it without a session, settings, engine or writer. The aggregate owns the splitter and the tracker
and hands both out with `TerminalSession::output_parts(&mut self) -> (&mut MarkerSplitter, &mut MarkerTracker)` (the one Demeter concession, O-2 of
session-domain). The eight ordering rules are P9-T1..T8. Tests use the real screen, not a recording double (plan step 2; R10).
Rejected: the loop inside the adapter (pulls the splitter, clock and tracker into infrastructure); the loop as a method of `ProcessShellOutputUseCase`
(the 26 tests would need the whole use case); the splitter inside `MarkerTracker` (changes the validated tracker design); the feeder owning the
splitter in a `RefCell` (per-session state hidden in a shared service).

### R15 `FrameInput` becomes an application DTO and gains `now` (tracker-infra D10; Phase 8 O2/R7)
`application/frame_input.rs` (the port names it, application cannot import `overlay_renderer.rs`); fields `settings`, `ghost`, `spinner`, `suppress`,
`scroll_region`, plus `now: Instant`, which `menu_patches` passes to `menu.loading(input.now)` instead of reading `Instant::now()` (renderer.rs:307).
`OverlayPainter` reads `clock.now_instant()` LAST, when it builds the frame, never the tick's captured `now`. The difference to HEAD is the
microseconds between the build and the renderer's read. `scroll_region` stays a field (copied from `state().scroll_region()`), so `compose`'s guard,
the renderer tests and the byte-hash test keep their bodies. At 9.2c the renderer's test module (551 lines) is also sliced to the sibling
`overlay_renderer/tests.rs` (Phase 8 deviation 24; the module path `...::overlay_renderer::tests::<name>` is unchanged).
Rejected: dropping `scroll_region` and letting the screen fill it (a second input type for no behaviour); skipping `now` (kept as the documented fallback:
the commit is separate and nothing else depends on it).

### R16 The `TerminalSession` aggregate decides and never acts (session-domain D-1, D-2, D-3, D-14)
It reads no clock, file, port or screen: time comes in as `now: Instant`, screen facts as `&ScreenState`, the scraped line as `Option<EditBuffer>`; it
returns values (`Option<CompletionQuery>`, `bool`, `Option<Vec<u8>>`, `KeyRoute`, `Vec<MenuEffect>`) and the use cases perform the engine, shell and
terminal calls. It keeps the legacy mutation order (state mutates progressively and `on_keys` writes to the PTY only as its last statement, so a panic
mid-loop leaves the same partial state): no deferred `Vec<command>`. 46 `Session` fields become 22 aggregate fields plus six sub-entities
(`InsertionLock`, `EchoHold`, `GhostCandidates`, `ResizeDebounce`, `ShellExit`, `ReloadWatch`); `redraw_until`, `spinner`, `spinner_at`,
`late_probe_until`, `passthrough` stay plain fields (no invariant). The aggregate owns the `MarkerTracker`, the `MarkerSplitter` and the `KeyDecoder`
(R17). Between 9.3g and 9.3i the aggregate's fields are `pub(crate)` so the legacy `Session` methods keep compiling while their logic moves into
methods (Phase 8 R3 precedent: a verbatim move first, privacy in a later step); the end state has none (grep B8).
Rejected: ports or a clock inside the aggregate (rule 1 forbids `::application`); one `SessionTimers` for all instants; the flat `Session` kept in
infrastructure with helpers extracted (the 46-field class the plan exists to remove); building the whole domain dead and swapping (R4).

### R17 The key decoder is a domain entity owned by the aggregate (use-cases AP18, Phase 8 hand-off, over session-domain D-15)
`StdinKeyDecoder` is pure std over dropdown key value objects and `KeyEvent`, with one `Option<usize>` of state that the hold must reset (pty.rs:1086).
It moves by `git mv` to `domain/entities/key_decoder.rs` as `KeyDecoder` (`decode`, `reset`, `#[cfg(test)] decode` free fn kept, its five tests with it) at 9.3b;
`TerminalSession::decode_keys(&mut self, chunk) -> Vec<KeyEvent>` and `hold_keys` (which resets it). The Phase 8 hand-off said "TerminalSession owns the
decoder" either way; a port object cannot be held by the domain (rule 1). The glossary row changes from Adapter to Entity.
Rejected: a `KeyDecoder` port with `reset()` owned by the keystroke service (per-session state hidden in a shared `Rc` service with a `RefCell`).

### R18 `KeyRouter::route` (session-domain D-7)
One pure `key_router::route(key: &Key, keymap: &Keymap, facts: &KeyFacts) -> KeyRoute`; `KeyRoute { Hold, MenuAction(KeyAction), Swallow, ShowOnTab,
AcceptGhost(String), Forward }`; `KeyFacts` (eight facts, built from `KeyFactsParts`). The match has no `_` arm over `Option<KeyBinding>` and keeps P8-T10's
guard order (`Action if intercepting`, `Unknown if intercepting`, `tab_show`, `Show/Toggle if global`, then ghost or forward); `Ignore` has no arm of its
own. `acts` counts a ghost key even when an Action arm would win, and does not count the global show/toggle arm. `ghost_key`, `is_ctrl` and the new
`plain_char`, `ends_line`, `is_enter` helpers live here (`ghost_key` takes dropdown key types, so it cannot go to ghost_text). The "lookup only when
`prompt`" rule has one owner because `route` takes the `Keymap`.

### R19 Time (session-domain D-8, use-cases AP10)
Every method that reads the clock takes `now`; use cases and services hold `Arc<dyn Clock + Send + Sync>` and read `now_instant()` at the HEAD site of each
`Instant::now()` (table in "Signatures"). Reads that were microseconds apart inside one function collapse to one (`refresh`: pty.rs:830, 845, 858;
`apply_completion`: 897, 899); `tick_spinner(now, judged_at)` keeps two instants (`now` at :1253, a fresh read at :1254); `HandleTimerTick` captures `now` once.
The harness uses `SystemClock` for all 24 moved tests: three of them (`split_prompt_redraw_keeps_the_menu`, `keys_after_an_unmodelled_key_wait_for_its_echo`,
`slow_echo_keeps_the_insertion_in_the_basis`) set a field to `Instant::now()` and then tick, and a `FixedClock` is built EARLIER than that instant, so
`d <= now` would be false. New boundary tests use `FixedClock` (`advance`). Rejected: a `&dyn Fn() -> Instant` into the aggregate to reproduce every read.

### R20 Application services and thin use cases (session-domain D-11, use-cases AP2, AP3)
Logic shared by several events lives in flat services held by `Rc`: `KeystrokeHandler` (`on_keys` and `release_held`, called from four events),
`CompletionRequester` (`refresh`, four callers), `OverlayPainter` (`frame`, `redraw`, `take_repaint`), `SessionFilePoller` (`poll_files`, two callers),
`ShellOutputFeeder` (R14). None is a `*_use_case.rs` or a `*UseCase`, so "a use case never calls another use case" holds (the skill's naming table lists
`Service`; rule 3 is cross-module only). `HandleKeystrokesUseCase` is a thin wrapper over `KeystrokeHandler` because `release_held` replays held bytes
from `ProcessShellOutput`, `ApplyCompletionResult` and `HandleTimerTick`. Rule 3 cannot see these services from another module (the Phase 4-6 ceiling), so
grep F1 pins that nothing outside `terminal_session` names them. Rejected: one use case calling another; port calls inside the aggregate; the driver replaying
`release_held` (it runs in the MIDDLE of the tick, pty.rs:1237); an `application/services/` folder (flat files are the precedent, `generator_runner.rs`).

### R21 Use cases write their own output; granularity per HEAD emit site (use-cases AP4, AP5; governance G4)
`ProcessShellOutputUseCase` does NOT return one buffer for the driver to write (plan section 4/7): `on_output` emits once (pty.rs:777) and THEN `release_held`
(:778) may emit a bell, a frame and write to the PTY. Folding those into one returned buffer would reorder stdout against the PTY write. The "one write per
batch" rule stays as ONE `writer.write(&out)` inside the use case. Write granularity is kept per site: batch 1 (:777), each `redraw` 1 (:1016-1019), tick
repaint+frame 1 (:1265), each bell 1 (:1139), `EndSession` 1 (:1309), the guard's clear and the passthrough raw batch (driver). A coalescing writer would change
terminal timing. `IoJournal` (test support) records the cross-port order of stdout and shell input, which the old code could not expose.

### R22 New ports: `TerminalWriter`, `WrappedShell`, `GeneratorProcessReaper`, `TerminalModes`, `LiveSettings`, `WorkingDirectory` (use-cases, driver-cli, session-domain)
All `&self`, dyn-compatible, never failing, each with a double in its file under `#[cfg(test)]`. `WrappedShell` = `write_input(Vec<u8>)` + `resize(columns,
rows)` (plan: "write input, resize"); the ConPTY master release stays an INHERENT method of `PtyWrappedShell` that only the runner calls (R23).
`TerminalModes { restore }` keeps the exact cleanup order (clear, shutdown, reap, restore modes, remove the context) and carries the fallback's restore: the
use-cases note accepted an unobservable reorder (restore after `EndSession`); the driver-cli note kept the order with a port. Taken: the port, because the
constraint list names write ORDER and the port costs one trait and one adapter (deviation 13). `LiveSettings { last_modified, load }` exists because the
lenient load must log `settings: <chain>` at warn in an adapter (D5) and fitness rule 3 forbids `load_or_defaults` and `JsonFileSettingsRepository`;
`LenientLiveSettings` logs inside the call, with the same text, level and condition. `WorkingDirectory::current() -> Option<PathBuf>` replaces the
`std::env::current_dir()` fallback read at EVERY request and LLM call (capturing once at start would change behaviour when the directory is deleted).
Rejected: returning warnings from use cases (two use cases would carry them, a panic after the load would lose them); a logger port; `application` calling
`SettingsRepository::load` and the logger; a third `WrappedShell` method for the master drop.

### R23 The ConPTY master release stays in the runner (use-cases AP6, driver-cli D7)
`master.take_if(|_| cfg!(windows))` plus the helper thread (pty.rs:669-671) is a platform fact next to the cfg(windows) test: `PtyWrappedShell::release_pseudo_console(&self)`,
called by the runner in the `ChildExit` arm BEFORE `HandleShellExitUseCase` (HEAD: after `exit_code` is set and before the `eof` check; the three effects are independent).
`conpty_is_closed_off_the_main_loop` keeps its fake `MasterPty` and builds a `PtyWrappedShell` over it.

### R24 `HandleShellExitUseCase` takes `Term` and returns the loop bool (use-cases AP14; plan routes Term to EndSession)
`execute(&mut TerminalSession, ShellEnd) -> bool` with `ShellEnd { ChildExited(i32), OutputClosed, Terminated }` returns exactly the bools of pty.rs:650, 672-676, 679
(the entity `ShellExit` of session-domain holds the state; the DTO is named `ShellEnd` to avoid the clash). `EndSessionUseCase` runs once, after the loop, for
every exit, so `Term` only records 129 and stops the loop.

### R25 Late probe replies: stripped in the runner's guard; `LATE_PROBE` lives in the aggregate (use-cases AP12, driver-cli D4, session-domain D-12)
`SessionRunner::on_input` (inside the same `guard` closure as the keys, exactly where `Session::on_input` is) calls `parse_probe` (infrastructure, the plan's
`terminal_probe.rs`), `ApplyTerminalBackgroundUseCase` for a colour, `session.probe_cursor_reported()` on a CPR, then `HandleKeystrokesUseCase`. The plan's table
("stripped by the driver") is read as "by the driver's guarded handler" (a panic in the strip must reach passthrough, not the outer fallback tier).
`TerminalSession::expect_late_probe_reply(probe_ended_at: Instant)` holds the 3 s window (`LATE_PROBE`, a domain const: deviation from the source map);
`late_probe_open(now)` is the `Instant::now() < t` check; an EXPIRED window stays `Some` until a CPR clears it. Rejected: moving `parse_probe` into the domain
(session-domain O-3: the plan places it in infrastructure and the strip does not need it there); a `ProbeReplyParser` port; stripping outside the guard.

### R26 Launch: `PrepareSessionLaunchUseCase` is pure and meaningful; `WrappedShellCommand` models the wrapped launch only (use-cases AP16, session-domain D-13)
`execute(LaunchRequest { shell, command, environment: LaunchEnvironment { tabsmith_shell, system_shell, login_shell } }) -> WrappedShellCommand` owns the shell
resolution order (explicit, then `TABSMITH_SHELL`, then `"pwsh"` on Windows, else `SHELL`; `cfg!(windows)` inside, precedent Phase 7's `cd_target`), the driver
reads the three env facts (`env_nonempty` for `TABSMITH_SHELL` and `SHELL`, where empty counts as unset; `TABSMITH_IS_LOGIN_SHELL == "1"` for the flag, pty.rs:80-89; reading `SHELL` on Windows or `TABSMITH_SHELL` when a shell was given is side-effect
free) and calls it BEFORE `catch_unwind` (pty.rs:90). `WrappedShellCommand::new(shell, extra, login)` is `argv` verbatim; `in_session(version, in_tmux, &SessionId)`
adds the five env changes of pty.rs:479-486; `window_title()` is pty.rs:495-496. The fallback's env (`TABSMITH_TERM_DISABLED=1` plus the same two removals,
:145-147) is a different mechanism (`std::process::Command` + `exec`) and a different set: it stays in `fallback_shell.rs` and reads `command.argv()`. The cwd
choice (:472-476) is I/O and stays in the launcher. Won over driver-cli D14 (shell resolution as a private `choose_shell` in the driver): the rule is business
logic and its four tests become use-case tests.

### R27 `SessionId` (session-domain D-9, tracker-domain 14)
`SessionId::from_entropy(nanos: u64, pid: u32) -> SessionId` is a pure constructor (`{nanos:016x}{pid ^ nanos.rotate_left(17):016x}`, pinned by the literal
`0123456789abcdef8acf13579bde026c` for `(0x0123456789abcdef, 0x2a)`, verified in Python); it lands with `MarkerTracker` at 9.1d but has NO caller until 9.6e (`session_id()` keeps generating the string, pinned by a 9.0d guard), so the interim composite (9.1e) and `Session::start` (9.3g) wrap the generated string with `SessionId::new(sid.clone()).expect("generated ids are valid")` (`SessionId::new` takes an owned `String`); that `expect` dies with `generate_session_id()` at 9.6e.
The launcher keeps the `SystemTime` and `process::id()` reads (`generate_session_id() -> SessionId`). The `Clock` port does NOT grow `now_nanos`: one call site, no
behaviour to fake, and a millisecond clock would change the 32-digit layout the shell scripts echo in `NewCmd=`. The aggregate's id is a required `SessionId`
(HEAD's `Option` at pty.rs:526 existed only because `SessionId::new` is fallible); the 28 tests that ran with `None` use `SID` against an in-memory repository that
holds nothing (`last_modified` is `None`, the same skip).

### R28 Module wiring: collaborators injected, `SessionUseCases::new` in application (driver-cli D9, use-cases AP17)
`build_terminal_session_module(&SharedDependencies, settings, themes, generator_processes, start_completion_engine)` (four positional collaborators, five parameters, as `build_completion_module` does; no struct for them); the module
exposes getters (`live_settings()`, `themes()`, `generator_processes()`, `session_contexts()`, `start_completion_engine()`, `clock()`, `locate_data_root()`) and holds
`report_session_context` and `prepare_session_launch`. One bootstrap helper `terminal_session_module()` serves the pty, pre-cmd and debug-shell arms (pre-cmd runs at
every prompt: three `Rc`s over lazy repositories, no I/O, no thread; `precmd-diff.sh` guards it). `themes` is `DropdownModule::themes` (the Phase 8 hand-off).
`launch_session` fills `SessionPortsParts` from the module and the per-session adapters and calls `SessionUseCases::new(parts)`, the same wiring the harness runs with
doubles (a wiring mistake shows in a use-case test). Rejected: a late factory `module.session_use_cases(ports)` (a second wiring point the harness would not share); a
second builder only for the pty; a global of collaborators.

### R29 Driver moves that ride along (driver-cli D2, D3, D12)
`types::Msg` becomes `infrastructure/pty/session_message.rs::SessionMessage` at 9.6a, deleted from `types.rs` at once (shims never cross steps); the variant names and the
POSITIONAL `Resize(cols, rows)` stay (a rename of variants would turn ~10 locked test bodies CHANGED; the `PtySize { rows, cols }` swap risk), so locked tests change only
by path except `pty_reader_waits_for_credits` (a bare type). The engine replies keep ONE concrete `Sender<SessionMessage>` inbox (`CompletionEngineStarter = fn(Sender<SessionMessage>,
Settings) -> Rc<dyn CompletionEngine>`): the single channel is what keeps abort-on-newer-request and the stash batching (plan section 7); bootstrap importing `session_message` is legal. Platform code is
selected per consumer with cfg'd use-aliases (`launcher`, `console_terminal_modes`), no re-export facade; `terminal_probe` takes `read_timeout` as a closure.

### R30 The runner (driver-cli D1, D4, D11, D13)
`SessionRunner` is a struct of trait objects (no generics, plan section 1), built from `SessionRunnerParts`; field declaration order keeps the HEAD drop order
(inbox, credits, the shared PTY object, then the rest). `run`, `step`, `guard`, `on_input`, `next_deadline` keep their HEAD signatures (`step` returns the loop bool). Three files:
`session_runner.rs` (loop, step, guard), `session_launcher.rs` (`launch_session`, HEAD 401-576 line for line, R12's order), `pty_session.rs` (`run_pty_session`, `env_nonempty`, the panic seam
`attempt(run) -> anyhow::Result<i32>`, tested by three new tests: it makes the OUTER panic tier testable, which no test covers today). The panic hook is set first in `run_pty_session` and
stays installed after it returns (debug shell's code runs after). Rejected: one 450-line runner file; a `Session` trait.

### R31 Test support: one harness in `application/test_fixtures.rs` (use-cases AP17/AP19, governance G10; over driver-cli P29)
`TerminalSessionHarness` wires the REAL `AlacrittyTerminalScreen::for_tests(80, 24)`, `CapturingTerminalWriter`, `CapturingWrappedShell` (both journaled), a
`RecordingCompletionEngine` whose receiver the harness keeps, `InMemoryLiveSettings`, `RecordingGeneratorProcessReaper`, `RecordingTerminalModes`, `SystemClock`, through
`SessionUseCases::new` (production wiring). The file is declared `#[cfg(test)] pub(crate) mod test_fixtures;` and EVERY `use` of an infrastructure path in it carries its own
`#[cfg(test)]` (the checker reads a test-support file as production code unless the item is test-only: governance probes V1-V4; precedent `completion/domain/test_fixtures.rs`). The
runner's tests (`session_runner/tests.rs`, exempt wholesale) build the runner from the harness parts. Won over driver-cli's `infrastructure/testing/session_fixture.rs`. The
`#[cfg(test)] maybe_panic()` hook stays the FIRST statement of `KeystrokeHandler::handle`'s per-key loop as a `#[cfg(test)]` statement calling `test_fixtures::maybe_panic()` (the plan's
"stays at the driver's call site" would fire when keys are held and for chunks that decode to nothing); `set_panic_in_key_handling(bool)` serves the two driver tests.

### R32 Test moves: names kept, move when the last needed use case exists, collisions renamed (all areas)
Every existing `#[test]` keeps its fn name; D6 splits keep the original name on one half (`suggestion_colors`, `pwsh_reported_prediction_colour`); `child_argv` is restructured. The 28
`Session` tests move to the use case that is their ACT once every use case their ARRANGEMENT needs exists: 5 context tests at 9.5b, 16 at 9.5e, 3 at 9.5g, 4 driver tests at 9.6f (table in
"Test moves"). The 26 marker tests move to `alacritty_terminal_screen/tests.rs` at 9.2f (one `Rig`; the six fixture-only ones are CHANGED by its constructor). Colliding NEW names found across the area notes are fixed here (the DUPLICATE guard): the
use-case versions of `should_exit_with_129_when_terminated`, `should_leave_at_once_when_the_output_had_already_ended`, `should_go_on_after_the_output_ended_until_the_child_exits`,
`should_ignore_an_llm_reply_for_another_request` and the driver's `should_give_the_settings_the_repository_loads` (it already exists in settings) get the names listed in "New tests";
`grep -rn "fn <name>(" src` is empty before any new name is added (K8).

### R33 The CLI commands (driver-cli D15; governance T-G3)
`DebugCmd` moves to `bootstrap/cli.rs` verbatim (variants `KeyTester`, `Shell { shell: Option<ShellArg> }`, `Logs`, same attributes, docs and order: D4); `InternalCmd` stays there.
`run_key_tester_command` and `key_line` move verbatim. `run_debug_shell_command(shell, module, render_post_script)` takes the post-script renderer as a callback built in
bootstrap from `RenderInitScriptUseCase::new(Rc::new(OsShellEnvironment), Rc::new(EmbeddedShellScripts))` and `init_script(.., IntegrationPhase::Post)` exactly as HEAD, wrapped in ONE bootstrap helper `pub(crate) fn debug_shell_post_script(shell: Shell) -> anyhow::Result<String>` in `bootstrap/cli.rs` (the `DebugCmd::Shell` arm passes it as the callback and so does the moved test's child branch, so terminal_session's test code names no shell_integration type and does not copy bootstrap's wiring; terminal_session may not call
shell_integration: Phase 4 hand-off, precedent `print_root_help`), the data root comes from `module.locate_data_root()` and is read AFTER the render (cli.rs:245-250), the two `std::env::set_var`
calls stay in the handler before `run_pty_session` spawns anything (building the module starts no thread), output stays `let _ = writeln!` (never `println!`). The D9 string of
`debug_shell_survives_closed_terminal` becomes `terminal_session::infrastructure::cli::debug_shell_command::tests::debug_shell_survives_closed_terminal` (a stale string makes the child run 0 tests
and the marker assertion fail after ~20 s). `debug_logs` stays in `cli.rs` and becomes `pub fn` for the bootstrap dispatch until Phase 10 moves it. `debug_shell_files` keeps its own 0700
`DirBuilder` and plain `fs::write`: NOT routed through `AtomicFile` (mode and purge guard differ).

### R34 Characterisation guards are text fixtures written once (governance G3, G5, G6)
Each scenario appends, per step, the stdout delta, the PTY bytes (in order) and the engine calls (no cwd, no time) to a log and compares it with
`tests/fixtures/terminal_session/characterisation/<name>.log` through `assert_matches_fixture`; a missing fixture is created only with `P9_WRITE_FIXTURES=1` (and the test then fails
"fixture created, review it"); an existing one is never rewritten. All field pokes of the scenarios (forcing a deadline due) go through helper functions defined once in the test module
(`poke_lock_due`, `poke_resize_due`, `poke_llm_due`, `poke_poll_due`, `poke_hold_due`, `poke_spinner_due`, `poke_probe_window`), so each later sub-entity step edits one helper, not the scenario
bodies. Time is forced, never waited, except the spinner scenario's single 250 ms floor sleep (a floor cannot flake). The within-event interleaving of a stdout write and a PTY write is not
observable in the old code and production is not touched to make it so: the new `IoJournal` tests pin it from the old call-order table (P9-T99). Governance's scenario 15 (`run`) is dropped:
the driver tests 4-8 of 9.0d cover the loop-exit semantics; scenarios 12 (`passthrough`) and 14 (`batching`) stay for the exact bytes.

### R35 Scripts (governance G7, G12; driver-cli S0a)
Four committed scripts in `scripts/hexagonal-refactor/`: `fitness-mod.sh [--worktree] [module...]` (architecture test on a copy with modules added; for experiments),
`phase9-greps.sh [base-rev]` (the acceptance greps, exit 1 on a FAIL), `session-diff.py <abs binary>` (tmux and bash screen snapshots of the menu, ghost, bottom row, resize, vim, `exit 3`,
reload, child env, key tester and every non-terminal failure path of `_ pty`, `debug shell`, `debug key-tester`; ~85 s; deterministic in four runs; catches an env and a resize mutation) and
`pty-diff.py <abs binary>` (a raw-pty differential harness: probe answered or not, typeahead during the probe, child env and cwd, deleted cwd, SIGTERM to 129, `exit 3`, no shell, not a tty,
corrupt settings warn line, `-vv` info line; ~12 s). They keep DIFFERENT names (12 s raw bytes versus 85 s screens). None joins `gate.sh` (it would double the gate): they run at the
steps listed in "Steps", at every run end and at the close.

### R36 Gate numbers and windows (governance G9)
Linux runs `BASELINE_CLIPPY=4 scripts/hexagonal-refactor/gate.sh` for the whole phase (the container's clippy 1.97 reports one more lint than macOS; one of the four, `pty.rs:1801`
`Modes::default()`, dies with the `modes` field at 9.4c; the budget stays 4, the report says 3). The gate's `cargo check --locked --all-targets --target x86_64-pc-windows-gnu` (38 s warm) is
mandatory for any step that touches `fallback`, `run`, `step`'s `ChildExit`, `sys`, the tracker's `defer`, `argv`, `Shell` defaults or any cfg line; `cargo check --locked --no-default-features
--all-targets` and `cargo test --locked --release -- --ignored all_bundle_modules` (`1 passed`) run at every run end. New code adds no clippy warning (macOS budget 3, all used).

### R37 cfg handling
The cfg lines that exist at HEAD (pty.rs `cfg!(windows)` at 83, 139, 424, 669; `cfg(unix)`/`cfg(windows)` at 148, 150, 1319, 1438, 1752, 2506; `cfg!(test)` at 714; tracker.rs 555; cli.rs 235, 278, 380 and 422; the `cfg!` lines 66 and 100 of cli.rs belong to `diagnostics` and stay)
move verbatim; the only line that disappears is `if cfg!(test) {` of `emit` (replaced by the writer port). Platform files are `#[cfg(unix)] pub(crate) mod platform_unix;` and
`#[cfg(windows)] pub(crate) mod platform_windows;` in `infrastructure/pty/mod.rs` (rule 6 allows attributes and child declarations). Grep C1 compares the sorted cfg multiset against the base revision.

### R38 Legacy code leaves with its last item (Phase 8 R22)
| Legacy code | Leaves in | How |
|---|---|---|
| `Splitter`/`Piece`, colours, `Extracted`, `Event`, state fields, the four dead fields | 9.1a, 9.1b, 9.1e | moved or deleted with their last user; the legacy `Tracker` composite stays |
| `OutFilter`, `tag`, `mark_line_start`, `extract`, `strip_pwsh_prompts`, `suggestion` | 9.2a, 9.2b | moved to adapters |
| `Tracker` composite, `src/tracker.rs`, `mod tracker;`, `types::Model` | 9.2f | the feeder replaces `feed`; the 26 tests move |
| `types::Msg` | 9.6a | renamed `SessionMessage` |
| consts, `argv`, `fallback`, `pump`, probe fns, `sys` modules, `session_id` | 9.3a, 9.3b, 9.4a, 9.6a, 9.6b | moved |
| `emit`, `write_stdout`, `sink`, `writer`, `master`, `modes`, `settings_repository` | 9.4b, 9.4c | ports adopted in `pty.rs` |
| `Session::on_*` bodies | 9.5a-9.5h | delegates, then gone |
| `src/pty.rs`, `mod pty;` | 9.6d | `git mv` to `infrastructure/pty/session_runner.rs` |
| `cli.rs` `DebugCmd`, `debug`, `key_line`, `key_tester`, `debug_shell`, `debug_shell_files`, 3 tests | 9.6c | moved; `cli.rs` keeps `diagnostic*`, `debug_logs` and `diagnostic_toml` for Phase 10 |
No `pub(crate) use` anywhere (grep B6). `LEGACY_MODULES` keeps `pty` and `tracker` (the names stay after the files die; Phase 11 sweep).

### R39 Documentation only at the close (Phase 8 R27)
The plan's status, sections 2, 4, 5, 7, 8, 9, the glossary, `README.md`, `handoff.md` and this file's Status line are 9.7's work. Exceptions, in the step that deletes `pty.rs` (9.6f): the four
`settings_catalog.rs` comments naming `pty.rs` (lines 345, 355, 375, 441) and the `architecture_tests.rs:376` comment example. `docs/SPEC.md` §4.1, §4.3, §5.1 and §10 stay Phase 11.

### R40 `rgb_to_256` stays where it is (tracker-domain 11)
It equals `dropdown`'s `color_parser::xterm256((r, g, b))` for all 2^24 inputs (standalone program, 0 differences), so D1's proof is available, but the dedupe edits two locked tests'
call sites (`suggestion_colors`, `pwsh_reported_prediction_colour`) and the plan does not ask for it: pure move now, dedupe is a Phase 11 item (O4).

### R41 Behaviour noticed and NOT fixed (agent brief: report)
See P9-N1..N6: a Windows `PreExec` ignored when its `NewCmd` is still queued; a bare `Dir` giving an empty cwd that `cwd()` does not fall back from; the end-of-batch flush starving while every
batch carries a marker; a panic in `release_held` after the emit writing the batch twice; `settings_mtime` read at the end of `start`; `try_clone_reader`/`take_writer` failing after the child exists.

### R42 Names and files beyond plan section 4 are accepted (all areas)
Domain: `cell_tag`, `text_emphasis`, `pwsh_prediction_color`, `edit_buffer_context`, `marker_effects`, `marker_reaction`, `key_facts`, `completion_query`, `entities/{ghost_candidates, resize_debounce,
shell_exit, reload_watch, key_decoder}`. Application: `frame_input`, `launch_request`, `launch_environment`, `shell_end`, `session_ports_parts`, `session_use_cases`, `test_fixtures`, the five services,
`apply_terminal_background_use_case`, ports `generator_process_reaper` (plan), `live_settings`, `working_directory`, `terminal_modes`. Infrastructure: `headless_model`, `lenient_live_settings`,
`process_working_directory`, `console_terminal_modes`, `pty/{completion_engine_starter, session_launcher, pty_session}`. Bootstrap: `GeneratorProcessBridge`, `terminal_session_module()`. Glossary rows and the
plan's section 4 layout are written at 9.7.

### R43 The manual smoke is asked once, after 9.7; nothing is pushed (governance 8, 11)
Plan section 8's smoke is due after Phase 9 (items 1-9 on zsh, bash and fish, plus Phase 8's P8-S1/P8-S2 still pending, plus P9-S1..S5 below). Agents cannot drive a terminal. The request names no push or PR:
none is made.

### R44 Commits
Conventional Commits, subject at most 72 characters (measured before committing), `refactor(terminal_session): <why>`, `test(terminal_session): ...`, `chore(scripts): ...`, `docs: ...`; the body ends with
the two trailer lines `Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>` and `Claude-Session: https://claude.ai/code/session_01D36FSrfVgyVzjL9KvXcgTZ` (agent-brief.md's Opus line and repo path are superseded; the
repo is `/home/user/tabsmith`). Always `git commit -F <file>` (the RTK hook rewrites words inside a quoted `-m`). Never rewrite history. Test-first seams: RED, GREEN, commit, REFACTOR, commit.

### R45 Per-step rules (the checklist every step brief repeats)
1. `BASELINE_CLIPPY=4 scripts/hexagonal-refactor/gate.sh` prints `GATE PASS` before each commit (it includes the architecture test with `terminal_session` registered, the windows check and the golden capture); a lone
   `integrations_zsh_powerlevel10k_prompt_wrapped` or `process_executor::tests::exec_status_output_and_timeout` failure is rerun alone with `--exact`, never skipped. From 9.0c until 9.4a injects `RecordingGeneratorProcessReaper` into the `session()` fixture, the guard tests that reach `Session::cleanup` or `Session::run` (the 9.0c scenario `cleanup`, and the 9.0d loop test that must call `run`) call the PROCESS-GLOBAL `kill_live_children()`, which SIGKILLs every exec child registered in the process-wide `LIVE` list (`specs/.../process_executor.rs`): in a parallel `cargo test` they can kill the live child of another test. So a lone failure of ANY test that spawns an exec child (every `specs::infrastructure::adapters::process_executor::tests::*`, `cmd_gets_no_cwd_search_env`, the quickjs and specs tests that run a process), not only `exec_status_output_and_timeout`, is rerun alone with `--exact` and then the gate; it is not read as a regression (the 20 SERIAL runs cannot show this hazard).
2. `python3 -B scripts/hexagonal-refactor/testlock.py . ef807c1 --diff`, `. phase9-start --diff` and (from 9.0d) `. phase9-guards --diff`: MISSING 0, DUPLICATE 0, every CHANGED in the step's list.
3. From 9.0d: `git diff phase9-guards -- tests/fixtures/terminal_session/characterisation` is empty.
4. Moves by script or `git mv`, then a block diff against `git show phase9-start:<file>` shows only paths, visibility, glossary names, accessors and the listed edits.
5. `grep -rn "fn <name>(" src` is empty before each new test name; new tests are `should_` names, AAA, no mocks.
6. Hand greps of "Acceptance greps" that apply (B1, B3, B6, B13 and the cfg multiset C1 at every step).
7. Steps touching `Session::start`, the probe, the reader, the screen order or the module wiring also run `cargo test --locked debug_shell_survives_closed_terminal` alone, `session-diff.py` and
   `pty-diff.py` (old versus new binary: both empty) and the four older diff scripts when `cli.rs`, `bootstrap/cli.rs` or the module changed.
8. Steps touching timers, locks, held keys, the loop or the driver run the pty tests and the characterisation tests 20 times serially.
9. Prune imports a move leaves unused in the same commit (clippy budget); no `new` over 7 parameters (use `*Parts`); no `Default` unit struct; no `match` with duplicated bodies.
10. After a step: `git status` clean; `rm -rf scripts/hexagonal-refactor/__pycache__` if it appears.

### R46 No new crates; nothing else moves; open questions use defaults
`Cargo.toml` and `Cargo.lock` do not change (the gate builds with `--locked`; AGENTS.md: exact versions only, no auto-upgrade). The only external crates the new code names are the ones the old files already used (`alacritty_terminal`, `portable_pty`, `crossterm`, `libc`, `signal_hook`, `windows_sys`, `anyhow`, `clap` in bootstrap only, `unicode_width` in the domain),
each in the layers greps B9 and B10 allow. Nothing outside `src/terminal_session/` and the table "Outside `src/terminal_session/`" of "Module map" changes: `doctor.rs`, `completion`, `ghost_text`, `dropdown`, `shell_integration`, `history` and `specs` are untouched, `settings` changes only the four comments of 9.6f, `shared` only gains the test helper of 9.0b;
`docs/SPEC.md` is Phase 11's. An open question nobody answers is decided by its default.

## Module map

Layers: **D** domain, **A** application, **I** infrastructure. `pub(crate)` at most; domain fields private (the one interim exception is R16). Every `mod.rs` only declares children (rule 6;
`#[cfg(unix)]`/`#[cfg(windows)]` attributes on a declaration are allowed). `src/terminal_session/` at the end of the phase (a file marked `(exists)` is already in the tree at HEAD):

| File (under `src/terminal_session/`) | Content | From (HEAD) | Layer | Step |
|---|---|---|---|---|
| `domain/value_objects/output_piece.rs` | `OutputPiece { Bytes, Marker(String), WindowName }` | `tracker.rs:24-31` | D | 9.1a |
| `domain/value_objects/text_emphasis.rs` | `TextEmphasis { dim, italic }` | new (replaces `Flags`) | D | 9.1a |
| `domain/value_objects/shell_suggestion_color.rs` | `ShellSuggestionColor`, `parse_zsh/fish`, private `parse_color`, `rgb_to_256` | `tracker.rs:167-275` | D | 9.1a |
| `domain/value_objects/pwsh_prediction_color.rs` | `PwshPredictionColor::parse` | `tracker.rs:950-989` (`parse_sgr`) | D | 9.1a |
| `domain/value_objects/edit_buffer.rs` | `EditBuffer` | `Extracted`, `tracker.rs:494-503` | D | 9.1b |
| `domain/value_objects/cell_tag.rs` | `CellTag { Prompt, Cycle(u64) }`, `PROMPT_URI`, `uri()` | `TAG_PROMPT` `:18` + four `format!` | D | 9.1b |
| `domain/value_objects/screen_command.rs` | `ScreenCommand` | the `tag`/`mark_line_start` calls | D | 9.1b |
| `domain/value_objects/marker_effects.rs`, `marker_reaction.rs` | the return values of the tracker | new | D | 9.1b |
| `domain/value_objects/shell_event.rs` | `ShellEvent { NewCommand, CommandEnded, PreExec }` | `Event`, `tracker.rs:483` | D | 9.1b (used 9.1e) |
| `domain/value_objects/shell_marker.rs` | `ShellMarker::parse`, `passes_integration_lock` | `Tracker::on_marker` grammar `:620-678` | D | 9.1c |
| `domain/value_objects/edit_buffer_context.rs` | `EditBufferContext<'a>`, `EditBufferContextParts<'a>` | the tracker fields `extract` reads | D | 9.1d |
| `domain/value_objects/deferred_marker.rs` | `DeferredMarker { EndPrompt, NewCmd }` (`pub(crate)`: the item type of `MarkerTracker::deferred()`, named by the moved `windows_deferral_fifo`) | `Deferred`, `tracker.rs` | D | 9.1d |
| `domain/value_objects/session_id.rs` (exists) | + `from_entropy(nanos, pid)` | `pty.rs:312-322` | D | 9.1d |
| `domain/value_objects/screen_state.rs` | `ScreenState`, `ScreenStateParts` | the reads of `alt`/`drawable`/`suppressed` | D | 9.2d |
| `domain/value_objects/wrapped_shell_command.rs` | `WrappedShellCommand` | `pty.rs:112-130, 479-486, 495-496` | D | 9.3b |
| `domain/value_objects/key_route.rs`, `key_facts.rs` | `KeyRoute`, `KeyFacts`, `KeyFactsParts` | branches of `on_keys` `:1056-1127` | D | 9.3f |
| `domain/value_objects/terminal_background.rs` | `TerminalBackground(Rgb)` | `pty.rs:251, 357` | D | 9.3g |
| `domain/value_objects/completion_query.rs` | `CompletionQuery` | the `CompletionRequest` literal `:865-872` | D | 9.3h |
| `domain/value_objects/key_event.rs` (exists) | `KeyEvent` | Phase 8 | D | |
| `domain/entities/marker_splitter.rs` | `MarkerSplitter` (+ the 7 tests) | `tracker.rs:14-161` | D | 9.1a |
| `domain/entities/marker_tracker.rs` | `MarkerTracker` | `Tracker` state and marker methods `:506-760` | D | 9.1d |
| `domain/entities/key_decoder.rs` | `KeyDecoder` (+ 5 tests) | `adapters/stdin_key_decoder.rs` | D | 9.3b |
| `domain/entities/insertion_lock.rs`, `echo_hold.rs` | `InsertionLock`, `EchoHold` | `lock` tuple, `unechoed`, `echo_pending`, `held`, `held_at` | D | 9.3c |
| `domain/value_objects/lock_settlement.rs` | `LockSettlement { StillHeld, Released }` (`pub(crate)`: returned by `InsertionLock::settle`, matched by the aggregate) | the outcome of the lock branch of `refresh` | D | 9.3c |
| `domain/entities/ghost_candidates.rs` | `GhostCandidates` | `ghost`, `llm`, `llm_req`, `llm_at` | D | 9.3d |
| `domain/entities/resize_debounce.rs`, `shell_exit.rs`, `reload_watch.rs` | the three small entities | `resize_at/repaint`, `exit_code/eof/exit_at`, `poll_at/settings_mtime/session_mtime` | D | 9.3e |
| `domain/entities/terminal_session.rs` (+ `domain/entities/terminal_session/tests.rs` if the tests pass ~500 lines) | `TerminalSession`, `TerminalSessionParts`, `LATE_PROBE` | `Session` state `:333-397` | D | 9.3g-9.3i |
| `domain/entities/session_context.rs` (exists) | | Phase 4 | D | |
| `domain/services/insertion_reconciler.rs` | `insertion_bytes`, `expected_buffer`, `unechoed_after` | `pty.rs:161-218` | D | 9.3a |
| `domain/services/key_router.rs` | `ghost_key`, `is_ctrl`, `plain_char`, `ends_line`, `is_enter`, `route` | `pty.rs:197, 247`; `on_keys` | D | 9.3a, 9.3f |
| `domain/services/session_deadlines.rs` | the five durations, `next_deadline` | `pty.rs:55-71, 721-739` | D | 9.3a |
| `domain/services/render_guard.rs` | `allows_drawing(&ScreenState)` | `drawable` `:989-1000` | D | 9.3a |
| `domain/repositories/session_context_repository.rs`, `domain/terminal_session_error.rs` (exist) | | Phase 4 | D | |
| `application/ports/terminal_screen.rs` | `TerminalScreen` | `term`, `renderer`, `tracker.filter` | A | 9.2d |
| `application/frame_input.rs` | `FrameInput` | `overlay_renderer.rs:28-39` | A | 9.2c |
| `application/shell_output_feeder.rs` | `ShellOutputFeeder` | `Tracker::feed` `:589-605` | A | 9.2f |
| `application/ports/generator_process_reaper.rs` | port + `RecordingGeneratorProcessReaper` | `kill_live_children` `:136, 1311` | A | 9.4a |
| `application/ports/terminal_writer.rs`, `wrapped_shell.rs` | ports + capturing doubles | `emit`/`write_stdout`/`sink`; `writer`, `master` | A | 9.4b |
| `application/test_fixtures.rs` (`#[cfg(test)]`) | `IoJournal` (9.4b); `TerminalSessionHarness`, `SID` (9.5a); `maybe_panic`, `set_panic_in_key_handling`, `subs`, `shell_line` (9.5c) | `session()` and helpers `:1779-1880` | A | 9.4b-9.5c |
| `application/ports/terminal_modes.rs`, `live_settings.rs`, `working_directory.rs` | ports + doubles | `modes`; `settings_repository`/`load_or_defaults`; `cwd()` | A | 9.4c |
| `application/overlay_painter.rs` | `OverlayPainter` | `frame`, `redraw`, `take_repaint` | A | 9.5a |
| `application/session_ports_parts.rs`, `application/session_use_cases.rs` | `SessionPortsParts` (11 fields), `SessionUseCases` | new | A | 9.5a (grows) |
| `application/session_use_cases/tests.rs` | the characterisation scenarios 1-10 and 13 over the harness | new | A | 9.6f |
| `application/completion_requester.rs`, `session_file_poller.rs` | the two services | `refresh`, `cwd`; `poll_files` | A | 9.5b |
| `application/keystroke_handler.rs`, `handle_keystrokes_use_case.rs` | `KeystrokeHandler`, the use case | `on_keys`, `insert`, `release_held` | A | 9.5c |
| `application/process_shell_output_use_case.rs` | | `on_output`, `on_event` | A | 9.5d |
| `application/apply_completion_result_use_case.rs`, `apply_llm_ghost_text_use_case.rs` | | `on_completion`; `Msg::Ghost` arm | A | 9.5e |
| `application/resize_terminal_use_case.rs`, `handle_shell_exit_use_case.rs`, `shell_end.rs` | | `on_resize`; `ChildExit/PtyEof/Term` arms | A | 9.5f |
| `application/handle_timer_tick_use_case.rs` | | `on_timers` | A | 9.5g |
| `application/end_session_use_case.rs`, `apply_terminal_background_use_case.rs`, `prepare_session_launch_use_case.rs`, `launch_request.rs`, `launch_environment.rs` | | `cleanup`; `on_input` probe arm; `run`'s resolution | A | 9.5h |
| `application/ports/completion_engine.rs`, `application/report_session_context_use_case.rs` (exist) | | Phase 4/6 | A | |
| `infrastructure/adapters/output_filter.rs` | `OutputFilter` (+ 4 tests) | `OutFilter`, `tracker.rs:283-470` | I | 9.2a |
| `infrastructure/adapters/headless_model.rs` | `type Model = Term<VoidListener>` | `types::Model` | I | 9.2a |
| `infrastructure/adapters/cell_tagger.rs` | `apply(model, ScreenCommand)` | `tag`, `mark_line_start` `:992-1032` | I | 9.2a |
| `infrastructure/adapters/edit_buffer_extractor.rs` | `extract`, `Scraped`, `matches_cell`, `cell_col`, `strip_pwsh_prompts` | `tracker.rs:248-254, 763-945` | I | 9.2b |
| `infrastructure/adapters/overlay_renderer/tests.rs` | the renderer's tests, sliced | `overlay_renderer.rs` inline tests | I | 9.2c |
| `infrastructure/adapters/alacritty_terminal_screen.rs` (+ `infrastructure/adapters/alacritty_terminal_screen/tests.rs`) | `AlacrittyTerminalScreen` | `Term` + `Processor` + filter + renderer | I | 9.2d; tests 9.2f |
| `infrastructure/adapters/stdout_terminal_writer.rs`, `pty_wrapped_shell.rs` | adapters | `write_stdout`; `writer` thread channel + `master` | I | 9.4b |
| `infrastructure/adapters/console_terminal_modes.rs`, `lenient_live_settings.rs`, `process_working_directory.rs` | adapters | `modes`; `load_or_defaults`; `current_dir()` | I | 9.4c |
| `infrastructure/adapters/{file_session_context_repository, icon_assets, kitty_graphics, overlay_renderer}.rs` (exist) | | Phases 4, 8 | I | |
| `infrastructure/pty/session_message.rs`, `pty_reader.rs`, `terminal_probe.rs` | `SessionMessage`; `pump`, `spawn_pty_reader`, `PTY_CREDITS`; `PROBE`, `PROBE_TIMEOUT`, `parse_*` (9.6a); `probe_terminal` (9.6e: it takes the `TerminalWriter` port of 9.4b) | `types::Msg`; `pump`; probe `:63-65, 251-310, 432-443` | I | 9.6a; 9.6e |
| `infrastructure/pty/platform_unix.rs`, `platform_windows.rs`, `completion_engine_starter.rs` | the `sys` modules verbatim; the starter alias | `pty.rs:1319-1604`; `:74` | I | 9.6b |
| `infrastructure/pty/fallback_shell.rs` | `run_fallback_shell(error, argv, reaper)` | `fallback` `:133-157` | I | 9.4a |
| `infrastructure/pty/session_runner.rs` (+ `infrastructure/pty/session_runner/tests.rs`) | `SessionRunner`, `SessionRunnerParts` | `Session` driver half | I | 9.6d; tests 9.6f |
| `infrastructure/pty/session_launcher.rs`, `pty_session.rs` | `launch_session`, `generate_session_id`; `run_pty_session`, `env_nonempty`, `attempt` | `Session::start`; `run` | I | 9.6e |
| `infrastructure/cli/key_tester_command.rs`, `pty_command.rs`, `debug_shell_command.rs` | the three commands | `cli.rs:190-304`; `bootstrap/cli.rs` pty arm | I | 9.6c |
| `infrastructure/terminal_session_module.rs` (exists) | the module grows: collaborators, getters, `prepare_session_launch`, `locate_data_root` | Phase 4 | I | 9.4a, 9.4c, 9.5h, 9.6c |
| `infrastructure/cli/pre_cmd_command.rs` (exists) | unchanged: its caller builds the module through `terminal_session_module()` | Phase 4 | I | |

The final tree of `src/terminal_session/` (generated from the table above; 116 files: the 23 that exist now plus 93 new ones, `mod.rs` files and sliced `tests.rs` modules included; `adapters/stdin_key_decoder.rs` leaves by `git mv` to `domain/entities/key_decoder.rs`; a `tests.rs` inside a folder named like a sibling file is that file's test module; `domain/entities/terminal_session/tests.rs` exists only if the aggregate's tests pass ~500 lines; every `mod.rs` only declares its children, with `#[cfg(unix)]`/`#[cfg(windows)]` attributes on the two platform files and `#[cfg(test)]` on `test_fixtures`):

```
src/terminal_session/
|-- mod.rs
|-- application/
|   |-- mod.rs
|   |-- ports/
|   |   |-- mod.rs
|   |   |-- completion_engine.rs
|   |   |-- generator_process_reaper.rs
|   |   |-- live_settings.rs
|   |   |-- terminal_modes.rs
|   |   |-- terminal_screen.rs
|   |   |-- terminal_writer.rs
|   |   |-- working_directory.rs
|   |   `-- wrapped_shell.rs
|   |-- session_use_cases/
|   |   `-- tests.rs
|   |-- apply_completion_result_use_case.rs
|   |-- apply_llm_ghost_text_use_case.rs
|   |-- apply_terminal_background_use_case.rs
|   |-- completion_requester.rs
|   |-- end_session_use_case.rs
|   |-- frame_input.rs
|   |-- handle_keystrokes_use_case.rs
|   |-- handle_shell_exit_use_case.rs
|   |-- handle_timer_tick_use_case.rs
|   |-- keystroke_handler.rs
|   |-- launch_environment.rs
|   |-- launch_request.rs
|   |-- overlay_painter.rs
|   |-- prepare_session_launch_use_case.rs
|   |-- process_shell_output_use_case.rs
|   |-- report_session_context_use_case.rs
|   |-- resize_terminal_use_case.rs
|   |-- session_file_poller.rs
|   |-- session_ports_parts.rs
|   |-- session_use_cases.rs
|   |-- shell_end.rs
|   |-- shell_output_feeder.rs
|   `-- test_fixtures.rs
|-- domain/
|   |-- mod.rs
|   |-- entities/
|   |   |-- mod.rs
|   |   |-- terminal_session/
|   |   |   `-- tests.rs
|   |   |-- echo_hold.rs
|   |   |-- ghost_candidates.rs
|   |   |-- insertion_lock.rs
|   |   |-- key_decoder.rs
|   |   |-- marker_splitter.rs
|   |   |-- marker_tracker.rs
|   |   |-- reload_watch.rs
|   |   |-- resize_debounce.rs
|   |   |-- session_context.rs
|   |   |-- shell_exit.rs
|   |   `-- terminal_session.rs
|   |-- repositories/
|   |   |-- mod.rs
|   |   `-- session_context_repository.rs
|   |-- services/
|   |   |-- mod.rs
|   |   |-- insertion_reconciler.rs
|   |   |-- key_router.rs
|   |   |-- render_guard.rs
|   |   `-- session_deadlines.rs
|   |-- value_objects/
|   |   |-- mod.rs
|   |   |-- cell_tag.rs
|   |   |-- completion_query.rs
|   |   |-- deferred_marker.rs
|   |   |-- edit_buffer.rs
|   |   |-- edit_buffer_context.rs
|   |   |-- key_event.rs
|   |   |-- key_facts.rs
|   |   |-- key_route.rs
|   |   |-- lock_settlement.rs
|   |   |-- marker_effects.rs
|   |   |-- marker_reaction.rs
|   |   |-- output_piece.rs
|   |   |-- pwsh_prediction_color.rs
|   |   |-- screen_command.rs
|   |   |-- screen_state.rs
|   |   |-- session_id.rs
|   |   |-- shell_event.rs
|   |   |-- shell_marker.rs
|   |   |-- shell_suggestion_color.rs
|   |   |-- terminal_background.rs
|   |   |-- text_emphasis.rs
|   |   `-- wrapped_shell_command.rs
|   `-- terminal_session_error.rs
`-- infrastructure/
    |-- mod.rs
    |-- adapters/
    |   |-- mod.rs
    |   |-- alacritty_terminal_screen/
    |   |   `-- tests.rs
    |   |-- overlay_renderer/
    |   |   `-- tests.rs
    |   |-- alacritty_terminal_screen.rs
    |   |-- cell_tagger.rs
    |   |-- console_terminal_modes.rs
    |   |-- edit_buffer_extractor.rs
    |   |-- file_session_context_repository.rs
    |   |-- headless_model.rs
    |   |-- icon_assets.rs
    |   |-- kitty_graphics.rs
    |   |-- lenient_live_settings.rs
    |   |-- output_filter.rs
    |   |-- overlay_renderer.rs
    |   |-- process_working_directory.rs
    |   |-- pty_wrapped_shell.rs
    |   `-- stdout_terminal_writer.rs
    |-- cli/
    |   |-- mod.rs
    |   |-- debug_shell_command.rs
    |   |-- key_tester_command.rs
    |   |-- pre_cmd_command.rs
    |   `-- pty_command.rs
    |-- pty/
    |   |-- mod.rs
    |   |-- session_runner/
    |   |   `-- tests.rs
    |   |-- completion_engine_starter.rs
    |   |-- fallback_shell.rs
    |   |-- platform_unix.rs
    |   |-- platform_windows.rs
    |   |-- pty_reader.rs
    |   |-- pty_session.rs
    |   |-- session_launcher.rs
    |   |-- session_message.rs
    |   |-- session_runner.rs
    |   `-- terminal_probe.rs
    `-- terminal_session_module.rs
```

Allowed edges used: terminal_session to shared (`Shell`, `Clock`, `DataDirectory`, `SharedDependencies`, `error_chain`, `logger`), settings (`Settings`, `SettingsRepository`, `SettingsError`, catalog keys), history (`HistoryEntry`),
completion (`CompletionRequest`, `CompletionResult`, `Insertion`, `Suggestion`), ghost_text (`GhostText`, `ghost_text_source_resolver`), dropdown (`Key`, `KeyAction`, `Keymap`, `Menu`, `MenuEffect`, `Theme`, `ThemeRepository`,
`resolve_theme`, `Rgb`, `GraphicsSupport`); test code only: specs (`SuggestionKind`), settings/dropdown infrastructure, bootstrap (the `debug_shell_survives_closed_terminal` child). `anyhow` only in `infrastructure/pty` and
`infrastructure/cli`; `clap` nowhere in terminal_session (`DebugCmd` and `InternalCmd` are bootstrap types); `alacritty_terminal`, `portable_pty`, `crossterm`, `libc`, `signal_hook`, `windows_sys` only in infrastructure; `serde`,
`serde_json` only where they are today (the session-context adapter) and in test modules.

Outside `src/terminal_session/`:

| File | Change | Step |
|---|---|---|
| `architecture_tests.rs` | `"terminal_session"` in `REGISTERED_MODULES`; the comment example at `:376` re-worded | 9.0a; 9.6f |
| `scripts/hexagonal-refactor/{fitness-mod.sh, phase9-greps.sh, session-diff.py, pty-diff.py}` | new | 9.0a |
| `shared/infrastructure/testing/{mod.rs, fixture_log.rs}` | `assert_matches_fixture` | 9.0b |
| `tests/fixtures/terminal_session/characterisation/*.log` | 17 text fixtures, written once | 9.0b, 9.0c |
| `main.rs` | `- mod tracker;` (9.2f), `- mod pty;` (9.6d) | 9.2f, 9.6d |
| `types.rs` | `- Model` (9.2f), `- Msg` (9.6a); `now_ms`/`now_secs` stay (Phase 11) | 9.2f, 9.6a |
| `pty.rs` (legacy) | shrinks every step; `git mv` into `infrastructure/pty/session_runner.rs` | 9.6d |
| `cli.rs` (legacy) | loses `DebugCmd`, `debug`, `key_line`, `key_tester`, `debug_shell`, `debug_shell_files`, 3 tests; `debug_logs` becomes `pub fn` | 9.4a (signature), 9.6c |
| `bootstrap/cli.rs` | `terminal_session_module()`, `DebugCmd`, the debug and pty arms | 9.4a, 9.6c |
| `bootstrap/{channel_completion_engine, engine_server, engine_server/tests}.rs` | `Msg` to `SessionMessage` | 9.6a |
| `bootstrap/bridges.rs` | + `GeneratorProcessBridge` | 9.4a |
| `settings/domain/services/settings_catalog.rs` | four comments naming `pty.rs` repointed | 9.6f |

## Signatures (the contract of the phase)

Every item is `pub(crate)`. Names are the glossary names of "Glossary additions". Signatures marked (v) were compiled in a prototype copy of HEAD (the aggregate's against a STAND-IN `MarkerTracker`: its tests open a prompt with a `#[cfg(test)] open_prompt()` that the real tracker lacks, see 9.3g; tracker domain:
`target/hexagonal-refactor/p9/recon-tracker-domain-prototype/`, 38 tracker tests plus a 36 000-batch differential test; session domain: `recon-session-domain-prototype/`, 90 domain tests); the implementer
rebuilds them test-first in the named steps and may lift the prototype code.

### Tracker domain (9.1)

```rust
// value_objects/shell_marker.rs (v)
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum ShellMarker {
    StartPrompt, EndPrompt,
    NewCmd(Option<String>),                 // raw UNTRIMMED value; None = a bare "NewCmd"
    PreExec, NoHistory,
    OscLock(String), OscUnlock(String),     // trimmed ("" for a bare key)
    Dir(PathBuf),                           // UNTRIMMED; a bare "Dir" is the empty path
    ExitCode(Option<i32>), Pid(Option<i32>),// trimmed parse; unparsable or bare = None
    Shell(String),                          // trimmed; bare = ""
    ZshAutosuggestionColor(Option<ShellSuggestionColor>), FishSuggestionColor(Option<ShellSuggestionColor>),
    PwshPredictionColor(Option<PwshPredictionColor>),
    PwshListView(bool),                     // trimmed == "1"
    PwshPrompts { continuation: String, prompt_texts: Vec<String> },   // UNTRIMMED, split on '\x1f', empty texts dropped
    Ignored,                                // "key=", an unknown key, ShellPath, TTY, User
}
impl ShellMarker { fn parse(payload: &str) -> ShellMarker; fn passes_integration_lock(&self) -> bool }  // parse is total; first '=' splits

// value_objects/{cell_tag, screen_command, shell_event, marker_effects, marker_reaction}.rs (v)
#[derive(Clone, Copy, Debug, PartialEq, Eq)] enum CellTag { Prompt, Cycle(u64) }
impl CellTag { const PROMPT_URI: &'static str = "tabsmith:p"; fn uri(self) -> String /* "tabsmith:c{n}" */ }
#[derive(Clone, Copy, Debug, PartialEq)] enum ScreenCommand { Tag(CellTag), ClearTag, MarkLineStart { redrawn_cycle: Option<u64> } }
#[derive(Debug, PartialEq)] enum ShellEvent { NewCommand(Option<HistoryEntry>), CommandEnded, PreExec }
#[derive(Debug, Default, PartialEq)] struct MarkerEffects { /* screen_commands, events */ }
impl MarkerEffects { fn screen_commands(&self) -> &[ScreenCommand]; fn events(&self) -> &[ShellEvent]; fn into_events(self) -> Vec<ShellEvent>;
                     fn push_screen_command(&mut self, c: ScreenCommand); fn push_event(&mut self, e: ShellEvent) }
#[derive(Debug, PartialEq)] enum MarkerReaction { Handled(MarkerEffects), EditBufferNeeded(MarkerEffects) }

// value_objects/edit_buffer.rs, edit_buffer_context.rs (v)
#[derive(Clone, Debug, Default, PartialEq)] struct EditBuffer { /* private */ }
impl EditBuffer { fn new(buffer: String, cursor: usize, shell_ghost: bool, at_prompt: bool) -> Self;
                  fn buffer(&self) -> &str; fn cursor(&self) -> usize /* BYTE offset */; fn shell_ghost(&self) -> bool; fn at_prompt(&self) -> bool }
struct EditBufferContextParts<'a> { cycle: u64, shell: Option<Shell>, line_col0: bool, pwsh_list_view: bool, pwsh_continuation: &'a str,
    pwsh_prompt_text: &'a [String], zsh_color: Option<&'a ShellSuggestionColor>, fish_color: Option<&'a ShellSuggestionColor>,
    pwsh_color: Option<&'a PwshPredictionColor> }                      // all pub(crate): a Parts struct (clippy: 7 arguments)
struct EditBufferContext<'a>;  impl<'a> EditBufferContext<'a> { fn new(p: EditBufferContextParts<'a>) -> Self; fn cycle(&self) -> u64; fn shell(&self) -> Option<Shell>;
    fn line_starts_at_column_zero(&self) -> bool; fn pwsh_list_view(&self) -> bool; fn pwsh_continuation(&self) -> &str;
    fn pwsh_prompt_text(&self) -> &[String]; fn zsh_color(&self) -> Option<&ShellSuggestionColor>; fn fish_color(&self) -> Option<&ShellSuggestionColor>;
    fn pwsh_color(&self) -> Option<&PwshPredictionColor> }

// value_objects/{shell_suggestion_color, text_emphasis, pwsh_prediction_color}.rs (v)
struct ShellSuggestionColor { fg: Vec<Option<u8>>, bg: Vec<Option<u8>> }  // Clone, Debug, Default, PartialEq; None = the terminal's default colour
impl ShellSuggestionColor { fn new(fg: Vec<Option<u8>>, bg: Vec<Option<u8>>) -> Self; fn fg(&self) -> &[Option<u8>]; fn bg(&self) -> &[Option<u8>];
    fn parse_zsh(spec: &str) -> Option<Self>; fn parse_fish(spec: &str) -> Option<Self>; fn matches(&self, fg: Option<u8>, bg: Option<u8>) -> bool /* empty list = any */ }
fn rgb_to_256(r: u8, g: u8, b: u8) -> u8;
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)] struct TextEmphasis { dim: bool, italic: bool }
impl TextEmphasis { const fn new(dim: bool, italic: bool) -> Self; fn is_empty(self) -> bool; fn is_within(self, other: TextEmphasis) -> bool /* old other.contains(self) */; fn is_dim_or_italic(self) -> bool }
#[derive(Clone, Debug, PartialEq)] struct PwshPredictionColor { colors: ShellSuggestionColor, emphasis: TextEmphasis }
impl PwshPredictionColor { fn parse(spec: &str) -> Option<Self> /* old parse_sgr */; fn colors(&self) -> &ShellSuggestionColor; fn emphasis(&self) -> TextEmphasis;
    fn matches(&self, fg: Option<u8>, bg: Option<u8>, cell: TextEmphasis) -> bool /* colours match && emphasis.is_within(cell) */ }

// entities/marker_splitter.rs, value_objects/output_piece.rs (pure move)
#[derive(Default)] struct MarkerSplitter { /* state, osc, name */ }  impl MarkerSplitter { fn feed(&mut self, data: &[u8]) -> Vec<OutputPiece> }
#[derive(Debug, PartialEq)] enum OutputPiece { Bytes(Vec<u8>), Marker(String), WindowName(Vec<u8>) }

// entities/marker_tracker.rs (v)
struct MarkerTracker { /* session_id, defer_markers, osc_lock, preexec, cycle, line_col0, cwd, shell, shell_name, pid, colours, submitted, no_history,
                          pwsh_list_view, pwsh_color, pwsh_continuation (">> "), pwsh_prompt_text, pending, exit_code, deferred, batch_markers */ }
impl MarkerTracker {
    fn new(session_id: SessionId, shell: Option<Shell>, defer_markers: bool) -> Self;
    fn on_marker(&mut self, marker: ShellMarker, cursor_column: usize, now_secs: u64) -> MarkerReaction;
    fn complete_pre_exec(&mut self, scraped: Option<&EditBuffer>, now_secs: u64) -> MarkerEffects;
    fn end_batch(&mut self, cursor_column: usize, now_secs: u64) -> MarkerEffects;        // ALWAYS called, even for an empty batch
    fn edit_buffer_context(&self) -> Option<EditBufferContext<'_>>;                       // None <=> no open cycle, or a command runs
    fn at_prompt(&self) -> bool;                                                          // cycle open && !preexec (NOT EditBuffer::at_prompt)
    fn preexec(&self) -> bool; fn cwd(&self) -> Option<&Path>; fn shell(&self) -> Option<Shell>; fn pwsh_list_view(&self) -> bool;
    fn submitted(&self) -> Option<&str>; fn set_submitted(&mut self, line: Option<String>);
    #[cfg(test)] fn cycle(&self) -> Option<u64>; fn osc_lock(&self) -> bool; fn no_history(&self) -> bool; fn pending_is_empty(&self) -> bool;
                 fn deferred(&self) -> &VecDeque<DeferredMarker>; fn set_pwsh_list_view(&mut self, on: bool);
}
// value_objects/deferred_marker.rs (v): #[derive(Clone, Copy, Debug, PartialEq)] enum DeferredMarker { EndPrompt, NewCmd }   // its own file: one type per file
// value_objects/session_id.rs: fn from_entropy(nanos: u64, pid: u32) -> SessionId   // format!("{nanos:016x}{:016x}", pid as u64 ^ nanos.rotate_left(17))
```
Per marker kind, the calls (application side; the cursor column is `screen.cursor_column()` read just before the tracker call, `now_secs` is `clock.now_secs()`):
`Bytes` -> `screen.pass_through`; `WindowName` -> `screen.pass_window_name_through`; `StartPrompt` -> `Handled([Tag(Prompt)])`; `EndPrompt` -> `[Tag(Cycle(n))]` when a cycle is open and no command runs, else `[ClearTag]`, queued when
deferring; `NewCmd=<our sid>` -> `[Tag(Cycle(n)), MarkLineStart{redrawn_cycle}]` plus `NewCommand(None)` or one event per drained pending entry (innermost first), queued when deferring; a bare, foreign or space-padded `NewCmd` -> nothing;
`PreExec` (no command running) -> `EditBufferNeeded(flush effects)`, then `screen.edit_buffer(&ctx)` (no `at_prompt`/alt filter), then `complete_pre_exec` -> `[ClearTag]` + `PreExec`; `NoHistory` (no command running) -> the flush effects;
everything else -> `Handled(empty)`; after the last piece `end_batch(cursor_column())`.

### Adapters and the screen port (9.2)

```rust
// application/ports/terminal_screen.rs
pub(crate) trait TerminalScreen {
    fn pass_through(&self, bytes: &[u8], out: &mut Vec<u8>);              // the model (then stop_sync), THEN the filter appends what may reach the terminal to `out`
    fn pass_window_name_through(&self, name: &[u8], out: &mut Vec<u8>);   // the filter only, never the model
    fn apply(&self, command: ScreenCommand);                              // model only; in the order given, each before the next byte is advanced
    fn cursor_column(&self) -> usize;                                     // grid().cursor.point.column.0; a pending wrap is NOT past the last column
    fn edit_buffer(&self, context: &EditBufferContext<'_>) -> Option<EditBuffer>;   // the old extract(); the caller filters on at_prompt / alt
    fn state(&self) -> ScreenState;                                       // fresh snapshot; never cache it across a pass_through
    fn resize(&self, columns: usize, lines: usize);                       // term.resize, filter.resize(lines), renderer.drop_clear — this order
    fn draw_overlay(&self, menu: &Menu, input: &FrameInput<'_>) -> Vec<u8>;
    fn take_clear(&self) -> Vec<u8>;                                      // taken LATE after output; also by guard and EndSession
    fn full_repaint(&self) -> Vec<u8>;                                    // drops the cached clear first
    fn set_theme(&self, theme: Theme);                                    // UNCONDITIONAL (also for an equal theme): forgets the last frame
}
// application/frame_input.rs
pub(crate) struct FrameInput<'a> { pub(crate) settings: &'a Settings, pub(crate) ghost: Option<&'a str>, pub(crate) spinner: usize,
    pub(crate) suppress: bool, pub(crate) scroll_region: bool, pub(crate) now: Instant }
// domain/value_objects/screen_state.rs
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)] pub(crate) struct ScreenState { /* 8 private fields */ }
pub(crate) struct ScreenStateParts { pub(crate) alt_screen: bool, origin_mode: bool, insert_mode: bool, pending_wrap: bool, columns: usize, lines: usize, scroll_region: bool, in_string: bool }
impl ScreenState { fn new(p: ScreenStateParts) -> Self; /* eight accessors, same names */ fn size(&self) -> (usize, usize) /* (columns, lines) */; #[cfg(test)] fn plain(columns: usize, lines: usize) -> Self }
// infrastructure/adapters/alacritty_terminal_screen.rs
pub(crate) struct AlacrittyTerminalScreen { inner: RefCell<Inner> }   // Inner { term: Model, processor: Processor, filter: OutputFilter, renderer: OverlayRenderer }
impl AlacrittyTerminalScreen {
    fn new(columns: usize, lines: usize, renderer: OverlayRenderer, lf_to_ind: bool) -> Self;    // scrolling_history 2000
    fn advance_model(&self, bytes: &[u8]);                                                     // startup CUP only; NOT on the port
    #[cfg(test)] fn for_tests(columns: usize, lines: usize) -> Self;                           // Config::default(), Theme::dark(), truecolor false, lf_to_ind false
    #[cfg(test)] fn history_size(&self) -> usize;  #[cfg(test)] fn model(&self) -> std::cell::Ref<'_, Model>;  #[cfg(test)] fn panic_while_borrowed(&self);   // test hook of the panic test
}
// infrastructure/adapters/{output_filter, cell_tagger, edit_buffer_extractor}.rs
pub(crate) struct OutputFilter { /* carry, rows, alt, partial_region, in_string, long_seq */ pub(crate) lf_to_ind: bool }
impl OutputFilter { fn new(rows: usize) -> Self; fn resize(&mut self, rows: usize); fn scroll_region(&self) -> bool; fn in_string(&self) -> bool; fn feed(&mut self, bytes: &[u8], out: &mut Vec<u8>) }
pub(crate) fn cell_tagger::apply(model: &mut Model, command: ScreenCommand);
pub(crate) fn edit_buffer_extractor::extract(model: &Model, context: &EditBufferContext<'_>) -> Option<EditBuffer>;
// application/shell_output_feeder.rs
pub(crate) struct ShellOutputFeeder { screen: Rc<dyn TerminalScreen>, clock: Arc<dyn Clock + Send + Sync> }
impl ShellOutputFeeder { fn new(screen: Rc<dyn TerminalScreen>, clock: Arc<dyn Clock + Send + Sync>) -> Self;
    fn feed(&self, splitter: &mut MarkerSplitter, tracker: &mut MarkerTracker, batch: &[u8], out: &mut Vec<u8>) -> Vec<ShellEvent> }
```

### Ports of the session (9.4)

```rust
pub(crate) trait TerminalWriter { fn write(&self, bytes: &[u8]); }        // the user's terminal; an empty slice writes nothing; errors ignored
pub(crate) trait WrappedShell { fn write_input(&self, bytes: Vec<u8>); fn resize(&self, columns: u16, rows: u16); }   // columns FIRST; a gone shell is ignored
pub(crate) trait GeneratorProcessReaper { fn kill_live_children(&self); }
pub(crate) trait TerminalModes { fn restore(&self); }
pub(crate) trait LiveSettings { fn last_modified(&self) -> Option<SystemTime>; fn load(&self) -> Settings; }   // load is lenient: defaults + the warning logged by the adapter
pub(crate) trait WorkingDirectory { fn current(&self) -> Option<PathBuf>; }
// doubles (#[cfg(test)], in the port's file): CapturingTerminalWriter { new, journaled(Rc<IoJournal>), written() -> Vec<u8>, clear() },
// CapturingWrappedShell { new, journaled, take_input() -> Vec<u8>, take_writes() -> Vec<Vec<u8>>, resizes() -> Vec<(u16, u16)> }, RecordingGeneratorProcessReaper { new, kills() -> usize },
// RecordingTerminalModes { new, restores() -> usize }, InMemoryLiveSettings { new(Settings), set(Settings, SystemTime), remove() }, FixedWorkingDirectory::new(Option<PathBuf>)
// IoJournal (test_fixtures.rs): RefCell<Vec<Io>>, enum Io { Stdout(Vec<u8>), ShellInput(Vec<u8>) }, new(), events() -> Vec<Io>
// adapters (infrastructure/adapters): StdoutTerminalWriter (write_stdout verbatim), PtyWrappedShell::new(input: Sender<Vec<u8>>, master: Box<dyn MasterPty + Send>) + inherent release_pseudo_console(&self),
// ConsoleTerminalModes::new(Modes), LenientLiveSettings::new(Rc<dyn SettingsRepository>) (load = repository.load() or logger::warn("settings: {chain}") + Settings::default()), ProcessWorkingDirectory (std::env::current_dir().ok())
// bootstrap/bridges.rs: GeneratorProcessBridge (impl GeneratorProcessReaper over specs' kill_live_children())
```

### The aggregate (9.3g-9.3i) (v: compiled against a stand-in `MarkerTracker`, see 9.3g)

```rust
pub(crate) const LATE_PROBE: Duration = Duration::from_secs(3);
pub(crate) struct TerminalSessionParts { pub(crate) id: SessionId, launched_shell: Shell, settings: Settings, tracker: MarkerTracker,
    terminal_background: Option<TerminalBackground>, settings_modified: Option<SystemTime>, now: Instant }
impl TerminalSession {
    fn new(parts: TerminalSessionParts) -> Self;      // keymap = Keymap::from_settings, menu = Menu::new(), splitter + decoder default, poll_at = now + FILE_POLL, changed_at = now
    // reads
    fn id(&self) -> &SessionId; fn settings(&self) -> &Settings; fn menu(&self) -> &Menu; fn tracker(&self) -> &MarkerTracker;
    fn output_parts(&mut self) -> (&mut MarkerSplitter, &mut MarkerTracker); fn terminal_background(&self) -> Option<TerminalBackground>; fn request_id(&self) -> u64;
    fn sent(&self) -> Option<(&str, usize)>; fn edit_buffer(&self) -> Option<&EditBuffer>; fn spinner_frame(&self) -> usize; fn has_insertion_lock(&self) -> bool;
    fn tracked_cwd(&self) -> Option<&Path>; fn shell(&self) -> Shell /* tracker.shell().unwrap_or(launched_shell) */; fn is_passthrough(&self) -> bool;
    fn menu_intercepts(&self) -> bool /* the step's `raw`, pty.rs:619 */; fn basis(&self) -> Option<String>;
    fn ghost_text(&self, typed: bool, screen: &ScreenState) -> Option<String>; fn suppressed(&self, screen: &ScreenState) -> bool; fn drawable(&self, screen: &ScreenState) -> bool;
    // settings, output, events, requests
    fn apply_settings(&mut self, settings: Settings);                                        // settings + keymap together
    fn output_arrived(&mut self, now: Instant); fn screen_changed(&mut self, before: &ScreenState, after: &ScreenState);
    fn on_shell_event(&mut self, event: &ShellEvent, now: Instant) -> bool;                  // true = look at the files
    fn refresh(&mut self, screen: &ScreenState, extracted: Option<EditBuffer>, now: Instant) -> Option<CompletionQuery>;   // owns the alt rule and the at_prompt filter
    fn apply_completion(&mut self, c: CompletionResult, now: Instant) -> bool;               // false = stale reply, dropped (no redraw, no release_held)
    fn accept_llm_reply(&mut self, reply: u64, text: Option<String>) -> bool;                // true = redraw
    fn forget_sent(&mut self);
    // keys
    fn decode_keys(&mut self, chunk: &[u8]) -> Vec<KeyEvent>; fn is_holding_keys(&self) -> bool; fn hold_behind(&mut self, chunk: &[u8]); fn hold_keys(&mut self, keys: &[u8], now: Instant) /* resets the decoder */;
    fn release_held_keys(&mut self, now: Instant) -> Option<Vec<u8>>; fn route_key(&self, key: &Key, typed: usize, screen: &ScreenState) -> KeyRoute;
    fn run_menu_action(&mut self, action: KeyAction) -> Vec<MenuEffect>; fn show_menu_on_tab(&mut self) -> Vec<MenuEffect>; fn accept_ghost(&mut self, text: &str);
    fn forward_key(&mut self, key: &Key) -> bool /* true = ends the line */; fn insert(&mut self, ins: &Insertion, now: Instant) -> Option<Vec<u8>> /* None = dropped: no record */;
    fn wrote_to_shell(&mut self, bytes: &[u8]);
    // probe, resize, timers, exit
    fn expect_late_probe_reply(&mut self, probe_ended_at: Instant); fn late_probe_open(&self, now: Instant) -> bool; fn probe_background_reported(&mut self, bg: TerminalBackground); fn probe_cursor_reported(&mut self);
    fn resize_applied(&mut self, now: Instant); fn settle_resize_if_due(&mut self, now: Instant); fn take_repaint(&mut self, screen: &ScreenState) -> bool; fn take_due_redraw_grace(&mut self, now: Instant) -> bool;
    fn insertion_lock_due(&self, now: Instant) -> bool; fn take_due_llm_request(&mut self, now: Instant) -> Option<(u64, String)>; fn tick_spinner(&mut self, now: Instant, judged_at: Instant) -> bool;
    fn poll_due(&mut self, now: Instant) -> bool; fn context_changed(&mut self, modified: Option<SystemTime>) -> bool; fn settings_changed(&mut self, modified: Option<SystemTime>) -> bool;
    fn enter_passthrough(&mut self); fn child_exited(&mut self, code: i32, now: Instant) -> bool; fn shell_output_ended(&mut self) -> bool; fn terminated(&mut self);
    fn exit_grace_over(&self, now: Instant) -> bool; fn exit_code(&self) -> i32 /* unwrap_or(0) */; fn next_deadline(&self) -> Option<Instant>;
    #[cfg(test)] fn tracker_mut(&mut self) -> &mut MarkerTracker /* production reaches the tracker mutably only through output_parts() */; fn unechoed(&self) -> &str; fn llm_due_at(&self) -> Option<Instant>; fn cancel_llm_schedule(&mut self); fn expect_llm_reply(&mut self, request: u64); fn set_redraw_until(&mut self, at: Option<Instant>);
                 fn expire_hold_at(&mut self, at: Instant); fn expire_insertion_deadline(&mut self, deadline: Instant) /* keeps the cap */; fn expire_insertion_lock(&mut self, deadline: Instant, cap: Instant);
                 fn reported_exit_code(&self) -> Option<i32>; fn late_probe_until(&self) -> Option<Instant>; fn context_modified(&self) -> Option<SystemTime>;
}
// entities: InsertionLock { new(now, expected, before), expected(), deadline(), is_due(now), settle(&mut self, shown: Option<&str>, now) -> LockSettlement }   // value_objects/lock_settlement.rs: #[derive(Clone, Copy, Debug, PartialEq, Eq)] enum LockSettlement { StillHeld, Released }
//   EchoHold (Default) { unechoed, consume_echo, typed_ahead, forwarded_unmodelled, accepted_ghost, forget_unechoed, reset, forget_pending, answered_by (strict >), is_waiting_for_echo, is_holding, hold_behind, hold, release(now), deadline }
//   GhostCandidates (Default) { reset, accepted, replace_local, schedule_llm, cancel_llm_schedule, llm_due_at, take_due_llm, expect_llm_reply, accept_llm_reply(reply, latest_request, text, sent_buffer) -> bool, rest_after(&str) }
//   ResizeDebounce (Default) { resized, output_arrived, is_settling, repaint_pending, settle_if_due, take_repaint(alt_screen) -> bool, deadline, wants(columns, lines, &ScreenState) -> bool }
//   ShellExit (Default) { child_exited(code, now) -> bool, output_ended() -> bool, terminated(), grace_over(now), deadline(), code(), exit_code() -> i32 }   ReloadWatch { new(now, settings_modified), poll_due, deadline, context_changed, settings_changed }
//   KeyDecoder (Default) { decode(&mut self, &[u8]) -> Vec<KeyEvent>, reset() }
// services: insertion_reconciler::{insertion_bytes(&Insertion,&str,&str)->Vec<u8>, expected_buffer(&Insertion,&str,usize)->String, unechoed_after(&str,Option<&str>,Option<&str>,bool)->String};
//   key_router::{route(&Key,&Keymap,&KeyFacts)->KeyRoute, ghost_key, is_ctrl, plain_char, ends_line, is_enter}; render_guard::allows_drawing(&ScreenState)->bool;
//   session_deadlines::{RESIZE_DEBOUNCE 150ms, REDRAW_GRACE 50ms, SPINNER_TICK 80ms, FILE_POLL 2s, EXIT_GRACE 200ms, next_deadline(passthrough, exit, pending: impl IntoIterator<Item=Option<Instant>>) -> Option<Instant>}
// value objects: KeyRoute { Hold, MenuAction(KeyAction), Swallow, ShowOnTab, AcceptGhost(String), Forward }; KeyFacts{Parts} (prompt, menu_intercepts, drawable, menu_has_items, autocomplete_disabled, only_show_on_tab, waiting_for_echo, ghost_text);
//   CompletionQuery { request, buffer, cursor (byte), history_mode }; TerminalBackground(Rgb) { new, rgb }; WrappedShellCommand (below)
```

```rust
// domain/value_objects/wrapped_shell_command.rs (v, except window_title)
#[derive(Clone, Debug, PartialEq)] pub(crate) struct WrappedShellCommand { argv: Vec<String>, env_to_set: Vec<(String, String)>, env_to_remove: Vec<&'static str> }
impl WrappedShellCommand {
    fn new(shell: Option<&str>, extra: &[String], login: bool) -> Self;                  // argv, pty.rs:112-130 verbatim
    fn in_session(self, version: &str, in_tmux: bool, id: &SessionId) -> Self;           // set TABSMITH_TERM, TABSMITH_TERM_TMUX iff in_tmux, TABSMITH_SESSION_ID; remove TABSMITH_SHELL, TABSMITH_IS_LOGIN_SHELL (:479-486)
    fn argv(&self) -> &[String]; fn program(&self) -> Option<&str>; fn env_to_set(&self) -> &[(String, String)]; fn env_to_remove(&self) -> &[&'static str];
    fn window_title(&self) -> Vec<u8>;                                                   // b"\x1b]0;{base}\x07", base = program.rsplit(['/', '\\']).next() (:495-496); empty without a program
}
```

### Services and use cases (9.5)

```rust
// application/ — every execute takes the aggregate; the screen is a port object held by the service (R10)
pub(crate) struct OverlayPainter { screen, writer: Rc<dyn TerminalWriter>, clock }
  fn frame(&self, session: &TerminalSession) -> Vec<u8>;            // ONE ScreenState; ghost_text(false); suppress = suppressed() NOT drawable(); scroll_region from the state; now read LAST
  fn redraw(&self, session: &TerminalSession);                      // writer.write(&frame)
  fn take_repaint(&self, session: &mut TerminalSession) -> Vec<u8>; // session.take_repaint(&state) ? screen.full_repaint() : empty
pub(crate) struct CompletionRequester { engine, screen, working_directory: Rc<dyn WorkingDirectory>, clock }
  fn refresh(&self, session: &mut TerminalSession);                 // state; scrape unless alt; session.refresh(..); engine.complete(CompletionRequest{..}) LAST
  fn working_directory_of(&self, session: &TerminalSession) -> PathBuf;   // tracked_cwd or WorkingDirectory::current() (lazy) or empty
pub(crate) struct SessionFilePoller { live_settings, session_contexts, engine, screen, themes }
  fn poll(&self, session: &mut TerminalSession) -> bool;            // context first (mtime stored BEFORE the read), then settings (any Option difference; apply_settings; set_theme(resolve_theme(..)); engine.apply_settings LAST); true = settings changed
pub(crate) struct KeystrokeHandler { engine, screen, shell: Rc<dyn WrappedShell>, writer, clock, requester: Rc<CompletionRequester>, painter: Rc<OverlayPainter> }
  fn handle(&self, session: &mut TerminalSession, chunk: &[u8]);    // pty.rs:1044-1158 (the per-key loop; #[cfg(test)] maybe_panic() FIRST in the loop; shell.write_input LAST)
  fn release_held(&self, session: &mut TerminalSession);            // session.release_held_keys(now) then handle(held)
pub(crate) struct HandleKeystrokesUseCase { handler: Rc<KeystrokeHandler> }       fn execute(&self, session: &mut TerminalSession, chunk: &[u8]);
pub(crate) struct ProcessShellOutputUseCase { /* engine, screen, writer, clock, feeder, requester, painter, poller, handler */ }   // new(ProcessShellOutputParts)
  fn execute(&self, session: &mut TerminalSession, batch: &[u8]);   // output_arrived; before = state; feed; events (engine first, then on_shell_event, then poll); screen_changed; refresh; take_clear; take_repaint; frame; ONE write; release_held
pub(crate) struct ApplyCompletionResultUseCase { clock, painter, handler }       fn execute(&self, session: &mut TerminalSession, result: CompletionResult);   // apply_completion? then redraw, release_held
pub(crate) struct ApplyLlmGhostTextUseCase { painter }                           fn execute(&self, session: &mut TerminalSession, request: u64, text: Option<String>);   // accept_llm_reply? then redraw (NO release_held)
pub(crate) struct ResizeTerminalUseCase { screen, shell, clock }                 fn execute(&self, session: &mut TerminalSession, columns: u16, rows: u16);   // ignore 0 / the MODEL's size; shell.resize; screen.resize; resize_applied
pub(crate) enum ShellEnd { ChildExited(i32), OutputClosed, Terminated }
pub(crate) struct HandleShellExitUseCase { clock }                               fn execute(&self, session: &mut TerminalSession, end: ShellEnd) -> bool;   // true = the loop goes on
pub(crate) struct HandleTimerTickUseCase { engine, writer, clock, requester, painter, poller, handler }   fn execute(&self, session: &mut TerminalSession);   // order of pty.rs:1224-1269
pub(crate) struct ApplyTerminalBackgroundUseCase { screen, themes }              fn execute(&self, session: &mut TerminalSession, background: TerminalBackground);   // probe_background_reported; set_theme(resolve_theme(.., Some(rgb), ..))
pub(crate) struct EndSessionUseCase { screen, writer, engine, reaper, modes, session_contexts }   fn execute(&self, session: &TerminalSession);   // clear + SGR0 + show cursor in ONE write; shut_down; kill_live_children; modes.restore; remove(id)
pub(crate) struct PrepareSessionLaunchUseCase;                                   fn execute(&self, request: LaunchRequest) -> WrappedShellCommand;
pub(crate) struct LaunchRequest { pub(crate) shell: Option<String>, pub(crate) command: Vec<String>, pub(crate) environment: LaunchEnvironment }
pub(crate) struct LaunchEnvironment { pub(crate) tabsmith_shell: Option<String>, pub(crate) system_shell: Option<String>, pub(crate) login_shell: bool }
// application/session_ports_parts.rs — 11 fields, all pub(crate)
pub(crate) struct SessionPortsParts { engine: Rc<dyn CompletionEngine>, screen: Rc<dyn TerminalScreen>, writer: Rc<dyn TerminalWriter>, shell: Rc<dyn WrappedShell>,
    reaper: Rc<dyn GeneratorProcessReaper>, modes: Rc<dyn TerminalModes>, live_settings: Rc<dyn LiveSettings>, session_contexts: Rc<dyn SessionContextRepository>,
    themes: Rc<dyn ThemeRepository>, working_directory: Rc<dyn WorkingDirectory>, clock: Arc<dyn Clock + Send + Sync> }
// application/session_use_cases.rs
pub(crate) struct SessionUseCases { pub(crate) handle_keystrokes, process_shell_output, apply_completion_result, apply_llm_ghost_text, resize_terminal, handle_shell_exit, handle_timer_tick,
    apply_terminal_background, end_session; /* private Rc services with #[cfg(test)] accessors */ }   impl SessionUseCases { fn new(ports: SessionPortsParts) -> Self }
```

### Clock reads (HEAD site to new)

| HEAD (pty.rs) | What | New |
|---|---|---|
| 587, 599 | recv timeout, exit-grace check | runner: `clock.now_instant()` |
| 760 | extend a resize's quiet time | `ProcessShellOutput`: `output_arrived(now)` |
| 787 | redraw grace | `on_shell_event(.., now)` |
| 830, 845, 858 | lock check, grace check, `changed_at` | ONE read per `refresh` |
| 897, 899 | spinner arm, `menu.on_completion` | ONE read per `apply_completion` |
| 964 | held timed out | `release_held_keys(now)` |
| 1026 | late probe window | `late_probe_open(now)` |
| 1085 | `held_at` | `hold_keys(.., now)` |
| 1185 | insertion lock | `insert(.., now)` |
| 1212 | `resize_at` | `resize_applied(now)` |
| 1225 and 1254 | the tick's `now` (once) and `menu.loading(Instant::now())` (fresh) | `now` once; `tick_spinner(now, clock.now_instant())` |
| renderer 307 | `menu.loading(Instant::now())` | `FrameInput.now`, read LAST when the frame is built |
| 434-445, 552, 569, 675 | start-up, `changed_at`, `poll_at`, `exit_at` | launcher / `TerminalSessionParts.now` / `child_exited(.., now)` |

### Driver and module (9.4, 9.6)

The module block below is the FINAL shape: `prepare_session_launch` arrives at 9.5h, `live_settings()` at 9.4c (the interim `settings_repository()` getter of 9.4a dies there), `locate_data_root()` at 9.6c; the four collaborators of `build_terminal_session_module` are positional parameters (no struct).

```rust
// infrastructure/pty/session_message.rs
pub(crate) enum SessionMessage { Stdin(Vec<u8>), Pty(Vec<u8>), PtyEof, Resize(u16, u16) /* (cols, rows) positional */, ChildExit(i32), Term, Completion(Box<CompletionResult>), Ghost { req: u64, text: Option<String> } }
// infrastructure/pty/completion_engine_starter.rs
pub(crate) type CompletionEngineStarter = fn(Sender<SessionMessage>, Settings) -> Rc<dyn CompletionEngine>;
// infrastructure/terminal_session_module.rs
pub(crate) struct TerminalSessionModule { pub(crate) report_session_context: ReportSessionContextUseCase, pub(crate) prepare_session_launch: PrepareSessionLaunchUseCase, /* collaborators, data_directory, clock, session_contexts, live_settings */ }
pub(crate) fn build_terminal_session_module(dependencies: &SharedDependencies, settings: Rc<dyn SettingsRepository>, themes: Rc<dyn ThemeRepository>, generator_processes: Rc<dyn GeneratorProcessReaper>, start_completion_engine: CompletionEngineStarter) -> TerminalSessionModule;   // five parameters, positional like build_completion_module
impl TerminalSessionModule { fn locate_data_root(&self) -> Result<PathBuf, SharedError>; fn live_settings(&self) -> Rc<dyn LiveSettings>; fn themes(&self) -> Rc<dyn ThemeRepository>; fn generator_processes(&self) -> Rc<dyn GeneratorProcessReaper>;
    fn session_contexts(&self) -> Rc<dyn SessionContextRepository>; fn start_completion_engine(&self) -> CompletionEngineStarter; fn clock(&self) -> Arc<dyn Clock + Send + Sync> }
// bootstrap/cli.rs: fn terminal_session_module() -> TerminalSessionModule   (pub(crate): the debug-shell child test builds the same wiring)
// infrastructure/pty/pty_session.rs
pub(crate) fn run_pty_session(shell: Option<String>, cmd: Vec<String>, module: &TerminalSessionModule) -> i32;
fn attempt(run: impl FnOnce() -> anyhow::Result<i32>) -> anyhow::Result<i32>;   // catch_unwind(AssertUnwindSafe(run)): Ok(Ok(c))->Ok(c), Ok(Err(e))->Err(e), panic->Err("internal error (see pty.log)")
// infrastructure/pty/fallback_shell.rs
pub(crate) fn run_fallback_shell(error: &anyhow::Error, argv: &[String], reaper: &dyn GeneratorProcessReaper) -> i32;
// infrastructure/pty/session_launcher.rs
pub(crate) fn launch_session(command: &WrappedShellCommand, modes: &mut Option<Rc<dyn TerminalModes>>, module: &TerminalSessionModule) -> anyhow::Result<SessionRunner>;
fn generate_session_id() -> SessionId;
// infrastructure/pty/session_runner.rs
pub(crate) struct SessionRunnerParts { inbox: Receiver<SessionMessage>, credits: SyncSender<()>, pty: Rc<PtyWrappedShell>, session: TerminalSession, screen: Rc<dyn TerminalScreen>,
    output: Rc<dyn TerminalWriter>, use_cases: SessionUseCases, clock: Arc<dyn Clock + Send + Sync> }
pub(crate) struct SessionRunner { /* the same fields, in this order = the HEAD drop order */ }
impl SessionRunner { fn new(parts: SessionRunnerParts) -> Self; fn run(self) -> i32; fn step(&mut self, message: SessionMessage, stash: &mut Option<SessionMessage>) -> bool;
    fn guard(&mut self, handler: impl FnOnce(&mut SessionRunner)) -> bool; fn on_input(&mut self, chunk: &[u8]); fn next_deadline(&self) -> Option<Instant> }
// infrastructure/pty/terminal_probe.rs
pub(crate) const PROBE: &[u8]; pub(crate) const PROBE_TIMEOUT: Duration;
pub(crate) fn parse_probe(buf: &[u8]) -> (Option<(u16, u16)>, Option<Rgb>, Vec<u8>);                 // the TUPLE stays (locked `probe_replies`)
pub(crate) fn probe_terminal(output: &dyn TerminalWriter, timeout: Duration, read_timeout: impl FnMut(Duration) -> Vec<u8>) -> (Option<(u16, u16)>, Option<Rgb>, Vec<u8>);   // arrives at 9.6e with the launcher
// infrastructure/pty/pty_reader.rs
pub(crate) const PTY_CREDITS: usize = 4;  pub(crate) fn spawn_pty_reader(reader: impl Read + Send + 'static, replies: Sender<SessionMessage>) -> SyncSender<()>;  fn pump(reader: impl Read, credits: Receiver<()>, tx: Sender<SessionMessage>);
// infrastructure/cli/*
pub(crate) fn run_pty_command(shell: Option<String>, cmd: Vec<String>, module: &TerminalSessionModule) -> anyhow::Result<i32>;
pub(crate) fn run_key_tester_command() -> anyhow::Result<i32>;
pub(crate) fn run_debug_shell_command(shell: Option<Shell>, module: &TerminalSessionModule, render_post_script: impl FnOnce(Shell) -> anyhow::Result<String>) -> anyhow::Result<i32>;
```

`SessionRunner::step` maps the messages as the use-case area's table: `Stdin` (passthrough: `pty.write_input`; else `raw = !session.menu_intercepts()` BEFORE the guard, then `guard(|r| r.on_input(&b))`, forward raw after a panic only if `raw`), `Pty` (batching with `try_recv` and the stash,
`guard(process_shell_output)`; passthrough or a panic: `output.write(&batch)` raw; one credit per chunk AFTER), `Resize` (passthrough: `pty.resize` only; else `guard(resize_terminal)`), `Completion`/`Ghost` (dropped in passthrough; else `guard`), `ChildExit`
(`pty.release_pseudo_console()` then `handle_shell_exit`), `PtyEof` and `Term` (`handle_shell_exit`), and `run` calls `handle_timer_tick` under `guard` after EVERY message and timeout unless passthrough, then `end_session.execute(&session)` and exits with `session.exit_code()`.

## Deviations from the plan's letter (record them in plan section 9 at 9.7)

1. **Guards first.** A fixture helper, 17 characterisation scenarios (3 tracker, 14 session), 13 loop tests and the id-shape test, four scripts and the local tag `phase9-guards` precede every move (R1, R34, R35).
2. **Forty-two steps in six runs instead of seven plan steps.** Legacy files leave with their last item, not in step 7: `tracker.rs` and `types::Model` at 9.2f, `types::Msg` at 9.6a, `pty.rs` at 9.6d (R38). The plan's step order is not followed literally (R2).
3. **`terminal_session` is registered FIRST (9.0a)**, not at the end; no module-edge rule is added; the test-only `terminal_session -> specs/completion/bootstrap/settings/dropdown infrastructure` edges are allowed explicitly (D2), closing Phase 8 deviation 25 (R3).
4. **The `MarkerTracker` protocol**: plan section 4 says it "takes the cursor column in and returns ShellEvents + ScreenCommands"; built: `on_marker(marker, cursor_column, now_secs)`, a two-call `PreExec` (`EditBufferNeeded`), `complete_pre_exec`, `end_batch` (R5, R6).
5. **Four dead fields and the `ShellPath`/`TTY`/`User` markers are deleted at 9.1e**, earlier than plan step 2; `User` leaves the lock allow-list (R7). `docs/SPEC.md` §5.1 is stale until Phase 11.
6. **`Tracker::feed` becomes the application service `ShellOutputFeeder`**, which the plan does not list; four more services (`KeystrokeHandler`, `CompletionRequester`, `OverlayPainter`, `SessionFilePoller`) hold the logic shared by several events (R14, R20).
7. **`now_secs` is read per tracker call**, not per drained entry (R6): it can differ only when a wall-clock second ticks inside one marker.
8. **Use cases write their own stdout and PTY output and return `()`**; plan sections 4 and 7 ("`ProcessShellOutputUseCase` returns one byte buffer; the driver writes once") would reorder stdout against the PTY write of `release_held` (R21).
9. **`HandleShellExitUseCase` takes `Term` and returns the loop bool**; `EndSessionUseCase` runs once after the loop for every exit (R24).
10. **`ApplyTerminalBackgroundUseCase`** is new (the late OSC 11 theme moment; Phase 8 hand-off).
11. **Late probe replies are stripped by the runner's guarded handler**, not by the driver outside the guard; `LATE_PROBE` lives in the aggregate (the source map says `terminal_probe.rs`); `parse_probe` stays in infrastructure (R25).
12. **The `#[cfg(test)]` panic injection stays inside `KeystrokeHandler`'s per-key loop**, not at the driver's call site (R31).
13. **Ports beyond plan section 4**: `TerminalModes`, `LiveSettings`, `WorkingDirectory`; `TerminalModes` keeps the exact cleanup order (plan: "`modes.restore` stays in the driver") (R22).
14. **The `TerminalScreen` port has eleven methods** (plan: nine; `set_theme` and `pass_window_name_through` are added, `drop_clear` is bundled into `resize`); `FrameInput` moves to application and gains `now` (R10, R15).
15. **`KeyDecoder` is a domain entity** (`domain/entities/key_decoder.rs`), no longer the adapter `StdinKeyDecoder` (R17).
16. **The aggregate's fields are `pub(crate)` between 9.3g and 9.3i** (Phase 8 R3 precedent); none at the end (R16).
17. **Entities and value objects beyond section 4**: `GhostCandidates`, `ResizeDebounce`, `ShellExit`, `ReloadWatch`, `KeyFacts`, `CompletionQuery` (R16, R18, R42).
18. **Tracker value objects beyond section 4**: `EditBufferContext`, `TextEmphasis`, `PwshPredictionColor`, `CellTag`, `MarkerEffects`, `MarkerReaction` (R8, R9).
19. **The screen is built after the probe** (in-memory work moved: `Term::new`, `Tracker::new`, the CUP replay); every observable action keeps its order (R12).
20. **`SessionId::from_entropy`** is a domain constructor and the aggregate's id is required (R27).
21. **`PrepareSessionLaunchUseCase` resolves the shell** from a `LaunchRequest` (plan: argv, env, title); the env changes and the title are `WrappedShellCommand` calls made once the id exists (R26).
22. **One test harness in `application/test_fixtures.rs`** (the use-case area's choice over `infrastructure/testing/`); the runner's tests are built from it (R31).
23. **`overlay_renderer.rs`'s 551-line test module is sliced** to `overlay_renderer/tests.rs` at 9.2c (Phase 8 deviation 24 closed).
24. **Test restructurings**: D6 splits (`suggestion_colors`, `pwsh_reported_prediction_colour`), `child_argv` through `WrappedShellCommand::new`, the D9 string of `debug_shell_survives_closed_terminal`, accessor/fixture edits listed in "Test moves"; no assertion changes.
25. **`rgb_to_256` is not deduplicated** with `dropdown`'s `xterm256` (proven equal for all 2^24 inputs; edits two locked tests; Phase 11) (R40).
26. **Four scripts are committed and none joins the gate** (R35); Linux runs `BASELINE_CLIPPY=4` (R36).
27. **`DebugCmd` lives in `bootstrap/cli.rs`**; the debug shell gets its post script from a bootstrap callback; `debug_logs` becomes `pub fn` in `cli.rs` until Phase 10 (R33).
28. **`ProcessShellOutputUseCase::new` takes a `ProcessShellOutputParts`** (nine collaborators; clippy's seven-argument limit with the budget used up).
29. **The legacy `Tracker` composite lives from 9.1e to 9.2f**, and `Session` delegates from 9.5a to 9.5h: interim, not shims; no `pub(crate) use` ever (R4).
30. **Manual smoke** is asked once, after 9.7; nothing is pushed (R43).

## Test moves

"path" means testlock's loose form shows no change (it strips `a::b` paths, so a rename confined to a `Type::fn` path is PATH-ONLY; a renamed struct-literal or bare type is not a path, so it is CHANGED). "CHANGED" is a justified restructuring: the same scenario and the same assertions.
Find tests by name; lines are `5efb4b2` `fn` lines. Helpers outside `#[test]` fns are not locked.

### `tracker.rs` (38 tests)

| Test (HEAD line) | Destination | Step: edit | Kind |
|---|---|---|---|
| `splitter_chunk_boundaries` 1115, `splitter_bel_st_and_passthrough` 1135, `splitter_holds_a_window_name_whole_across_chunks` 1159, `splitter_ends_a_window_name_at_another_escape_like_tmux` 1172, `splitter_cancels_a_window_name_at_can_or_sub_like_tmux` 1187, `splitter_flushes_huge_window_name_to_the_terminal_only` 1195, `splitter_flushes_huge_osc` 1207 | `domain/entities/marker_splitter.rs` tests (helpers `split_all`, `bytes_of`, `markers_of` with them; `OSC_MAX` stays a private const reached by `use super::*`) | 9.1a: `Splitter` to `MarkerSplitter`, `Piece` to `OutputPiece` | path (or CHANGED by the bare type in a helper) |
| `suggestion_colors` 2038 | D6 split: the parse asserts and the `(16, 196, 244)` triple keep the name in `value_objects/shell_suggestion_color.rs`; the `Cell` asserts (2068-2082) are `suggestion_colors_cell_matching` | 9.1a (split; the Cell half stays in `tracker.rs`), 9.2b (the Cell half moves to `edit_buffer_extractor.rs`: `z.matches(&cell)` to `matches_cell(&z, &cell)`) | CHANGED |
| `pwsh_reported_prediction_colour` 1481 | D6 split: the extract half (1481-1511) keeps the name; the `parse_sgr` asserts (1511-1525) are `pwsh_reported_prediction_colour_sgr` in `value_objects/pwsh_prediction_color.rs` (`Flags::DIM \| Flags::ITALIC` to `TextEmphasis::new(true, true)`, `Flags::ITALIC` to `new(false, true)`, `Flags::empty()` to `default()`; same `Some`/`None` results) | 9.1a split; 9.1b `e.*` accessors; 9.2f the extract half moves | CHANGED |
| `extract_ghost_after_cursor_vs_typed_grey` 1416, `bash_redraw_of_the_prompt_alone_keeps_the_line` 1631, `at_prompt_only_on_the_command_line` 1682, `soft_wrap_redrawn_with_crlf_is_still_the_line` 1731, `prompt_ending_in_a_newline_is_at_the_prompt` 1759, `prompt_drawn_without_markers_is_at_the_prompt` 1808 | `alacritty_terminal_screen/tests.rs` | 9.1b: `e.buffer`/`.cursor`/`.shell_ghost`/`.at_prompt` field reads to the `EditBuffer` accessors; 9.2e fixture (`Model` to `AlacrittyTerminalScreen`, the `at` closures take the new pair); 9.2f the move | CHANGED |
| `markers_stripped_from_output` 1218 | same | 9.1e `t.cwd` to `t.marker.cwd()`; 9.2f `rig.tracker.cwd()` | CHANGED |
| `lock_and_foreign_session_ids` 1244 | same | 9.1e `t.osc_lock`, `t.cwd`, `t.cycle` to accessors (`osc_lock()` and `cycle()` are `#[cfg(test)]`; `cwd` compared as `Some(Path::new("/a"))`) | CHANGED |
| `bare_newcmd_cannot_fake_a_prompt_while_locked` 1285 | same | 9.1e `t.preexec`, `t.cycle` to accessors | CHANGED |
| `command_cycle_produces_history` 1305 | same | 9.1e `Event::NewCmd` to `ShellEvent::NewCommand`, `t.events` stays on the composite; 9.2f `rig.events` (accumulated, cleared with `.clear()` as before), `t.extract(&m)` to `rig.edit_buffer()` | CHANGED |
| `no_history_marker_drops_the_command` 1529 | same | 9.1e `Event`, `t.events`, `t.no_history` (`no_history()` is `#[cfg(test)]`); 9.2f `mem::take(&mut rig.events)` | CHANGED |
| `submitted_line_drops_shell_output_before_preexec` 1586, `transient_prompt_before_preexec_keeps_the_submitted_line` 1843 | same | 9.1e `t.submitted = Some(..)` to `set_submitted(Some(..))`, `t.submitted` to `submitted()`, `Event` to `ShellEvent`; events accumulate over the feeds as before | CHANGED |
| `nested_shell_prompt_keeps_outer_command_running` 1885 | same | 9.1e `t.pending.is_empty()` to `pending_is_empty()` (`#[cfg(test)]`), events | CHANGED |
| `pwsh_list_view_rows_not_in_buffer` 1967 | same | 9.1e `t.pwsh_list_view` to `pwsh_list_view()` | CHANGED |
| `windows_deferral_fifo` 2017 | same | 9.1e `t.defer = true` to the rig built with `defer_markers: true`, `t.deferred == [Deferred::EndPrompt, Deferred::NewCmd]` to `deferred() == &[DeferredMarker::EndPrompt, DeferredMarker::NewCmd]`, `t.cycle`, events | CHANGED |
| `window_name_reaches_the_terminal_but_is_not_printed` 1231 | same | 9.2e `m.grid().cursor.point.column.0` to `screen.cursor_column()` (same expression) | CHANGED |
| `extract_wrapped_long_command` 1385 | same | 9.2e `m.resize(TermSize::new(15, 5))` (model only) to `screen.resize(15, 5)` (the assert reads reflowed cells; the filter's rows do not feed `extract`) | CHANGED |
| `extract_prompt_scrolled_by_output` 1999 | same | 9.2e `m.grid().history_size()` to `screen.history_size()` (`#[cfg(test)]`, `for_tests`' 10000 lines) | CHANGED |
| `extract_zsh_prompt_and_command` 1350, `extract_ignores_rprompt` 1360, `extract_ps2_multiline` 1371, `extract_excludes_completion_listing_below` 1402, `extract_keeps_blank_skipped_before_cursor` 1464, `pwsh_continuation_and_prompt_text_stripped` 1934 | same | 9.2f: the fixture shape only, through the new `Rig` (`let (mut t, mut m) = setup(..)` becomes `let mut rig = setup(..)`, `feed(&mut t, &mut m, s)` becomes `feed(&mut rig, s)`, `buf(&t, &m)` becomes `buf(&rig)`); same scenario, same assertions | CHANGED |
| `output_filter_strips_keyboard_upgrades` 2100, `output_filter_tracks_dcs_and_apc_strings` 2145, `output_filter_passes_overlong_sequence_through` 2168, `output_filter_tracks_scroll_region_and_utf8` 2188 | `adapters/output_filter.rs` tests (the `filter` helper with them; the private `carry` and `SEQ_MAX` stay reachable; `f.lf_to_ind = true` unchanged) | 9.2a: `OutFilter` to `OutputFilter` | path |

### `pty.rs` (36 tests)

| Test (HEAD line) | Destination | Step: edit | Kind |
|---|---|---|---|
| `insertion_bytes_reconcile` 1621, `expected_buffer_after_insertion` 1640, `unechoed_keys_are_consumed_by_their_echo` 1707 | `domain/services/insertion_reconciler.rs` (the `ins` helper with them) | 9.3a | path |
| `ghost_keys_and_words_keys` 1695 | `domain/services/key_router.rs` (`ghost_key` private in the same file) | 9.3a | path |
| `child_argv` 1676 | `domain/value_objects/wrapped_shell_command.rs` | 9.3b: `argv(a, b, c)` to `WrappedShellCommand::new(a, b, c).argv().to_vec()` through a local closure; same inputs and expected vectors | CHANGED |
| `probe_replies` 1657 | `infrastructure/pty/terminal_probe.rs` | 9.6a | path |
| `pty_reader_waits_for_credits` 1729 | `infrastructure/pty/pty_reader.rs` | 9.6a: the bare `Result<Msg, ..>` and `Msg::Pty/PtyEof` to `SessionMessage` | CHANGED |
| `signal_death_exit_code` 1754 (unix) | `infrastructure/pty/platform_unix.rs` | 9.6b: `sys::wait_pid` to `wait_pid` | path |
| `identical_redraw_writes_nothing` 2024 | `application/overlay_painter.rs` tests | 9.4b `s.sink` reads to the capturing writer; 9.5e moves (`h.written()`, `h.redraw()`) | CHANGED |
| `split_paste_is_never_intercepted` 1888, `menu_hidden_by_render_guards_does_not_intercept` 1899, `disabled_menu_lets_tab_through` 1913, `insertions_build_on_keys_not_yet_echoed` 1935, `unbracketed_paste_is_not_intercepted` 2174, `enter_records_the_submitted_line` 2243, `enter_clears_the_menu_before_the_line_runs` 2279, `pwsh_list_view_owns_the_rows_below` 2355 | `application/handle_keystrokes_use_case.rs` tests | 9.5e moves them; on the way: 9.1e (`s.tracker.submitted`, `pwsh_list_view` to the tracker's accessors and setters), 9.3c (`s.unechoed`), 9.3g (`s.menu`, `s.settings`, `s.req`, `s.drawable()` to the aggregate accessors), 9.4b (`sent(&pty)` reads the capturing shell); at the move: `h.session.menu_intercepts()`, `h.session.menu().has_items()`, `h.session.tracker().submitted()`, `h.session.tracker_mut().set_submitted(..)`, `h.session.run_menu_action(..)`, `h.session.insert(&ins, Instant::now())`, the engine swap to `h.engine_calls()` (filtered to `CommandFinished`) | CHANGED |
| `llm_ghost_requested_for_commands_without_spec` 1974 | `application/apply_completion_result_use_case.rs` tests | 9.3d (`s.llm_at` to `llm_due_at()`/`cancel_llm_schedule()`); 9.5e moves | CHANGED |
| `should_show_the_llm_ghost_text_of_the_current_request_behind_the_local_one` 1988 | `application/apply_llm_ghost_text_use_case.rs` tests | 9.3d (`s.llm_req = s.req` to `expect_llm_reply(request_id())`); 9.5e: `s.step(Msg::Ghost ..)` to `h.llm_reply(req, text)` | CHANGED |
| `prompt_redraw_keeps_the_menu` 2038, `no_frame_inside_an_open_dcs` 2053, `prompt_redraw_does_not_reshow_a_hidden_menu` 2118, `no_history_command_still_ends` 2301, `inline_tui_below_the_prompt_is_not_the_line` 2321 | `application/process_shell_output_use_case.rs` tests | 9.5e moves them (`s.sink` to `h.written()`/`h.clear_written()`, `s.req` to `request_id()`, `(s.req, s.cur.clone())` to `(request_id(), edit_buffer().cloned())`, `s.sent.as_ref().map(..)` to `sent().map(..)`, the engine swap to `h.engine_calls()`) | CHANGED |
| `split_prompt_redraw_keeps_the_menu` 2154 | `application/process_shell_output_use_case.rs` tests | 9.5g moves (needs `tick()`); `s.redraw_until = Some(Instant::now())` to `set_redraw_until(Some(Instant::now()))` (`#[cfg(test)]`); real clock | CHANGED |
| `keys_after_an_unmodelled_key_wait_for_its_echo` 2185 | `application/handle_keystrokes_use_case.rs` tests | 9.5g moves; `s.held_at = Some(Instant::now())` to `expire_hold_at(Instant::now())`; real clock | CHANGED |
| `slow_echo_keeps_the_insertion_in_the_basis` 2217 | `application/handle_timer_tick_use_case.rs` tests | 9.5g moves; the first poke changes ONLY the deadline (keeps the cap: re-armed, not released) so it is `expire_insertion_deadline(now)`; `(l.0, l.1) = (now, now)` is `expire_insertion_lock(now, now)`; `s.lock.is_some()/is_none()` to `has_insertion_lock()`; real clock | CHANGED |
| the 5 context tests: `should_send_the_engine_the_env_and_alias_of_a_saved_session_context` 2421, `should_not_send_the_session_context_again_while_its_file_keeps_its_modification_time` 2438, `should_send_nothing_and_keep_the_modification_time_once_the_context_file_is_deleted` 2452, `should_send_nothing_for_a_context_file_that_is_not_json_but_keep_its_new_modification_time` 2470, `should_send_the_session_context_again_once_its_file_is_rewritten_with_a_new_modification_time` 2487 | `application/session_file_poller.rs` tests (helpers `session_reading_context_in`, `write_context_file`, `contexts_sent`, `at` private there) | 9.3e (`s.session_mtime` to `context_modified()`), 9.5b moves: `s.poll_files()` to `poller.poll(&mut h.session)`; the harness `with_session_contexts(SessionId "abc", Rc<FileSessionContextRepository>)` | CHANGED |
| `late_probe_reply_is_not_typed` 2011 | `infrastructure/pty/session_runner/tests.rs` | 9.3g/9.3i: `s.probe_until = Some(now + LATE_PROBE)` to `expect_late_probe_reply(Instant::now())`, `(s.terminal_bg, s.probe_until)` to `(terminal_background().map(TerminalBackground::rgb), late_probe_until())`; 9.6f moves | CHANGED |
| `panic_turns_the_session_into_a_pass_through` 2071 | same | 9.4b `s.sink` to the capturing writer, 9.6a `Msg` to `SessionMessage`, 9.3e/9.3i `s.exit_code` to `reported_exit_code()`, `s.passthrough` to `is_passthrough()`; asserts `(Some(3), true)` unchanged; 9.6f moves | CHANGED |
| `panic_while_handling_keys_still_forwards_them` 2093 | same | 9.5c `PANIC_IN_INPUT.with(..)` to `test_fixtures::set_panic_in_key_handling(..)`; `s.passthrough` to `is_passthrough()`; 9.6f moves | CHANGED |
| `conpty_is_closed_off_the_main_loop` 2508 (windows) | same | 9.4b: `s.master = Some(Box::new(Master(tx)))` to a `PtyWrappedShell` over the fake master; 9.6f moves; compiled only by the windows check | CHANGED |

### `cli.rs` and the other files touched

| Test | Destination | Step: edit | Kind |
|---|---|---|---|
| `key_tester_line` C:327 | `infrastructure/cli/key_tester_command.rs` | 9.6c | path |
| `debug_shell_rc_dir_and_quoting` C:361 | `infrastructure/cli/debug_shell_command.rs` (`use super::*`) | 9.6c | unchanged |
| `debug_shell_survives_closed_terminal` C:424 (unix) | same | 9.6c: `NAME` (D9); the child branch calls `run_debug_shell_command(Some(Shell::Bash), &crate::bootstrap::cli::terminal_session_module(), crate::bootstrap::cli::debug_shell_post_script)`; the env contract (`TABSMITH_HOME`, `TABSMITH_INIT_SNAPSHOT_TEST`, `TABSMITH_TEST_DEBUG_SHELL`) is unchanged | CHANGED |
| `diagnostic_toml` C:336 | stays in `cli.rs` (Phase 10) | none | unchanged |
| `decode_table` 232, `decode_multi_key_chunk` 300, `esc_not_last_and_double_esc` 316, `paste_across_chunks` 337, `keymap_defaults_and_overrides` 365 (`adapters/stdin_key_decoder.rs`) | `domain/entities/key_decoder.rs` tests (the `decode` free fn stays `#[cfg(test)]`; helpers `NONE/CTRL/ALT/SHIFT/k/one/settings` with them) | 9.3b: `StdinKeyDecoder` to `KeyDecoder` | path / CHANGED by the bare type |
| the overlay renderer tests (`overlay_renderer.rs` inline) | `overlay_renderer/tests.rs` | 9.2c slice (pure); `input()` helper gains `now`; the 10 hash scenarios and `round_trip_arg_hint_spinner_and_wide_ghost` keep their bodies and values | unchanged |
| `should_report_the_context_into_the_run_files_of_the_data_directory_it_was_built_with` (`terminal_session_module.rs`) | same file | 9.4a: `build_terminal_session_module(&deps, collaborators)` with in-memory collaborators | CHANGED |
| `bootstrap/engine_server/tests.rs` helpers (`run_engine`, `serve`, `serve_ghosts`) and the six `Msg::Completion/Ghost` sites | same | 9.6a: `Msg` to `SessionMessage` | path (helpers are not locked) |

Expected over the phase against `phase9-start` (each a restructuring justified in the tables above): CHANGED = the 38 tracker tests that are not pure moves (the 7 splitter tests are path-only or CHANGED by a bare type in a helper; `suggestion_colors`,
`pwsh_reported_prediction_colour`, the 6 other `EditBuffer` field readers, the 10 tracker-state readers, `window_name_reaches_the_terminal_but_is_not_printed`, `extract_wrapped_long_command`, `extract_prompt_scrolled_by_output`), `child_argv`, `pty_reader_waits_for_credits`,
all 28 `Session` tests (fixture and accessor edits; four of them move only at 9.6f), `debug_shell_survives_closed_terminal`, the module test of `terminal_session_module.rs` and the decoder tests whose bodies name the bare type; PATH-ONLY = `insertion_bytes_reconcile`,
`expected_buffer_after_insertion`, `unechoed_keys_are_consumed_by_their_echo`, `ghost_keys_and_words_keys`, `probe_replies`, `signal_death_exit_code`, `key_tester_line`, the four `output_filter_*` tests and the tests that only change a path. `MISSING` 0 and `DUPLICATE` 0 throughout.

### New tests (xp-tdd, `should_` names, AAA, no mocks; `grep -rn "fn <name>(" src` empty first). 321 NEW names (the four timer tests of 9.3i carry suggested names that the step greps first); the Linux pass count grows from 1091 to 1411 (one `#[cfg(windows)]` name of 9.5h does not run there). Moved tests keep their names, so no name leaves.

| Step | File | Tests |
|---|---|---|
| 9.0b | `shared/infrastructure/testing/fixture_log.rs` | `should_fail_a_missing_fixture_and_say_how_to_create_it`, `should_create_a_missing_fixture_only_when_asked_and_still_fail`, `should_show_the_difference_and_never_rewrite_an_existing_fixture` (the helper's inner `check(root, name, actual, create: bool)` is tested without touching the environment) |
| 9.0b | `tracker.rs` | `should_filter_and_track_a_bash_cycle_exactly_as_at_the_phase_9_start`, `should_defer_windows_markers_in_order_exactly_as_at_the_phase_9_start`, `should_track_zsh_colours_and_a_nested_shell_exactly_as_at_the_phase_9_start` |
| 9.0c | `pty.rs` (suffix `_exactly_as_at_the_phase_9_start`) | `should_draw_and_insert_when_typing_with_a_menu`, `should_ring_the_bell_before_the_frame_and_write_the_line_last`, `should_release_held_keys_after_the_frame`, `should_report_a_command_cycle_to_the_engine`, `should_clear_the_overlay_and_shut_down_the_engine_at_the_end`, `should_accept_the_ghost_whole_and_by_word`, `should_keep_an_insertion_in_the_basis_until_it_is_echoed`, `should_repaint_once_the_resize_settled`, `should_debounce_and_show_the_llm_ghost_text`, `should_reload_the_settings_and_the_theme_from_the_file`, `should_theme_the_late_probe_reply_and_not_type_it`, `should_pass_everything_through_after_a_panic`, `should_serve_the_spinner_after_two_hundred_milliseconds`, `should_merge_queued_chunks_into_one_batch` |
| 9.0d | `pty.rs` | `should_write_every_queued_pty_chunk_to_stdout_as_one_batch_when_passing_through`, `should_stash_the_first_message_that_is_not_pty_output_behind_the_batch`, `should_return_one_credit_per_pty_chunk_after_handling_the_batch`, `should_keep_running_at_pty_eof_until_the_child_has_exited`, `should_leave_the_loop_at_pty_eof_once_the_child_has_exited`, `should_leave_the_loop_when_the_child_exits_after_the_output_ended`, `should_give_the_output_two_hundred_milliseconds_after_the_child_exits`, `should_exit_with_129_when_terminated`, `should_ignore_engine_replies_while_passing_through`, `should_resize_only_the_pty_while_passing_through` (a fake `MasterPty` recording sizes), `should_run_the_timers_after_every_message_outside_passthrough`, `should_emit_the_cached_clear_then_the_raw_batch_and_return_the_credits_when_the_refresh_panics`, `should_write_the_batch_again_raw_when_release_held_panics_after_the_frame`, `should_generate_a_session_id_of_32_lowercase_hex_digits` |
| 9.1a | `text_emphasis.rs`, `shell_suggestion_color.rs`, `pwsh_prediction_color.rs` | `should_be_within_an_emphasis_that_has_every_attribute_and_an_empty_one_within_any`, `should_tell_dim_or_italic`, `should_match_any_listed_colour_and_every_colour_when_a_list_is_empty`, `should_require_every_reported_emphasis_besides_the_colours`; the split names `suggestion_colors_cell_matching`, `pwsh_reported_prediction_colour_sgr` |
| 9.1b | `edit_buffer.rs`, `cell_tag.rs`, `marker_effects.rs` | `should_expose_what_it_was_built_with`, `should_encode_the_prompt_tag_and_a_cycle_tag_as_the_uris_the_cells_carry`, `should_keep_the_screen_commands_and_the_events_in_the_order_they_were_pushed` |
| 9.1c | `shell_marker.rs` | `should_read_a_bare_token_as_a_marker_without_a_value`, `should_split_the_payload_at_the_first_equals_sign`, `should_ignore_a_marker_with_an_empty_value`, `should_ignore_an_unknown_key_and_the_keys_that_set_nothing_the_wrapper_reads`, `should_keep_the_session_id_of_new_cmd_untrimmed_and_none_when_bare`, `should_trim_the_session_id_of_osc_lock_and_unlock`, `should_keep_the_dir_untrimmed_and_read_a_bare_dir_as_the_empty_path`, `should_read_an_unparsable_exit_code_or_pid_as_none`, `should_trim_the_shell_name_and_read_a_bare_shell_as_empty`, `should_switch_the_list_view_on_only_for_one`, `should_split_pwsh_prompts_untrimmed_on_the_unit_separator_and_drop_empty_texts`, `should_read_the_suggestion_colours_and_none_when_they_do_not_parse`, `should_let_only_the_five_lock_exempt_markers_through_the_integration_lock` |
| 9.1d | `session_id.rs`, `marker_tracker.rs` | `should_format_the_nanoseconds_and_the_mixed_pid_as_two_zero_padded_hex_words` (literal `0123456789abcdef8acf13579bde026c`), `should_generate_an_id_the_validation_accepts`; `should_open_a_cycle_and_mark_the_line_start_on_new_cmd`, `should_mark_a_redraw_with_the_cycle_that_was_open`, `should_ignore_a_new_cmd_of_another_session_or_without_one`, `should_defer_end_prompt_and_new_cmd_and_apply_them_in_order_when_a_batch_has_no_marker`, `should_count_an_ignored_marker_as_a_marker_of_the_batch`, `should_flush_the_deferred_markers_before_asking_for_the_edit_buffer_on_pre_exec`, `should_ignore_pre_exec_and_no_history_while_a_command_runs`, `should_let_only_the_lock_exempt_markers_through_while_locked`, `should_record_the_scraped_command_with_the_shell_pid_cwd_and_time_it_started`, `should_finish_the_innermost_command_first_with_the_exit_code_of_the_outer_prompt`, `should_record_a_blank_command_for_no_history_and_end_it_without_recording`, `should_prefer_the_submitted_line_when_the_scrape_ends_with_shell_output_after_it`, `should_keep_the_submitted_line_through_a_redraw_prompt_that_finishes_nothing`, `should_replace_a_known_shell_by_none_and_keep_the_raw_name`, `should_have_no_edit_buffer_context_before_the_first_cycle_or_while_a_command_runs`, `should_reset_the_list_view_when_a_command_starts`, `should_clear_the_tag_and_report_pre_exec_when_the_command_starts`, `should_tag_the_prompt_on_start_prompt`, `should_end_the_prompt_with_the_open_cycle_tag_or_clear_it_when_none_is_open`, `should_ignore_a_pre_exec_whose_new_cmd_is_still_queued`, `should_not_flush_a_queued_new_cmd_for_no_history_while_a_command_runs`, `should_flush_the_deferred_markers_before_no_history`, `should_apply_the_deferred_markers_even_while_the_lock_is_on`, `should_consume_the_exit_code_when_a_prompt_finishes_nothing`, `should_match_a_pending_command_without_pid_when_no_pid_was_reported`, `should_take_the_scrape_when_the_submitted_line_is_not_in_it`, `should_reset_no_history_on_new_cmd`, `should_use_the_default_pwsh_continuation_until_the_prompt_reports_it`, `should_overwrite_a_colour_with_none_when_the_new_one_does_not_parse`, `should_report_the_context_of_the_open_cycle_with_the_reported_pwsh_prompts` |
| 9.2a | `cell_tagger.rs` | `should_tag_the_cells_written_after_a_tag_command`, `should_stop_tagging_after_a_clear_tag_command`, `should_turn_the_untagged_cells_before_the_cursor_into_the_prompt_on_mark_line_start`, `should_retag_the_redrawn_cycle_cells_from_the_cursor_on_mark_line_start`, `should_follow_wrapped_rows_only_when_a_redrawn_cycle_is_given` |
| 9.2b | `edit_buffer_extractor.rs` (hand-built `EditBufferContext`s) | `should_treat_a_cell_with_only_one_of_the_required_flags_as_typed_text`, `should_treat_a_dim_or_italic_cell_as_the_shell_suggestion_without_a_reported_prediction_colour`, `should_map_named_colours_above_fifteen_to_the_default_colour`, `should_match_indexed_and_truecolour_cells_like_the_parsed_colour`, `should_count_the_pending_wrap_cursor_as_past_the_last_column` |
| 9.2c | `overlay_renderer/tests.rs` | `should_show_the_spinner_only_after_the_menu_has_been_loading_for_more_than_200_ms_at_the_given_instant` (`on_completion(c, &s, t0)`; a frame at `t0 + 199 ms` has no glyph, at `t0 + 201 ms` `SPINNER[spinner % 10]`) |
| 9.2d | `screen_state.rs`, `alacritty_terminal_screen/tests.rs` | `should_report_the_size_as_columns_then_lines`; `should_append_the_filtered_bytes_and_advance_the_model_on_pass_through`, `should_hold_an_incomplete_utf8_tail_until_the_next_pass_through`, `should_leave_the_model_alone_for_a_window_name`, `should_end_a_synchronized_update_after_each_pass_through_piece`, `should_keep_one_parser_across_pass_through_calls`, `should_report_the_alternate_screen_of_the_model`, `should_report_origin_and_insert_mode`, `should_report_a_pending_wrap_after_the_last_column`, `should_report_a_partial_scroll_region_and_forget_it_on_resize`, `should_report_an_open_string_until_it_is_terminated`, `should_report_the_size_of_the_model_and_follow_a_resize`, `should_report_the_cursor_column_and_not_move_it_when_commands_apply`, `should_keep_two_thousand_lines_of_scrollback_in_production`, `should_forget_the_cached_clear_on_resize`, `should_return_the_clear_once_and_empty_after`, `should_skip_an_identical_frame_and_draw_it_again_after_set_theme_even_with_the_same_theme`, `should_repaint_the_whole_model_on_full_repaint`, `should_survive_a_panic_in_the_overlay_and_still_hand_out_the_clear` |
| 9.2f | `shell_output_feeder.rs` (over the real screen) | `should_apply_the_screen_commands_of_a_marker_before_advancing_the_next_bytes`, `should_send_a_window_name_to_the_terminal_only`, `should_read_the_cursor_column_just_before_each_marker`, `should_read_the_clock_before_each_marker`, `should_flush_the_deferred_markers_when_a_batch_has_no_marker`, `should_apply_the_flush_before_reading_the_edit_buffer_on_pre_exec`, `should_complete_pre_exec_with_the_edit_buffer_the_screen_returned`, `should_return_the_events_of_a_batch_in_emission_order`, `should_end_the_batch_even_when_it_has_no_piece` |
| 9.3a | `render_guard.rs`, `session_deadlines.rs`, `overlay_renderer/tests.rs` | `should_allow_drawing_on_a_plain_80_by_24_screen`, `should_not_allow_drawing_with_a_scroll_region`, `should_not_allow_drawing_on_the_alternate_screen`, `should_not_allow_drawing_in_origin_mode`, `should_not_allow_drawing_in_insert_mode`, `should_not_allow_drawing_with_a_pending_wrap`, `should_need_ten_columns_to_draw`, `should_need_three_lines_to_draw`; `should_wake_for_the_earliest_pending_deadline`, `should_look_only_at_the_exit_deadline_in_a_passthrough`; the parity test `should_draw_a_frame_exactly_when_the_render_guard_allows_drawing` (a separate `test(terminal_session)` commit) |
| 9.3b | `wrapped_shell_command.rs` | `should_set_the_version_tmux_marker_and_session_id_and_remove_the_shell_choice_variables`, `should_not_mark_tmux_outside_tmux`, `should_start_nothing_without_a_shell_or_a_command`, `should_title_the_window_with_the_program_name_after_the_last_separator`, `should_title_the_window_with_the_whole_program_when_it_has_no_separator` |
| 9.3c | `insertion_lock.rs`, `echo_hold.rs` | `should_hold_until_the_deadline_while_the_shell_shows_something_else`, `should_release_once_the_shell_shows_the_expected_buffer`, `should_rearm_when_nothing_was_echoed_after_the_deadline_and_before_the_cap`, `should_release_past_the_cap_even_when_nothing_was_echoed`, `should_release_after_the_deadline_when_the_shell_did_something_else`, `should_release_after_the_deadline_when_there_is_no_buffer`; `should_keep_the_typed_ahead_keys_the_shell_has_not_echoed`, `should_forget_the_prediction_and_wait_for_the_reply_after_an_unmodelled_key`, `should_stop_waiting_only_for_a_reply_to_a_later_request`, `should_not_release_held_keys_while_the_reply_is_pending_and_the_hold_is_young`, `should_release_held_keys_once_the_hold_timed_out_even_without_a_reply`, `should_release_held_keys_when_the_reply_came`, `should_keep_the_held_keys_in_their_order`, `should_leave_held_keys_alone_when_a_command_starts` |
| 9.3d | `ghost_candidates.rs` | `should_prefer_the_local_ghost_text_over_the_llm_one_of_the_current_request`, `should_ignore_an_llm_reply_for_another_request`, `should_clear_the_llm_ghost_text_for_an_empty_reply`, `should_keep_the_llm_debounce_when_a_ghost_text_is_accepted_but_not_when_a_command_starts`, `should_spend_the_deadline_when_the_llm_request_is_taken` |
| 9.3e | `resize_debounce.rs`, `shell_exit.rs`, `reload_watch.rs` | `should_repaint_once_after_the_quiet_spell`, `should_extend_the_quiet_spell_while_output_arrives`, `should_not_start_a_quiet_spell_for_output_without_a_resize`, `should_postpone_the_repaint_on_the_alternate_screen`, `should_ignore_a_resize_to_zero_or_to_the_size_the_model_has`; `should_wait_the_grace_for_the_rest_of_the_output_after_the_child_exited`, `should_leave_at_once_when_the_output_had_already_ended`, `should_go_on_after_the_output_ended_until_the_child_exits`, `should_exit_with_129_when_terminated_and_with_zero_when_nothing_was_reported`; `should_report_a_context_file_only_when_its_time_changes`, `should_not_report_a_deleted_context_file_and_keep_its_last_time`, `should_report_deleted_settings_as_a_change`, `should_poll_again_after_the_interval` |
| 9.3f | `key_router.rs` | `should_hand_a_bound_key_to_the_menu_while_it_intercepts`, `should_forward_a_bound_key_once_a_render_guard_hides_the_menu`, `should_forward_every_key_off_the_command_line`, `should_swallow_a_key_bound_to_an_unknown_action_while_the_menu_intercepts`, `should_forward_a_key_bound_to_ignore`, `should_show_a_hidden_menu_with_items_on_tab_when_only_tab_shows_it`, `should_let_tab_through_a_disabled_menu`, `should_run_show_autocomplete_on_a_hidden_menu_with_items_but_not_other_actions`, `should_accept_the_whole_ghost_text_on_right_and_its_next_word_on_ctrl_right`, `should_not_accept_a_ghost_text_the_render_guards_hide`, `should_hold_a_key_that_would_act_while_an_unmodelled_keys_echo_is_pending`, `should_hold_a_ghost_key_even_when_the_menu_would_take_it`, `should_not_hold_a_key_that_does_not_act`, `should_count_only_unmodified_chars_as_typed_and_enter_ctrl_c_ctrl_d_as_ending_the_line` |
| 9.3g-9.3i | `terminal_session.rs` | 9.3g: `should_prefer_the_shell_the_tracker_saw_over_the_one_the_wrapper_launched`, `should_open_the_late_probe_window_until_a_cursor_report_closes_it`, `should_wake_up_for_the_nearest_deadline_and_only_for_the_exit_in_a_passthrough`, `should_wake_for_each_source_of_a_deadline_on_its_own`; 9.3h: `should_ask_for_a_changed_line_with_the_next_request_id`, `should_not_ask_again_for_an_unchanged_line`, `should_count_a_blank_line_as_a_request_but_ask_nothing_and_reset_the_menu`, `should_see_no_command_line_on_the_alternate_screen`, `should_see_no_command_line_when_the_cursor_is_off_it`, `should_keep_an_insertion_in_the_basis_while_the_shell_echoes_nothing_and_release_it_at_the_cap`, `should_not_act_on_an_empty_line_before_the_prompt_redraw_grace_is_over`, `should_keep_the_menu_and_the_unchanged_line_over_a_prompt_redraw`, `should_forget_what_was_sent_when_a_command_ends_so_the_same_line_is_asked_again`, `should_not_poll_the_files_for_a_command_that_starts`, `should_be_drawable_on_a_plain_command_line_and_not_when_a_guard_holds`, `should_hold_the_overlay_back_over_a_pwsh_list_view_only_for_a_non_empty_line`, `should_schedule_the_llm_request_for_an_error_reply_with_the_cursor_at_the_end_and_not_mid_word`, `should_drop_a_reply_for_an_older_request`, `should_draw_the_llm_ghost_text_of_the_current_request_behind_the_local_one`; 9.3i: `should_hold_enter_behind_an_unmodelled_backspace_until_its_reply_came` and one test per timer method, named by that step (suggested: `should_tick_the_spinner_only_while_a_reply_is_loading`, `should_take_the_llm_request_once_its_debounce_ran_out`, `should_poll_the_files_once_the_interval_passed_and_arm_the_next_poll`, `should_report_the_exit_once_the_child_exited_and_the_output_had_ended`) |
| 9.4a | `generator_process_reaper.rs` | `should_count_each_kill_of_the_live_children` |
| 9.4b | `terminal_writer.rs`, `wrapped_shell.rs`, `pty_wrapped_shell.rs`, `test_fixtures.rs` | `should_capture_the_bytes_written_in_the_order_they_were_written`, `should_forget_what_was_captured_once_cleared`, `should_note_each_write_in_the_shared_journal`; `should_hand_the_input_over_in_the_order_it_was_written_and_only_once`, `should_record_a_resize_as_columns_then_rows`, `should_interleave_stdout_and_shell_input_in_one_journal`; `should_send_the_input_bytes_to_the_writer_in_order`, `should_ignore_input_once_the_writer_is_gone`, `should_resize_the_master_to_the_given_columns_and_rows`, `should_keep_the_master_open_unless_on_windows` |
| 9.4c | `terminal_modes.rs`, `live_settings.rs`, `lenient_live_settings.rs`, `working_directory.rs`, `process_working_directory.rs` | `should_count_each_restore`; `should_load_the_settings_it_holds_and_report_their_time`, `should_load_the_defaults_and_no_time_once_removed`; `should_load_what_the_repository_saved`, `should_use_the_defaults_and_warn_when_the_settings_cannot_be_loaded`, `should_word_the_session_settings_warning_like_the_load_or_defaults_one`, `should_report_the_modification_time_of_the_repository`; `should_report_the_directory_it_was_given`, `should_report_none_when_it_has_none`; `should_report_the_process_current_directory` |
| 9.5a | `overlay_painter.rs` | `should_write_the_frame_the_screen_draws`, `should_draw_the_frame_at_the_instant_the_clock_reads_when_it_is_built` (FixedClock, `advance(300 ms)` on a loading menu: pins `FrameInput.now`), `should_write_nothing_for_a_frame_identical_to_the_last_one`, `should_repaint_the_model_once_after_a_resize_settled`, `should_not_repaint_on_the_alternate_screen` |
| 9.5b | `completion_requester.rs`, `session_file_poller.rs` | `should_send_the_changed_line_with_the_tracked_directory_and_shell`, `should_fall_back_to_the_process_directory_when_the_shell_reported_none`, `should_not_ask_the_directory_when_the_shell_reported_one`, `should_send_nothing_for_an_unchanged_line`, `should_reset_the_menu_for_a_blank_line_without_asking_the_engine`, `should_ask_in_history_mode_when_the_menu_is_in_it`, `should_read_no_command_line_on_the_alternate_screen`; `should_reload_the_settings_when_their_file_changed_and_tell_the_engine_last`, `should_load_the_defaults_once_the_settings_file_is_gone`, `should_set_the_theme_even_when_the_theme_did_not_change`, `should_rebuild_the_key_bindings_from_the_reloaded_settings`, `should_tell_the_engine_the_session_context_before_the_settings`, `should_not_reload_settings_while_their_time_is_unchanged` |
| 9.5c | `handle_keystrokes_use_case.rs` (arranged through the aggregate and the feeder) | `should_forward_keys_untouched_when_there_is_no_command_line`, `should_write_to_the_shell_only_after_the_frame_was_drawn`, `should_ring_the_bell_before_the_frame_and_the_shell_write`, `should_ask_the_engine_again_before_redrawing_when_the_history_mode_toggles`, `should_toggle_fuzzy_search_in_the_engine_then_ask_again`, `should_record_the_insertion_in_the_engine_after_the_lock_is_set`, `should_record_nothing_when_the_insertion_is_dropped_while_a_command_runs`, `should_not_decode_a_read_that_arrives_behind_held_keys`, `should_decode_the_replayed_held_keys_again`, `should_keep_a_split_paste_opaque_after_a_hold_reset_the_decoder`, `should_hold_a_key_that_would_act_while_the_echo_is_pending`, `should_ghost_accept_a_next_word_with_alt_f_and_the_whole_ghost_with_right`, `should_set_the_submitted_line_for_execute_only_at_the_prompt`, `should_write_nothing_to_the_shell_when_every_key_is_swallowed` |
| 9.5d | `process_shell_output_use_case.rs` | `should_write_the_batch_in_one_write_and_release_held_keys_after_it`, `should_send_command_finished_before_the_session_context_and_settings`, `should_record_the_finished_command_once`, `should_reset_the_menu_when_the_alternate_screen_ends`, `should_extend_a_resize_quiet_time_by_output`, `should_take_the_clear_after_the_refresh`, `should_write_the_repaint_before_the_frame_in_the_same_write` |
| 9.5e | `apply_completion_result_use_case.rs`, `apply_llm_ghost_text_use_case.rs`, `session_use_cases.rs` | `should_drop_a_reply_for_an_older_request_without_drawing`, `should_arm_the_llm_debounce_from_the_time_the_line_changed` (FixedClock), `should_arm_the_spinner_only_once`, `should_keep_the_local_ghost_of_a_generators_reply_without_one`, `should_clear_the_echo_wait_only_for_a_later_request`, `should_redraw_then_release_the_held_keys`; `should_not_release_held_keys_for_an_llm_reply`, `should_draw_nothing_for_an_llm_reply_to_another_request`; `should_share_one_painter_between_the_use_cases` |
| 9.5f | `resize_terminal_use_case.rs`, `handle_shell_exit_use_case.rs` | `should_resize_the_shell_before_the_screen`, `should_pass_columns_before_rows_to_the_shell`, `should_ignore_a_size_of_zero`, `should_ignore_the_size_the_model_already_has`, `should_repaint_after_the_quiet_time` (FixedClock + tick), `should_forget_the_cached_clear_when_resized`; `should_go_on_for_the_rest_of_the_output_after_the_child_exited`, `should_stop_the_loop_at_once_when_the_output_had_already_ended`, `should_keep_the_loop_going_after_the_output_ended_until_the_child_exits`, `should_stop_the_loop_with_exit_code_129_when_terminated` |
| 9.5g | `handle_timer_tick_use_case.rs` | `should_request_the_llm_ghost_text_when_the_debounce_ran_out` (FixedClock), `should_drop_the_llm_schedule_when_nothing_was_sent`, `should_spin_while_a_reply_is_loading` (FixedClock), `should_poll_the_files_only_after_the_poll_interval` (FixedClock), `should_release_held_keys_before_checking_the_insertion_lock`, `should_write_the_repaint_and_the_frame_in_one_write`, `should_act_on_an_empty_line_once_the_redraw_grace_is_over`, `should_redraw_when_the_settings_changed`, `should_poll_the_files_before_drawing_the_repaint_and_the_frame` |
| 9.5h | `end_session_use_case.rs`, `apply_terminal_background_use_case.rs`, `prepare_session_launch_use_case.rs` | `should_clear_the_overlay_and_show_the_cursor_in_one_write`, `should_shut_the_engine_down_then_reap_the_generators_then_restore_the_modes_then_remove_the_context`, `should_remove_the_context_of_the_session_only`; `should_recolour_the_screen_from_the_reported_background_for_the_system_theme`, `should_set_the_theme_even_when_it_did_not_change`, `should_remember_the_background_for_later_reloads`; `should_prefer_the_explicit_shell_over_the_environment`, `should_use_tabsmith_shell_before_the_system_shell` (`#[cfg(not(windows))]`), `should_use_pwsh_on_windows_without_reading_the_system_shell` (`#[cfg(windows)]`), `should_treat_an_empty_shell_as_unset`, `should_add_login_only_for_the_shells_that_take_it`, `should_replace_the_shell_with_a_first_extra_argument_that_is_not_an_option` |
| 9.6c | `terminal_session_module.rs` | `should_locate_the_data_root_of_the_data_directory_it_was_built_with` (and the no-home error text) |
| 9.6e | `terminal_probe.rs` (`probe_terminal`, written when the launcher is: it needs the 9.4b writer) | `should_stop_probing_at_the_first_cursor_report`, `should_stop_probing_at_the_deadline_without_a_cursor_report` (timeout 10 ms), `should_return_the_keys_typed_while_probing`, `should_write_the_probe_query_before_reading` |
| 9.6e | `pty_session.rs` | `should_return_the_exit_code_when_the_session_ends_normally`, `should_pass_a_failed_start_through_as_the_error`, `should_report_an_internal_error_when_the_session_panics_outside_a_handler` |

Name collisions found across the area notes and resolved above (R32): the use-case versions of the exit tests are `should_stop_the_loop_at_once_when_the_output_had_already_ended`, `should_keep_the_loop_going_after_the_output_ended_until_the_child_exits` and `should_stop_the_loop_with_exit_code_129_when_terminated`
(the entity keeps `should_leave_at_once_when_the_output_had_already_ended` and `should_go_on_after_the_output_ended_until_the_child_exits`, and the loop test of 9.0d keeps `should_exit_with_129_when_terminated`); the use-case LLM test is `should_draw_nothing_for_an_llm_reply_to_another_request`
(the entity keeps `should_ignore_an_llm_reply_for_another_request`); the lenient-settings test is `should_load_what_the_repository_saved` (`should_give_the_settings_the_repository_loads` already exists in settings). The tests that pin the cross-port order through `IoJournal` are
`should_write_to_the_shell_only_after_the_frame_was_drawn` and `should_ring_the_bell_before_the_frame_and_the_shell_write` (9.5c) and `should_write_the_batch_in_one_write_and_release_held_keys_after_it` (9.5d).

## Output-equivalence guard

The golden capture (131 CLI commands) reaches none of this phase except `_ pty` error paths and `debug shell --help`: the nets are the unit tests, the characterisation fixtures, two real-session diff scripts, the older diff scripts, the byte-hash tests of Phase 8, the architecture test and a
throwaway differential test. Commands per step (repo root, after `cargo build --locked`):

```
BASELINE_CLIPPY=4 scripts/hexagonal-refactor/gate.sh                                   # GATE PASS: build, tests, clippy, windows check, golden, testlock vs ef807c1
python3 -B scripts/hexagonal-refactor/testlock.py . ef807c1 --diff                      # MISSING 0, DUPLICATE 0, CHANGED = the step's list
python3 -B scripts/hexagonal-refactor/testlock.py . phase9-start --diff
python3 -B scripts/hexagonal-refactor/testlock.py . phase9-guards --diff                # from 9.0d
git diff phase9-guards -- tests/fixtures/terminal_session/characterisation              # EMPTY from 9.0d (the frozen values)
scripts/hexagonal-refactor/phase9-greps.sh phase9-start                                 # the greps of "Acceptance greps" that apply
OLD=target/hexagonal-refactor/baseline-tree/target/debug/tabsmith; export PATH=$PWD/scripts/hexagonal-refactor/compat:$PATH   # the gate's stat/sed shims
for x in precmd inline theme integrations; do diff <(scripts/hexagonal-refactor/$x-diff.sh $PWD/$OLD) <(scripts/hexagonal-refactor/$x-diff.sh $PWD/target/debug/tabsmith); done   # 1 s, 7 s, 20 s, 62 s; all empty
diff <(python3 -I scripts/hexagonal-refactor/pty-diff.py $PWD/$OLD) <(python3 -I scripts/hexagonal-refactor/pty-diff.py $PWD/target/debug/tabsmith)           # ~12 s, empty
diff <(python3 -I scripts/hexagonal-refactor/session-diff.py $PWD/$OLD) <(python3 -I scripts/hexagonal-refactor/session-diff.py $PWD/target/debug/tabsmith)  # ~85 s, empty; needs tmux, bash, vim, git
cargo test --locked debug_shell_survives_closed_terminal                                # the only automated start-up of the wrapper, ALONE
cargo check --locked --no-default-features --all-targets; cargo test --locked --release -- --ignored all_bundle_modules      # at every run end
```
Which step runs which (the rest of the matrix is "gate, testlock, frozen fixtures, greps" every step):

| Net | Steps |
|---|---|
| older four diff scripts | 9.4a, 9.6c, every run end (they cover `bootstrap/cli.rs`, the module wiring, `cli.rs`) |
| `pty-diff.py` + `session-diff.py` | 9.0a (baseline equality, twice) and every step from 9.1e on except 9.1c, 9.1d, 9.2d (code without a caller), 9.6b (platform files moved) and 9.6c (only the scenarios `keytester`, `failures`, `files`); every run end |
| `debug_shell_survives_closed_terminal` alone, 3 runs | 9.2e, 9.3b, 9.3g, 9.4a, 9.4b, 9.4c, 9.5a, 9.5h, 9.6a, 9.6c, 9.6d, 9.6e, 9.6f; every run end |
| 20 serial runs of the pty tests and the characterisation tests | 9.0c, 9.0d, 9.1e, 9.2e, 9.2f, 9.3a-9.3i, 9.4a-9.4c, 9.5a-9.5h, 9.6d-9.6f |
| windows `cargo check --all-targets` (inside the gate) | mandatory at 9.1e, 9.3b, 9.3e, 9.4a, 9.4b, 9.4c, 9.5f, 9.5h, 9.6a, 9.6b, 9.6c, 9.6d, 9.6e, 9.6f; free elsewhere |
| the throwaway differential test | 9.1e only (never committed) |

### The characterisation tests (9.0b-9.0d): scenario design

- Helper `assert_matches_fixture(name, actual)` in `shared/infrastructure/testing/fixture_log.rs` (the module is `#[cfg(test)]`): compares `actual` with `tests/fixtures/<name>` found through `CARGO_MANIFEST_DIR`, so a test can move; a missing file is created only when `P9_WRITE_FIXTURES=1` and the test then
  FAILS ("fixture created, review it"); an existing file is never rewritten; a mismatch prints a line diff. Its inner `check(root, name, actual, create)` is what the helper's own tests call (no process environment in tests).
- Tracker (in `tracker.rs`'s test module; helper `transcript(rig, chunks) -> String` records per chunk the input, the filtered output bytes, the events (`describe_event`: no clock value), `at_prompt/preexec/cycle/cwd`, the `extract` tuple and the hyperlink runs of every row via `tags(m)`):
  `tracker_bash` (30x8 bash: split markers across chunks, a 40-char line wrapping with `\x1b[2K`, a title OSC passing, `NoHistory` split mid-marker, `ExitCode`, the keyboard-upgrade strips `?9001h`, `>4;1m`, `=1;1u`, `>c`, `1;5r`), `tracker_defer` (pwsh, `defer = true`: the queue flushing at the END of a marker-free batch at that
  batch's cursor, `NewCmd(Some)` with `end_time` set: the only Linux pin of the real Windows path; PLUS one chunk that carries `EndPrompt`, `NewCmd=<sid>` and `PreExec` back to back while the previous command is still running (`preexec` true), followed by a marker-free chunk. That is the P9-T6/P9-N1 case: with the deferral on, the `NewCmd` is only QUEUED, so `PreExec` still sees `preexec == true` and is ignored, and the queue is flushed at the end of the marker-free chunk. The fixture must show no `PreExec` event for the third marker, the flush at the marker-free chunk and nothing else; a tracker that flushes BEFORE testing the PreExec guard would print a different log. This is the only committed pin of that order on the Windows path), `tracker_zsh` (`ZshAutosuggestionColor=fg=8`, grey `ho hi` versus typed `ec`, OSCLock/PreExec, a nested prompt with `PID=99`, `exit`, unlock).
- Session (in `pty.rs`'s test module; `Rig { s: Session, pty: Receiver<Vec<u8>>, engine: Receiver<CompletionEngineCall>, log: String, mark: usize }`, `note(step)` appends `step`, `stdout=<escape_ascii>` (the sink delta), `pty=<escape_ascii>` (the PTY bytes since the last note) and `engine=[..]` (`describe(call)`: no cwd, no time), `finish(fixture)`);
  the 14 scenarios of "New tests" 9.0c with fixtures `typing`, `bell`, `held`, `lifecycle`, `cleanup`, `ghost`, `lock`, `resize`, `llm`, `reload`, `probe`, `passthrough`, `spinner`, `batching` (322 to 2909 bytes each, ~20 KB in all). All forced-due field pokes go through the `poke_*` helpers (R34).
  Within-event interleaving of a stdout write and a PTY write is not observable: the per-step deltas isolate what can be (a step with a Bell has the Bell in `stdout` and the line in `pty`).
- Survival through the moves: ONLY the driver lines of a scenario change (`s.on_input(b)` to `input`, `s.on_output(b)` to `output`, `s.on_completion(c)` to `complete`, `s.on_timers()` to `tick`, `s.redraw()` to `redraw`, `s.on_resize(c, r)` to `resize`, `s.cleanup()` to `end`, `sent(&pty)` to `h.sent()`, the sink delta to `h.written()` since the last
  `clear_written()`, the engine receiver to `h.engine_calls()`, the pokes to the aggregate's `#[cfg(test)]` setters); the scenario script, labels and fixtures never change. Review rule for every moving step: the frozen-fixture diff is EMPTY and the test bodies differ only in those lines.
  Final homes (9.6f): scenarios 1-10 and 13 in `application/session_use_cases/tests.rs`; 11 (`probe`: the late-reply strip is the runner's guarded `on_input`), 12, 14 and the 13 loop tests in `infrastructure/pty/session_runner/tests.rs`, the three tracker scenarios in `alacritty_terminal_screen/tests.rs` (they move at 9.2f with the 26).
- Loop tests (9.0d), characterising the untested batching and exit logic: names in "New tests"; the fake `MasterPty` of `should_resize_only_the_pty_while_passing_through` records `PtySize { rows, cols, pixel_width: 0, pixel_height: 0 }`.

### `session-diff.py` and `pty-diff.py`

- `pty-diff.py <abs binary>` drives `tabsmith _ pty` on a real pty (python `pty.fork`) through 10 scenarios and prints exit code, scrubbed bytes (home, 32-hex session id, version), `run/` files and `logs/pty.log` lines without timestamps: `answers-probe-exit3` (OSC 11 and CPR answered, `exit 3`), `no-answer-typeahead` (300 ms timeout, `\x1b[H\x1b[2J`), `env-and-cwd`, `sigterm` (129), `no-shell` (`SHELL=""`, fallback `/bin/sh`), `not-a-tty-fallback`
  (`TABSMITH_TERM_DISABLED=1`), `typeahead-during-probe` (`xy` reaches the child), `cwd-gone-falls-back-to-home`, `corrupt-settings-warns` (`Warn - settings: <HOME>/settings.json is corrupted: ..`), `log-line-at-vv` (`Info - pty: [..] 1x1 sid <SID> cpr Some((5, 1))`). The script text is the driver-cli note's Appendix A (proven identical between the baseline binary and HEAD).
- `session-diff.py <abs binary> [scenario ...]` starts each interactive scenario in a detached tmux server (`-L <unique> -f /dev/null`, 80x24) running `env -i HOME=.. TABSMITH_HOME=.. TERM=xterm-256color COLORTERM=truecolor <bin> -vv debug shell bash; echo EXIT=$?; sleep 30`, types with `send-keys` (ONE keystroke per read for the menu scenarios: the first reply after a prompt only arms the menu),
  waits for expected PLAIN screen text and for 0.45 s of quiet, and prints `capture-pane -p -e` (the escapes keep every colour). Scenarios: `startup`, `menu` (`git ch`, Down x2, Tab, Esc), `ghost` (`echo hi`, `ec`, Right), `child` (`_ pty --shell bash -- --noprofile --norc`: cwd, argv0, the `TABSMITH_*` env, the title OSC, `exit 5`), `bottom` (`seq 1 40`, menu above a bottom-row prompt, resize 60x18 and back),
  `vim` (no overlay on the alternate screen, `exit 3`), `reload` (`settings autocomplete.height 2`, `theme light` from outside, wait 2.6 s), `keytester`, `failures` (no tmux: `_ pty`, `debug shell [bash zsh fish pwsh]` and `debug key-tester` with stdin `/dev/null`, stub shells on PATH, three envs), `files` (`logs/pty.log`, `run/`, `history.jsonl`). Deterministic in four runs, baseline equals HEAD, catches an env var and a resize-debounce mutation. Not covered by design: zsh, fish, pwsh,
  kitty/ghostty/tmux icons (the smoke), Windows (CI).

### Manual smoke (the user's, after 9.7; plan section 8)
`TABSMITH_HOME=$(mktemp -d) cargo run -- debug shell zsh` (and bash, fish): (1) `git ch` shows the menu; (2) Tab inserts, a shared prefix inserts; (3) Esc hides it; (4) `echo hi` then `ec` shows ghost `ho hi`, Right accepts it, Alt-f one word; (5) a prompt on the bottom row draws the menu above; (6) a resize leaves no artefacts; (7) `vim` draws no overlay; (8) `exit 3` returns 3; (9) `tabsmith doctor` passes. Phase 8's still-pending P8-S1 (the spinner after ~200 ms on a slow generator) and P8-S2 (`tabsmith theme light` from a second terminal recolours the menu; a late OSC 11 reply with `system`).
Phase 9's: **P9-S1** closing the terminal window of a `debug shell` ends it cleanly (the rc dir is removed); **P9-S2** `kill -TERM` of the wrapper exits 129; **P9-S3** typing during start-up reaches the shell (typeahead during the probe); **P9-S4** a settings change from a second terminal (`tabsmith settings autocomplete.height 3`) takes effect within ~2 s; **P9-S5** pwsh on Windows (CI-only): the prompt markers deferral and the ConPTY exit.

## Hand-off notes

**Phase 10 (diagnostics).**
- `settings::infrastructure::load_or_defaults` has one caller left, `doctor.rs` (the pty caller went at 9.4c); `LenientLiveSettings` is the pattern step 2 of Phase 10 repeats in diagnostics' own infrastructure, after which `load_or_defaults` is deleted.
- `cli.rs` after Phase 9 holds `DiagnosticArgs`, `diagnostic`, `diagnostics`, `to_toml`, `Section`, `debug_logs` (now `pub fn`, called from `bootstrap/cli.rs`) and the test `diagnostic_toml`; `main.rs` keeps `mod cli;` until Phase 10 deletes it. `debug_logs` reads `data_directory_from_env().logs_dir()`: the `logs/{cli,pty}.log` contract with `shared/infrastructure/logger.rs` is unchanged.
- `bootstrap::cli::terminal_session_module()` and `GeneratorProcessBridge` exist; doctor does not need them.
- The doctor checks that read the session context or `run/` files keep reading them through their own raw checks (Phase 4 hand-off).

**Phase 11.**
- `types.rs` ends the phase holding only `now_ms` (no caller; the one mention left is a string inside a fixture of `architecture_tests.rs:713`), `now_secs` (one caller, `doctor.rs:32`) and the `Clock`/`SystemClock` imports (its `CompletionResult` import dies with `Msg` at 9.6a); Phase 10 may move the doctor caller to the `Clock` port, Phase 11 deletes the file with `#![allow(dead_code)]`. `LEGACY_MODULES` keeps every dead name (`pty`, `tracker` join `cli doctor engine icons inline integrations js keys menu parser render shell_parser spec suggest types`); drop the dead names in the sweep. The dead-code scan (delete the `#![allow(dead_code)]` line in a COPY, `cargo check --locked --bin tabsmith`, group the `-->` paths) found 7 warnings at HEAD and 0 under `terminal_session`; Phase 9 must add none (D7: test-only items get `#[cfg(test)]`).
- Watch list (D7): `EditBuffer: Default` (kept for `Option<EditBuffer>` defaults; drop if clippy or the scan flags it), `MenuEffect::Insert.full` and `Menu::vis()` (Phase 8), `MarkerTracker`'s `#[cfg(test)]` accessors, `TerminalSession::tracker_mut` (`#[cfg(test)]`, read by two tests; drop it when they read a narrower setter), `TerminalBackground::rgb`, `LockSettlement`, the temporary `pub(crate)` fields (none must remain: grep B8), `TerminalSession` itself (about 575 production lines and 60 methods: `route_key`, `insert`, `forward_key` could move into a keystroke-specific domain object).
- `rgb_to_256` equals `dropdown::color_parser::xterm256((r, g, b))` for all 2^24 inputs (standalone program, 0 differences): the dedupe edits two locked tests' call sites (`suggestion_colors`, `pwsh_reported_prediction_colour`).
- A module-edge rule (`ALLOWED_EDGES`, about 60 lines and four tests of checker code) turns plan section 2 into a test; production edges of every module were verified by script at 9.0a (`bootstrap -> everything`, `completion -> history settings shared specs`, `dropdown -> completion settings shared specs`, `ghost_text -> completion history settings shared specs`, `history -> shared`,
  `settings -> shared`, `shell_integration -> shared`, `specs -> settings shared`, `terminal_session -> completion dropdown history settings shared`).
- `docs/SPEC.md`: §4.1 (rows `pty.rs`, `tracker.rs`, lines 497-498), §4.3, §5.1 (lines 679, 689 still document `has_seen_prompt` and the `ShellPath`/`TTY`/`User` context fields), §10 (test locations).
- Noticed, not fixed: P9-N1..N6 above.

## Glossary additions (proposed for `docs/ubiquitous-language.md`; 9.7 writes them)

The terminal_session section's intro ("Not in the table") gains: `icon_assets`, `kitty_graphics` (unchanged), `EditBufferExtractor`, `CellTagger`, `headless_model` and the `*Parts` structs (`TerminalSessionParts`, `EditBufferContextParts`, `ScreenStateParts`, `KeyFactsParts`, `SessionPortsParts`, `SessionRunnerParts`, `ProcessShellOutputParts`: named fields for a
constructor that would otherwise take many parameters).

| Term | Kind | Meaning | Replaces |
|---|---|---|---|
| `TerminalSession` (edit) | Aggregate | The wrapper's view of the running shell: the latest request, the menu, the insertion lock, the echo hold, the ghost texts, the timers, the exit state; owns the `MarkerTracker`, the `MarkerSplitter` and the `KeyDecoder`. It decides and never acts: time and screen facts come in, values go out. | `pty::Session` (minus plumbing) |
| `SessionId` (edit) | VO | + `from_entropy(nanos, pid)`: 32 lowercase hex digits, the shape the shell scripts echo in `NewCmd=`. | `session_id()` |
| `MarkerTracker` (edit) | Entity | Prompt/command cycle from markers: cwd, shell, pid, exit code, integration lock, pending commands, deferral. Driven by the application: it takes the parsed marker, the cursor column and the wall-clock seconds, and answers with `MarkerEffects`. | `tracker::Tracker` state |
| `MarkerEffects` / `MarkerReaction` | VO | What one tracker step asks of its caller: the screen commands to apply now, in order, and the events to hand on after the batch / `Handled`, or `EditBufferNeeded` (the `PreExec` handshake). | |
| `ShellMarker` (edit) | VO | A parsed OSC 697 marker (total parse; each key keeps its own trimming; `Ignored` for empty values, unknown keys and the keys that set nothing). | marker strings |
| `ShellEvent` (edit) | VO | `NewCommand(Option<HistoryEntry>)`, `CommandEnded`, `PreExec`. | `tracker::Event` |
| `EditBuffer` (edit) | VO | + private fields; `cursor` is a byte offset; `at_prompt` means the cursor is on the command line, not `MarkerTracker::at_prompt`. | `Extracted` |
| `EditBufferContext` | VO | What reading the command line off the screen needs of the tracker (open cycle, shell, line start, list view, pwsh prompts, three colours), borrowed. | the `Tracker` fields `extract` read |
| `PwshPredictionColor` / `TextEmphasis` | VO | PSReadLine's reported prediction colour and the dim/italic it requires. | `(SuggestionColor, Flags)` |
| `CellTag` | VO | Which cells are the prompt or the command line of cycle `n`: the `tabsmith:p` / `tabsmith:c{n}` URIs. | `TAG_PROMPT`, `format!("tabsmith:c{n}")` |
| `ScreenCommand` (edit) | VO | `Tag(CellTag)`, `ClearTag`, `MarkLineStart { redrawn_cycle }`. | inline `tag` / `mark_line_start` calls |
| `ScreenState` (edit) | VO | A fresh `Copy` snapshot of what the render guards read: alternate screen, origin and insert mode, pending wrap, size, scroll region, open string. | ad-hoc reads in `suppressed` / `drawable` |
| `GhostCandidates` | Entity | The local and the LLM ghost text, the LLM request id and its debounce deadline. | `ghost`, `llm`, `llm_req`, `llm_at` |
| `ResizeDebounce` | Entity | The quiet spell after a resize and the pending repaint (SPEC 7.6). | `resize_at`, `repaint` |
| `ShellExit` | Entity | Exit code, end of the output, the exit grace. | `exit_code`, `eof`, `exit_at` |
| `ReloadWatch` | Entity | When the settings and session-context files are looked at and the modification times seen. | `poll_at`, `settings_mtime`, `session_mtime` |
| `KeyDecoder` (replaces `StdinKeyDecoder`) | Entity | Decodes stdin bytes (UTF-8, `ESC`, SS3, CSI, bracketed paste) into `KeyEvent`s; keeps only the paste state between chunks; reset when keys are held. | `keys::Decoder` |
| `KeyRoute` (edit) / `KeyFacts` | VO | `Hold`, `MenuAction`, `Swallow`, `ShowOnTab`, `AcceptGhost`, `Forward` / what the router needs besides the key. | branches of `on_keys` |
| `CompletionQuery` | VO | What a refreshed line asks of the engine (request id, buffer, byte cursor, history mode); the service adds directory and shell. | the `CompletionRequest` literal |
| `KeyRouter` / `RenderGuard` / `SessionDeadlines` / `InsertionReconciler` | Domain service | Routes one key / whether the screen allows drawing / the nearest wake-up and the timer durations / the bytes and the expected buffer of an insertion. | `on_keys` arms, `drawable`, `next_deadline`, `insertion_bytes` |
| `WrappedShellCommand` (edit) | VO | The wrapped launch: argv, the five env changes of a session, the window title. | `pty::argv` + env setup |
| `TerminalBackground` (edit) | VO | A thin newtype over dropdown's `Rgb`: the background colour OSC 11 reported. | `terminal_bg` |
| `TerminalScreen` | Port | The wrapper's mirror of the real terminal: pass shell output through the model and the filter, tag cells, read the command line and the render-guard state, resize, draw and clear the overlay. | `term`, `renderer`, `tracker.filter` |
| `TerminalWriter` / `WrappedShell` | Port | The user's terminal (stdout) / the wrapped shell's input and window size. | `emit`, `write_stdout`, `sink` / `writer`, `master` |
| `GeneratorProcessReaper` / `TerminalModes` | Port | Kills generator processes still running at the end / restores the terminal modes. | `kill_live_children` / `modes.restore` |
| `LiveSettings` (`LenientLiveSettings`) | Port | The settings a running session follows: the file's modification time and the lenient load (defaults and a warning when they cannot be loaded). | `load_or_defaults`, `settings_mtime` |
| `WorkingDirectory` (`ProcessWorkingDirectory`) | Port | The wrapper process's own directory, read at every request. | `std::env::current_dir()` in `Session::cwd` |
| `AlacrittyTerminalScreen` / `OutputFilter` | Adapter | The headless alacritty model (2000 lines of scrollback), its long-lived parser, the output filter and the session's ONE overlay renderer / filters the bytes written to the real terminal (keyboard-protocol upgrades, scroll region, open strings, held UTF-8). | `types::Model` + `Tracker::{proc, filter}` / `OutFilter` |
| `StdoutTerminalWriter`, `PtyWrappedShell`, `ConsoleTerminalModes`, `GeneratorProcessBridge` (bootstrap) | Adapter | The port implementations. | |
| `ShellOutputFeeder` | Application service | Feeds one PTY batch through the splitter, the tracker and the screen in stream order and returns the events. | `Tracker::feed` |
| `KeystrokeHandler` / `CompletionRequester` / `OverlayPainter` / `SessionFilePoller` | Application service | The key loop and the replay of held keys / the edit-buffer refresh and the request / drawing, redrawing and repainting the overlay / reloading settings and the session context. Shared by several use cases; never use cases themselves. | `on_keys`+`release_held` / `refresh` / `frame`+`redraw`+`take_repaint` / `poll_files` |
| `HandleKeystrokesUseCase`, `ProcessShellOutputUseCase`, `ApplyCompletionResultUseCase`, `ApplyLlmGhostTextUseCase`, `ResizeTerminalUseCase`, `HandleShellExitUseCase`, `HandleTimerTickUseCase`, `ApplyTerminalBackgroundUseCase`, `EndSessionUseCase`, `PrepareSessionLaunchUseCase` | Use case | One per event of the session (table in plan section 4, amended). | `Session::on_*`, `step`, `cleanup`, `run`'s resolution |
| `FrameInput` (edit) | DTO | The per-frame inputs, with the instant the spinner is judged at; now in the application layer. | `render::FrameInput` |
| `ShellEnd` / `LaunchRequest` / `LaunchEnvironment` / `SessionPortsParts` | DTO | How a shell ended / what the launch resolves a shell from / the env facts / the runtime ports one session runs on. | |
| `SessionRunner` / `SessionMessage` | Adapter (driver) | The event loop, the panic pass-through and the batching / the messages of its one inbox. | `pty::Session` (driver half) / `types::Msg` |

"Words with more than one meaning" gains: **at prompt** | `MarkerTracker::at_prompt` (a command cycle is open and no command runs) · `EditBuffer::at_prompt` (the cursor is on the command line, not in an inline TUI's rows); **shell** | the launched shell (`launched_shell`: the program the wrapper spawns) · the tracker's `Shell` (what the prompt reported with
`Shell=`; it wins once known and can be `None`); **screen** | the real terminal · the model (the headless `Term`) · `TerminalScreen` (the port over the model, the filter and the renderer). The `lock` row is unchanged. The `StdinKeyDecoder` row of the terminal_session table is deleted, the `OverlayRenderer / FrameInput` row drops "stays in the renderer's file until Phase 9", and the
`WrappedShellCommand`, `TerminalBackground`, `ShellMarker`, `ShellEvent`, `EditBuffer`, `ScreenCommand`, `ScreenState` rows are edited to the meanings above.

## Steps

Each implementer reads `docs/hexagonal-refactor/agent-brief.md`, then this file (the traps and decisions its step cites, "Module map", "Signatures", "Test moves", "New tests") and the research notes it names, and loads the skills `xp-tdd`, `testing-standard`, `coding-standards` and `hexagonal-architecture` (AGENTS.md).
Every step ends with the full gate green and everything committed (R45), the test lock clean (MISSING 0, DUPLICATE 0, exactly the step's CHANGED tests), the frozen fixtures untouched, the hand greps that apply, the cfg check, the clippy budget (4 on Linux) and the `should_` names grepped first (K8).
Each step compiles on its own and leaves no shim across its end: the only interim composites are the legacy `Tracker` (9.1e-9.2f), the aggregate's `pub(crate)` fields (9.3g-9.3i) and the `Session` delegates (9.5a-9.5h); none is a `pub(crate) use` (R4). New seams are test-first: RED, GREEN, commit, REFACTOR, commit (xp-tdd); a pure move is
one commit whose block diff against `git show phase9-start:<file>` shows only paths, visibility, glossary names, accessors and the listed edits (R45.4). Subjects are `refactor(terminal_session): ...`, `test(terminal_session): ...`, `chore(scripts): ...`, at most 72 characters, bodies ending with the two trailers (R44). "NEW n" counts names in "New tests";
"CHANGED" is first-time against `phase9-start`. A step that finds the tree different from the rows below stops and reports; it does not improvise.

### Run plan

| Run | Steps | Ends with | The orchestrator then |
|---|---|---|---|
| 1 Guards | 9.0a-9.0d | `terminal_session` registered; four scripts; 17 fixtures frozen; 14 loop and id tests; 34 NEW, no production change | gate; reads the fixtures once; both diff scripts equal the baseline; sets the local tag `phase9-guards` (R1) |
| 2 Tracker | 9.1a-9.1e, 9.2a-9.2f | `MarkerTracker`, the adapters, `TerminalScreen`, `ShellOutputFeeder`; `src/tracker.rs` and `types::Model` gone; `Session` holds the screen | run-end list (below); `SINCE` preamble for run 3 |
| 3 Session domain | 9.6a, 9.6b, 9.3a-9.3i | `types::Msg` gone, the reader, probe parsers and `sys` moved; `TerminalSession` and six entities own the state (fields private again); the legacy `Session` methods are delegates that still perform the I/O | run-end list; `SINCE` for run 4 |
| 4 Ports and CLI | 9.4a-9.4c, 9.6c | collaborators injected; six ports with adapters; `cli.rs` free of the debug code; `pty.rs` free of rule-3 imports (dry run) | run-end list incl. the four older diff scripts; `SINCE` for run 5 |
| 5 Services and use cases | 9.5a-9.5h | four more services (`OverlayPainter`, `CompletionRequester`, `SessionFilePoller`, `KeystrokeHandler`) and the ten use cases; every `Session::on_*` body gone; 24 tests moved to the use cases | run-end list; `SINCE` for run 6 |
| 6 Runner and close | 9.6d-9.6f, 9.7 | `src/pty.rs` gone; `SessionRunner`, launcher, `run_pty_session`; docs; the greps below all OK | the full list; asks the user for the manual smoke (O14) |

Run-end list (R1, "Output-equivalence guard"): the gate; `testlock.py` against `ef807c1`, `phase9-start` and `phase9-guards`; `git diff phase9-guards -- tests/fixtures/terminal_session/characterisation` empty; `phase9-greps.sh`; the four older diff scripts, `pty-diff.py` and `session-diff.py` old versus new binary, all empty;
`cargo test --locked --release -- --ignored all_bundle_modules`; `cargo check --locked --no-default-features --all-targets`; 20 serial runs of the pty and characterisation tests; `cargo test --locked debug_shell_survives_closed_terminal` alone x3.

### Run 1: guards (9.0a-9.0d)
Ends with the tag `phase9-guards`. No production line changes in this run (a characterisation test has no RED).

| Id | Title | Plan step | What it does |
|---|---|---|---|
| 9.0a | Register the module; the scripts | (section 8) | Commit 1 `chore(scripts)`: lift `recon-governance-prototype/{fitness-mod.sh, session-diff.py}`, `p9-greps.sh` (as `phase9-greps.sh`; fix A3: its second alternative must exclude a preceding `:` or every `infrastructure::pty::..` path of the final tree matches) and Appendix A of `recon-driver-cli.md` (as `pty-diff.py`) into `scripts/hexagonal-refactor/`; prove `session-diff.py` and `pty-diff.py` print identical output for the baseline binary and `target/debug/tabsmith`, twice each (R35); `fitness-mod.sh terminal_session` prints 55 passed. Commit 2 `refactor(terminal_session): register the module in the fitness test`: `"terminal_session"` after `"specs"` in `REGISTERED_MODULES`, `LEGACY_MODULES` untouched (R3); `phase9-greps.sh phase9-start` shows A1-A6 FAIL by design, E1 OK, B and C1 as expected. Re-check K1-K16 and report the table |
| 9.0b | Fixture helper; three tracker scenarios | (section 8) | RED/GREEN `shared/infrastructure/testing/fixture_log.rs` (`assert_matches_fixture(name, actual)` over the inner `check(root, name, actual, create)`; 3 tests; `pub(crate) mod fixture_log;` in `testing/mod.rs`; R34). Then in `tracker.rs`'s test module: helper `transcript(rig, chunks)` + `describe_event`, the 3 tests of New tests 9.0b and `tests/fixtures/terminal_session/characterisation/tracker_{bash,defer,zsh}.log`, created with `P9_WRITE_FIXTURES=1` (each test fails once with "fixture created, review it"; read each file: filtered bytes, events, tags, extract tuple), then green without the variable. Lift `characterisation_tracker_tests.rs.txt`. All pass on the old code |
| 9.0c | Fourteen session scenarios | (section 8) | `pty.rs` test module: `Rig`, `note`, `finish` and the seven `poke_*` helpers (`poke_lock_due`, `poke_resize_due`, `poke_llm_due`, `poke_poll_due`, `poke_hold_due`, `poke_spinner_due`, `poke_probe_window`), the 14 `should_.._exactly_as_at_the_phase_9_start` tests of New tests 9.0c and 14 fixtures (`typing bell held lifecycle cleanup ghost lock resize llm reload probe passthrough spinner batching`, `.log`), lifted from `characterisation_session_tests.rs.txt`. Every field poke goes through a `poke_*` helper; time is forced, except the spinner scenario's single 250 ms floor sleep. Each fixture is read once against the call-order table P9-T99; 20 serial runs green |
| 9.0d | Loop tests and the id shape | (section 8) | `pty.rs` tests: the thirteen loop tests (the eleven of `recon-driver-cli.md` d2 plus the two panic-path tests designed here; names in New tests 9.0d; they drive `Session::step` over the `session()` fixture, and `run` only where `step` cannot show the behaviour; the resize test uses a fake `MasterPty` recording `PtySize { rows, cols, 0, 0 }`) and `should_generate_a_session_id_of_32_lowercase_hex_digits` (calls `session_id()`). All pass on the old code. The orchestrator then sets `phase9-guards` |

### Run 2: tracker (9.1a-9.1e, 9.2a-9.2f)
Ends with `src/tracker.rs`, `mod tracker;` and `types::Model` deleted; `crate::tracker` and `types::Model` no longer appear (A5 half, F2 half).

| Id | Title | Plan step | What it does |
|---|---|---|---|
| 9.1a | Splitter, pieces, colour value objects | 1 | `domain/entities/marker_splitter.rs` (`Splitter` -> `MarkerSplitter`, `tracker.rs:47-153`, the 7 splitter tests and helpers `split_all`, `bytes_of`, `markers_of` moved by script; `OSC_MAX` stays private), `value_objects/output_piece.rs` (`Piece` -> `OutputPiece`). Test-first: `text_emphasis.rs` (2 tests); `shell_suggestion_color.rs` (over `Option<u8>` colour indexes, `matches(fg, bg)`, `parse_zsh/fish`, `rgb_to_256`; 1 test, then the move; D6 split of `suggestion_colors`: the parse asserts keep the name, the Cell asserts become `suggestion_colors_cell_matching` and stay in `tracker.rs`); `pwsh_prediction_color.rs` (`PwshPredictionColor { colors, emphasis }`, `parse` from `parse_sgr`, `matches(fg, bg, cell)`; 1 test; D6 split of `pwsh_reported_prediction_colour`: the extract half keeps the name, the SGR asserts become `pwsh_reported_prediction_colour_sgr`). `tracker.rs` imports the new types (an import, not a re-export) and keeps the alacritty `Color`/`Flags` conversions (`cell_col`, `matches_cell`, `cell_emphasis`) until 9.2b; `mod` lines in `domain/{entities,value_objects}/mod.rs`. R7, R8; P9-T9, T16, T28 |
| 9.1b | Protocol value objects | 1 | Test-first: `edit_buffer.rs` (`EditBuffer`, private fields and accessors; replaces `Extracted` in `tracker.rs` and the type of `cur` in `pty.rs`; the six tracker tests and the pty code read `buffer()/cursor()/shell_ghost()/at_prompt()`), `cell_tag.rs` (`CellTag`, `PROMPT_URI`, `uri()`: replaces `TAG_PROMPT` and the four `format!("tabsmith:c{n}")` of `tracker.rs:682, 691, 692, 778`), `marker_effects.rs` with `marker_reaction.rs` (1 test). Created now, used from 9.1d, without tests of their own: `screen_command.rs`, `shell_event.rs`. R8, R9; P9-T15, T20 |
| 9.1c | `ShellMarker` | 1 | Test-first: `shell_marker.rs` (`ShellMarker::parse` total, `passes_integration_lock`; 13 tests). The grammar is `Tracker::on_marker` (`tracker.rs:620-678`) read as the spec: per-key trimming (P9-T9), the lock allow-list (T10), `ShellPath`/`TTY`/`User` -> `Ignored` (R7). New, no caller (the legacy `on_marker` still parses strings) |
| 9.1d | `MarkerTracker`; `SessionId::from_entropy` | 1 | Test-first: `SessionId::from_entropy(nanos, pid)` (2 tests; the literal `0123456789abcdef8acf13579bde026c`); `entities/marker_tracker.rs` (`MarkerTracker`) and `value_objects/deferred_marker.rs` (`DeferredMarker`, `pub(crate)`) with `new(session_id, shell, defer_markers)`, `on_marker`, `complete_pre_exec`, `end_batch`, `edit_buffer_context` (the VO in `edit_buffer_context.rs` with its `Parts` struct), `at_prompt`, `preexec`, `cwd`, `shell`, `pwsh_list_view`, `submitted`/`set_submitted` and the `#[cfg(test)]` accessors; 30 tests over hand-built `ShellMarker`s. Lift `recon-tracker-domain-prototype/`. New, no caller. R5, R6, R27; P9-T1-T14, T24, T25 |
| 9.1e | The legacy `Tracker` delegates | 1 | `tracker.rs`: `Tracker` becomes a composite `{ splitter: MarkerSplitter, marker: MarkerTracker, filter: OutFilter, proc: Processor, events: Vec<ShellEvent> }`; `feed` runs the application loop in place (per piece: `ShellMarker::parse`, `marker.on_marker(parsed, cursor_column, now_secs())`, apply the `ScreenCommand`s with the legacy `tag`/`mark_line_start`, scrape through the legacy `extract` for `EditBufferNeeded`, then `complete_pre_exec`; after the last piece `end_batch`); `shell_path`, `tty`, `user`, `has_seen_prompt` die with their three markers (R7); `Event` -> `ShellEvent`. `pty.rs` reads the tracker through accessors (`tracker.marker.cwd()`, `shell()`, `at_prompt()`, `submitted()`, `set_submitted(..)`, `pwsh_list_view()`, `preexec()`); the ten tracker-state tests and the pty tests naming `tracker.submitted`/`pwsh_list_view` read accessors. The throwaway differential test (prototype `differential_test.rs` against the pre-rewire `Tracker` kept as a module of a scratch copy, 36 000 random batches: equal events, tags, scrapes; never committed) passes. Interim composite until 9.2f. Windows check (deferral) mandatory |
| 9.2a | Adapters over the model | 2 | `adapters/headless_model.rs` (`type Model = Term<VoidListener>`; `overlay_renderer.rs` and the new adapters import it, the legacy files keep `types::Model` until 9.2f), `adapters/output_filter.rs` (`OutFilter` -> `OutputFilter`, `tracker.rs:283-470`, the 4 `output_filter_*` tests and the `filter` helper; `lf_to_ind` stays a `pub(crate)` field, R13). Test-first: `adapters/cell_tagger.rs` (`apply(model, ScreenCommand)`; 5 tests); the legacy `tag`/`mark_line_start` functions are deleted and the composite maps each `ScreenCommand` through `cell_tagger::apply`. P9-T19, T20 |
| 9.2b | The scrape | 2 | `adapters/edit_buffer_extractor.rs`: `extract(model, &EditBufferContext) -> Option<EditBuffer>`, private `Scraped` (the old `Extracted`), `matches_cell`, `cell_col`, `cell_emphasis`, `strip_pwsh_prompts` (`tracker.rs:248-254, 763-945`, a verbatim move); test-first: 5 tests with hand-built contexts; `suggestion_colors_cell_matching` moves here (`z.matches(&cell)` -> `matches_cell(&z, &cell)`); the composite builds the context from `marker.edit_buffer_context()` and calls `extract`. P9-T16, T17 |
| 9.2c | `FrameInput` to the application layer; renderer tests sliced | 2 | Commit 1 (pure): the renderer's inline test module -> `overlay_renderer/tests.rs` (`#[cfg(test)] mod tests;`; every test path unchanged). Commit 2 (RED/GREEN): `application/frame_input.rs` (`FrameInput` leaves `overlay_renderer.rs:28-39` and gains `now: Instant`), `menu_patches` passes `input.now` to `menu.loading()`, the spinner test of New tests 9.2c; `pty.rs`' `frame()` sets `now: Instant::now()` (interim); the test helper `input()` gains `now`. R15; P9-T22 |
| 9.2d | `ScreenState`, the `TerminalScreen` port, `AlacrittyTerminalScreen` | 2 | Test-first, no caller: `value_objects/screen_state.rs` (+ `ScreenStateParts`; 1 test), `application/ports/terminal_screen.rs` (eleven methods, no double: the real adapter is the double, R10), `adapters/alacritty_terminal_screen.rs` (`Inner { term, processor, filter, renderer }` in a `RefCell`; `new` with 2000 lines of scrollback, `advance_model`, `#[cfg(test)] for_tests/history_size/model/panic_while_borrowed`) and `alacritty_terminal_screen/tests.rs` (18 tests incl. the scrollback pin and the panic test). ONE borrow per method, `pass_through` appends to `out`, `resize` bundles model, filter and `drop_clear` in HEAD order. R10-R12; P9-T18, T21-T23, T26 |
| 9.2e | The screen switch | 2 | `pty.rs`: `Session` drops `term`, `renderer` and the tracker's `proc`/`filter` and holds `screen: Rc<dyn TerminalScreen>`, an `AlacrittyTerminalScreen` built AFTER the probe where the renderer is built today, then `advance_model(CUP)` (R12); `on_output`, `refresh`, `frame`, `redraw`, `on_resize`, `take_repaint`, `cleanup`, `guard`, both `set_theme` moments, `alt()`, `suppressed`, `drawable` call the port (a fresh `state()` per entry, R11); the composite's `feed` takes `&dyn TerminalScreen` and applies commands through `apply`; the `session()` fixture and the tracker rig build `for_tests` screens (three tracker tests read the port: `window_name_reaches_the_terminal_but_is_not_printed`, `extract_wrapped_long_command`, `extract_prompt_scrolled_by_output`). The riskiest step of the run: P9-T1-T3, T18, T21-T23, T26, T47, T48, T60 |
| 9.2f | `ShellOutputFeeder`; `tracker.rs` leaves | 2 | Test-first: `application/shell_output_feeder.rs` (`feed(splitter, tracker, batch, out) -> Vec<ShellEvent>` over the real screen; 9 tests). `Session` holds `splitter: MarkerSplitter` and `marker: MarkerTracker` itself (the composite is deleted) and `on_output` calls the feeder with the clock read before each tracker call; the 26 marker tests and the 3 tracker scenarios move to `alacritty_terminal_screen/tests.rs` (one `Rig { screen, splitter, tracker, feeder, events }`, R32; the six fixture-only tests are CHANGED by its constructor, the three guards are not touched: their helpers are); `src/tracker.rs`, `mod tracker;`, `types::Model` deleted. R14; P9-T1-T8 |

### Run 3: session domain (9.6a, 9.6b, 9.3a-9.3i)
Ends with `TerminalSession` and its six entities owning the state, `types::Msg` gone, the aggregate's fields private again; the legacy `Session` still performs every I/O call through delegates.

| Id | Title | Plan step | What it does |
|---|---|---|---|
| 9.6a | `SessionMessage`, the reader, the probe parsers | 6 | `infrastructure/pty/{mod.rs, session_message.rs, pty_reader.rs, terminal_probe.rs}` (`pub(crate) mod pty;` in `infrastructure/mod.rs`): `types::Msg` -> `SessionMessage` (variants and positional `Resize(cols, rows)` kept; deleted from `types.rs`, whose `CompletionResult` import goes with it; `pty.rs`, its two `sys` modules, `bootstrap/{engine_server, channel_completion_engine, engine_server/tests}.rs` updated); `pump`, `PTY_CREDITS` and the extracted `spawn_pty_reader(reader, replies) -> SyncSender<()>` (the credit channel and the reader thread, `pty.rs:499-504`; the writer thread at 505-516 stays in `start` and later in the launcher) to `pty_reader.rs`; `PROBE`, `PROBE_TIMEOUT`, `parse_probe`, `parse_cpr`, `parse_osc11` to `terminal_probe.rs` (`type Rgb` -> dropdown's alias); the tests `pty_reader_waits_for_credits` and `probe_replies` move. R29; P9-T76, T84. Windows check mandatory |
| 9.6b | Platform code; the starter alias | 6 | `infrastructure/pty/platform_unix.rs` (`#[cfg(unix)]`) and `platform_windows.rs` (`#[cfg(windows)]`): the two `sys` modules verbatim (`pty.rs:1319-1604`), declared with their attributes in `pty/mod.rs`; `pty.rs` selects them with cfg'd `use .. as sys` aliases (no re-export); `completion_engine_starter.rs` (`CompletionEngineStarter`, `pty.rs:74`); `signal_death_exit_code` moves (`sys::wait_pid` -> `wait_pid`). R29, R37; P9-T95. Windows check mandatory |
| 9.3a | Pure session services | 3 | `domain/services/{mod.rs, insertion_reconciler.rs, key_router.rs, session_deadlines.rs, render_guard.rs}` (`pub(crate) mod services;` in `domain/mod.rs`): `insertion_bytes`, `expected_buffer`, `unechoed_after` with their 3 tests and the `ins` helper; `ghost_key`, `is_ctrl` with `ghost_keys_and_words_keys` (`pub(crate)` while `on_keys` and `note_forwarded` still call them; `ghost_key` turns private at 9.3f when `route` takes over); `session_deadlines.rs` takes `RESIZE_DEBOUNCE`, `REDRAW_GRACE`, `SPINNER_TICK`, `FILE_POLL`, `EXIT_GRACE` (`INSERT_LOCK` and `LOCK_CAP` follow in 9.3c) and `next_deadline(passthrough, exit, pending)`; test-first `render_guard::allows_drawing(&ScreenState)` (the screen half of `drawable`, `pty.rs:989-1000`; 8 tests). `Session::drawable` = `!suppressed() && allows_drawing(&screen.state())`, `Session::next_deadline` calls the service (2 tests). A separate `test(terminal_session)` commit adds the parity test in `overlay_renderer/tests.rs` (the renderer draws exactly when the guard allows). R18-R19; P9-T47, T51, T52 |
| 9.3b | `WrappedShellCommand`; the key decoder as an entity | 3 | (a) `value_objects/wrapped_shell_command.rs`: `new(shell, extra, login)` is `argv` moved (`pty.rs:112-130`), `child_argv` restructured through it (CHANGED), test-first `in_session(version, in_tmux, &SessionId)` and `window_title()` (5 tests), `run`, `fallback` and `Session::start` use `argv()`, `env_to_set()`, `env_to_remove()` in the legacy order (`:479-486`, `:495-496`). (b) `git mv adapters/stdin_key_decoder.rs domain/entities/key_decoder.rs`, `StdinKeyDecoder` -> `KeyDecoder` (`decode`, `reset`, `#[cfg(test)] decode`), its 5 tests with it (R17); `pty.rs` imports it. Windows check mandatory. R26, R17; P9-T79 |
| 9.3c | `InsertionLock`, `EchoHold` | 3 | Test-first: `entities/insertion_lock.rs` (`InsertionLock`, `INSERT_LOCK`, `LOCK_CAP`) with `value_objects/lock_settlement.rs` (`LockSettlement`); 6 tests, `entities/echo_hold.rs` (`EchoHold`: `unechoed`, `echo_pending`, `held`, `held_at`; 8 tests); `Session` replaces its `lock`, `unechoed`, `echo_pending`, `held`, `held_at` fields by `lock: Option<InsertionLock>` and `echo: EchoHold` (settle, release and consume keep the legacy order); `poke_lock_due` and `poke_hold_due` are the only characterisation lines that change; `insertions_build_on_keys_not_yet_echoed` reads the entity. P9-T29, T33, T38, T41, T52, T56 |
| 9.3d | `GhostCandidates` | 3 | Test-first: `entities/ghost_candidates.rs` (`ghost`, `llm`, `llm_req`, `llm_at`; 5 tests); `Session` holds `ghost: GhostCandidates`; `poke_llm_due` changes; `llm_ghost_requested_for_commands_without_spec` and `should_show_the_llm_ghost_text_of_the_current_request_behind_the_local_one` use the entity methods. P9-T34, T35, T37, T65 |
| 9.3e | `ResizeDebounce`, `ShellExit`, `ReloadWatch` | 3 | Test-first: the three entities `entities/resize_debounce.rs` (`resize_at`/`repaint`), `entities/shell_exit.rs` (`exit_code`/`eof`/`exit_at`), `entities/reload_watch.rs` (`poll_at`/`settings_mtime`/`session_mtime`); 13 tests; `Session` holds `resize`, `exit`, `watch`; `settings_mtime` is still read at the end of `start` (P9-N6); `poke_resize_due` and `poke_poll_due` change; the five context tests read `context_modified()`, `panic_turns_the_session_into_a_pass_through` reads `reported_exit_code()`. Windows check mandatory (`step`'s exit arms). P9-T45, T46, T52 |
| 9.3f | `KeyRouter::route` | 3 | Test-first: `value_objects/{key_route.rs, key_facts.rs}` (`KeyRoute`, `KeyFacts` with `KeyFactsParts`) and `key_router::route` (+ `plain_char`, `ends_line`, `is_enter`; 14 tests); `Session::on_keys` replaces its match by `route(&key, &self.keymap, &facts)` with the facts built per key (P9-T66); the effect loop and everything else stay. R18; P9-T39-T41, T66 |
| 9.3g | The aggregate takes the state | 3 | Test-first (4 tests): `entities/terminal_session.rs` (`TerminalSession`, `TerminalSessionParts`, `LATE_PROBE`) and `value_objects/terminal_background.rs`; `Session` keeps only its ports, plumbing and a `session: TerminalSession` that now owns: settings, keymap, menu, `req`, `cur`, `sent`, `changed_at`, `redraw_until`, `spinner`, `spinner_at`, `late_probe_until`, `passthrough`, the six entities, the `MarkerTracker`, the `MarkerSplitter`, the `KeyDecoder`. Its fields are `pub(crate)` until 9.3i (R16), so every legacy method keeps its body with `self.session.x`; `on_output` takes the feeder's `output_parts()`. Tests naming moved fields (`s.menu`, `s.settings`, `s.req`, `s.drawable()`, `s.probe_until`) read aggregate accessors. R16, R25; P9-T50, T54, T58 |
| 9.3h | Aggregate methods: refresh, replies, drawing facts | 3 | Test-first (15 tests): `value_objects/completion_query.rs`; `refresh(screen, extracted, now) -> Option<CompletionQuery>`, `apply_completion(c, now) -> bool`, `accept_llm_reply`, `forget_sent`, `on_shell_event(event, now) -> bool`, `output_arrived`, `screen_changed`, `ghost_text`, `suppressed`, `drawable`, `basis`, `shell`, `tracked_cwd`; the legacy `refresh`, `on_completion`, `on_event`, `ghost_text`, `suppressed`, `drawable` become delegates that read `Instant::now()` and `screen.state()` (the clock table of "Signatures") and perform the engine calls the aggregate only reports. R16, R19; P9-T29-T37, T47, T49, T55 |
| 9.3i | Aggregate methods: keys, insertion, timers, exit | 3 | Test-first (1 + 4 tests): `decode_keys`, `hold_*`, `release_held_keys`, `route_key`, `run_menu_action`, `show_menu_on_tab`, `accept_ghost`, `forward_key`, `insert`, `wrote_to_shell`; the probe methods (`expect_late_probe_reply`, `late_probe_open`, `probe_background_reported`, `probe_cursor_reported`); `resize_applied`, `settle_resize_if_due`, `take_repaint`, `take_due_redraw_grace`; the timer methods (`insertion_lock_due`, `take_due_llm_request`, `tick_spinner(now, judged_at)`, `poll_due`, `context_changed`, `settings_changed`); the exit methods and `next_deadline`. The legacy `on_keys`, `insert`, `release_held`, `on_resize`, `on_timers`, `poll_files` bodies call them; **every aggregate field becomes private** (B8); the listed `#[cfg(test)]` setters serve the pokes. R16, R19; P9-T39-T45, T51-T58 |


### Run 4: ports and CLI (9.4a-9.4c, 9.6c)
Ends with the collaborators injected, the six ports adopted in `pty.rs`, `cli.rs` free of the debug code, and `pty.rs` free of rule-3 imports (dry run: placed as `infrastructure/pty/session_runner.rs` in a `fitness-mod.sh --worktree` copy it trips nothing; at HEAD it trips seven, P9-T98).

| Id | Title | Plan step | What it does |
|---|---|---|---|
| 9.4a | Collaborators injected; `GeneratorProcessReaper`; the fallback shell | 4 | Test-first: `application/ports/generator_process_reaper.rs` (`kill_live_children(&self)`, `RecordingGeneratorProcessReaper`; 1 test) and `bootstrap/bridges.rs` + `GeneratorProcessBridge` over the `specs` global `kill_live_children()` (a one-line bridge, no test). `infrastructure/pty/fallback_shell.rs`: `run_fallback_shell(error, argv, reaper)` from `fallback` (`pty.rs:133-157`, the `cfg(unix)` exec and `cfg(windows)` spawn lines verbatim). `terminal_session_module.rs`: `build_terminal_session_module(&deps, settings, themes, generator_processes, start_completion_engine)`, getters `settings_repository()` (interim: dies at 9.4c), `themes()`, `generator_processes()`, `session_contexts()`, `start_completion_engine()`, `clock()`; its module test builds with in-memory collaborators (CHANGED). `bootstrap/cli.rs`: `pub(crate) fn terminal_session_module()` builds the collaborators once and serves the pty, pre-cmd and debug arms (pre-cmd keeps its behaviour: three `Rc`s, no I/O; `precmd-diff.sh`); `cli::debug(c, &module)` and `pty::run(shell, cmd, &module)`; `Session::start` and `cleanup` read the repositories and the reaper from the module, so `pty.rs` loses its `FileThemeRepository`, `JsonFileSettingsRepository`, `FileSessionContextRepository` and `kill_live_children` imports. R22, R28; P9-T91, T92, T98. Windows check mandatory |
| 9.4b | `TerminalWriter`, `WrappedShell`, the journal | 4 | Test-first: `ports/terminal_writer.rs` (`write(&self, &[u8])`, `CapturingTerminalWriter`; 3 tests) and `ports/wrapped_shell.rs` (`write_input(Vec<u8>)`, `resize(columns, rows)`, `CapturingWrappedShell`; 3 tests), `application/test_fixtures.rs` (`#[cfg(test)] pub(crate) mod test_fixtures;` with `IoJournal` and its enum; every infrastructure `use` in it carries its own `#[cfg(test)]`, K12), `adapters/stdout_terminal_writer.rs` (`write_stdout` verbatim: nothing for an empty slice, errors ignored), `adapters/pty_wrapped_shell.rs` (`PtyWrappedShell::new(input, master)`, `write_input`, `resize` as `PtySize { rows, cols, 0, 0 }`, inherent `release_pseudo_console(&self)`; 4 tests). `pty.rs`: `emit` (its `if cfg!(test)` branch dies, P9-T73), `write_stdout`, `sink`, the writer-thread sender and `master` become `output: Rc<dyn TerminalWriter>` and `pty: Rc<PtyWrappedShell>`; the `session()` fixture builds the capturing doubles; the six `s.sink` readers (`identical_redraw_writes_nothing`, `no_frame_inside_an_open_dcs`, `panic_turns_the_session_into_a_pass_through`, `enter_clears_the_menu_before_the_line_runs`, `inline_tui_below_the_prompt_is_not_the_line`, `pwsh_list_view_owns_the_rows_below`), every `sent(&pty)` and `conpty_is_closed_off_the_main_loop` (a `PtyWrappedShell` over its fake master) read the doubles. R21-R23; P9-T63, T73, T84, T88-T90. Windows check mandatory; `fitness-mod.sh --worktree` after the commit proves the test-support file is clean |
| 9.4c | `TerminalModes`, `LiveSettings`, `WorkingDirectory` | 4 | Test-first: the three ports with `RecordingTerminalModes`, `InMemoryLiveSettings`, `FixedWorkingDirectory` and the adapters `console_terminal_modes.rs` (`ConsoleTerminalModes::new(Modes)`; a console call, no test), `lenient_live_settings.rs` (`LenientLiveSettings::new(Rc<dyn SettingsRepository>)`; `load` logs `settings: {chain}` at warn and returns the defaults; 4 tests), `process_working_directory.rs` (1 test). `pty.rs`: `modes` -> `Rc<dyn TerminalModes>` (still set on the line after `sys::enter`, P9-T80), `settings_repository` + `load_or_defaults` -> `LiveSettings`, the `std::env::current_dir()` fallback of `cwd()` -> `WorkingDirectory::current()` (read at every request, P9-T71); the module's interim `settings_repository()` getter dies and `live_settings()` arrives. The `Modes::default()` clippy lint (`pty.rs:1801`) dies with the field. Rule-3 dry run: 0 hits. R22; P9-T80, T91 |
| 9.6c | The CLI commands move | 6 | `infrastructure/cli/{pty_command.rs, key_tester_command.rs, debug_shell_command.rs}`: `run_pty_command(shell, cmd, module)` (still calls the legacy `pty::run`: with `debug_shell_command.rs` the one interim infrastructure-to-legacy import, which B1 and A3 list until 9.6d, as Phase 8 left `crate::types::Model` in the renderer), `run_key_tester_command()` (`key_line` + `key_tester` verbatim: banner before raw mode, `\r\n` lines, ends on exactly `[4]` or EOF, `disable_raw_mode()?` before `res?`), `run_debug_shell_command(shell, module, render_post_script)` (`debug_shell` + `debug_shell_files` verbatim: own 0700 `DirBuilder`, plain `fs::write`, NOT `AtomicFile`; the data root from `module.locate_data_root()` AFTER the render; the two `set_var` calls before the session; `let _ = writeln!`). `bootstrap/cli.rs`: `DebugCmd` moved verbatim (variants, attributes, docs, order); the Pty and Debug arms call the commands; the post-script callback is the helper `pub(crate) fn debug_shell_post_script(shell: Shell) -> anyhow::Result<String>` built there from `RenderInitScriptUseCase::new(Rc::new(OsShellEnvironment), Rc::new(EmbeddedShellScripts))` and `init_script(.., IntegrationPhase::Post)`; the `DebugCmd::Shell` arm and the moved test both pass it. `cli.rs` loses `DebugCmd`, `debug`, `key_line`, `key_tester`, `debug_shell`, `debug_shell_files` and 3 tests; `debug_logs` becomes `pub fn`; the `crate::pty` imports die. The module gains `locate_data_root()` (test-first, 1 test). D9: `NAME` is the new libtest path of `debug_shell_survives_closed_terminal` and its child branch calls `run_debug_shell_command(Some(Shell::Bash), &crate::bootstrap::cli::terminal_session_module(), ..)`. R33; P9-T93, T94, T100. Windows check mandatory |

### Run 5: services and use cases (9.5a-9.5h)
Ends with every `Session::on_*` body gone: `Session` holds `SessionUseCases`, the aggregate and the plumbing; 24 tests moved to the use cases (5 + 16 + 3).

| Id | Title | Plan step | What it does |
|---|---|---|---|
| 9.5a | `OverlayPainter`; `SessionPortsParts`; `SessionUseCases`; the harness | 5 | Test-first: `application/overlay_painter.rs` (`frame(&self, &TerminalSession)`, `redraw`, `take_repaint`; `now` read LAST; 5 tests), `session_ports_parts.rs` (11 fields), `session_use_cases.rs` (`SessionUseCases::new(ports)`, holding the painter so far), and `TerminalSessionHarness` + `SID` in `test_fixtures.rs` (the real `AlacrittyTerminalScreen::for_tests(80, 24)`, the journaled capturing doubles, `RecordingCompletionEngine` with its receiver, `InMemoryLiveSettings`, `RecordingGeneratorProcessReaper`, `RecordingTerminalModes`, `SystemClock`, built through `SessionUseCases::new`, R31). `pty.rs`: `Session` holds `use_cases: SessionUseCases` built from `SessionPortsParts` (in `start` and in the `session()` fixture, now rebuilt over the harness parts); `frame`, `redraw`, `take_repaint` become delegates. In the `Rig` only the `s.redraw()` line is re-pointed. R20, R28, R31; P9-T22, T47, T68 |
| 9.5b | `CompletionRequester`, `SessionFilePoller` | 5 | Test-first: `completion_requester.rs` (`refresh(&self, &mut TerminalSession)`, `working_directory_of`; 7 tests) and `session_file_poller.rs` (`poll(&self, &mut TerminalSession) -> bool`; 6 tests); the legacy `refresh`, `cwd` and `poll_files` delegate; the five context tests move to `session_file_poller.rs` (`poller.poll(&mut h.session)`; harness `with_session_contexts(SessionId "abc", Rc<FileSessionContextRepository>)`). R20, R22; P9-T29-T31, T44, T71, T72, T91 |
| 9.5c | `KeystrokeHandler`, `HandleKeystrokesUseCase` | 5 | Test-first (14 tests, three of them pin the cross-port order through `IoJournal`): `keystroke_handler.rs` (`handle`, `release_held`; the `#[cfg(test)] maybe_panic()` hook is the FIRST statement of the per-key loop, the PTY write the LAST) and `handle_keystrokes_use_case.rs`; `test_fixtures.rs` gains `maybe_panic`, `set_panic_in_key_handling`, `subs`, `shell_line`; the legacy `on_keys`, `insert`, `release_held` delegate; the thread-local `PANIC_IN_INPUT` goes (`panic_while_handling_keys_still_forwards_them` calls `set_panic_in_key_handling`). R21, R31; P9-T39-T42, T56, T58, T62-T64, T66 |
| 9.5d | `ProcessShellOutputUseCase` | 5 | Test-first (7 tests): `process_shell_output_use_case.rs` + `ProcessShellOutputParts` (nine collaborators); the order of `pty.rs:758-780`: `output_arrived`, `before` state, feed, events (engine first, then `on_shell_event`, then the poll), `screen_changed`, `refresh`, `take_clear`, `take_repaint`, frame, ONE write, then `release_held`; the legacy `on_output`/`on_event` delegate. R21; P9-T59-T61, T70, T99 |
| 9.5e | `ApplyCompletionResult`, `ApplyLlmGhostText`; the first test wave | 5 | Test-first (9 tests): both use cases and their entries in `SessionUseCases`; the legacy `on_completion` and the `Ghost` arm of `step` delegate. **16 tests move** with harness reads (`h.written()`, `h.clear_written()`, `h.engine_calls()`, `request_id()`, `edit_buffer()`): the eight keystroke tests to `handle_keystrokes_use_case.rs`, five to `process_shell_output_use_case.rs` (`prompt_redraw_keeps_the_menu`, `no_frame_inside_an_open_dcs`, `prompt_redraw_does_not_reshow_a_hidden_menu`, `no_history_command_still_ends`, `inline_tui_below_the_prompt_is_not_the_line`), `identical_redraw_writes_nothing` to `overlay_painter.rs`, `llm_ghost_requested_for_commands_without_spec` to `apply_completion_result_use_case.rs`, `should_show_the_llm_ghost_text_of_the_current_request_behind_the_local_one` to `apply_llm_ghost_text_use_case.rs`. In the `Rig` the `on_completion` and `Ghost` lines are re-pointed. R20; P9-T55, T65 |
| 9.5f | `ResizeTerminal`, `HandleShellExit` | 5 | Test-first (10 tests): `resize_terminal_use_case.rs` (ignore 0 and the MODEL's size; `shell.resize` before `screen.resize`, columns first, then `resize_applied`), `handle_shell_exit_use_case.rs` + `shell_end.rs` (`execute(&mut TerminalSession, ShellEnd) -> bool` with the bools of `pty.rs:650, 672-676, 679`); the legacy `on_resize` and the `ChildExit`/`PtyEof`/`Term` arms of `step` delegate, `step` keeps the ConPTY `release_pseudo_console()` call before the use case (R23). R24; P9-T21, T46, T87, T88. Windows check mandatory |
| 9.5g | `HandleTimerTick`; the second test wave | 5 | Test-first (9 tests): `handle_timer_tick_use_case.rs` in the exact order of `pty.rs:1224-1269` (resize settle, redraw grace and refresh, `release_held`, lock due evaluated AFTER the release, LLM request, spinner at a fresh instant, file poll with the tick's `now`, THEN repaint and frame in one write, else a plain redraw: the poll can reload the settings and the theme, so the frame is built after it; P9-T67, pty.rs `on_timers`); the legacy `on_timers` delegates. Three tests move with the real clock: `split_prompt_redraw_keeps_the_menu` (`process_shell_output_use_case.rs`), `keys_after_an_unmodelled_key_wait_for_its_echo` (`handle_keystrokes_use_case.rs`), `slow_echo_keeps_the_insertion_in_the_basis` (`handle_timer_tick_use_case.rs`). R19; P9-T45, T51, T52, T67, T68, T75 |
| 9.5h | `EndSession`, `ApplyTerminalBackground`, `PrepareSessionLaunch` | 5 | Test-first (12 tests): `end_session_use_case.rs` (ONE write of clear + `\x1b[0m\x1b[?25h`, `shut_down`, `kill_live_children`, `modes.restore`, remove the context, in that order), `apply_terminal_background_use_case.rs` (remember the colour, `set_theme(resolve_theme(..))` unconditionally), `prepare_session_launch_use_case.rs` + `launch_request.rs` + `launch_environment.rs` (shell resolution with `cfg!(windows)` inside; two cfg'd tests). The legacy `cleanup` delegates; `on_input`'s late-probe branch calls `ApplyTerminalBackground`; `run` resolves the shell through the module's `prepare_session_launch` after reading the env facts BEFORE `catch_unwind` (`env_nonempty` for `TABSMITH_SHELL` and `SHELL`, `TABSMITH_IS_LOGIN_SHELL == "1"` for the login flag, `pty.rs:80-90`). After this step the legacy `Session` has no `on_*` body. R24-R26; P9-T54, T69, T79. Windows check mandatory |

### Run 6: runner and close (9.6d-9.6f, 9.7)
Ends with `src/pty.rs` gone and Phase 9 recorded.

| Id | Title | Plan step | What it does |
|---|---|---|---|
| 9.6d | `pty.rs` becomes `SessionRunner` | 6, 7 | `git mv src/pty.rs src/terminal_session/infrastructure/pty/session_runner.rs`; `Session` -> `SessionRunner` built from `SessionRunnerParts { inbox, credits, pty, session, screen, output, use_cases, clock }` (field order = the HEAD drop order, R30); `run`, `step`, `guard`, `on_input`, `next_deadline` keep their signatures; `mod pty;` leaves `main.rs`, `pub(crate) mod session_runner;` joins `pty/mod.rs`; `pty_command.rs` and `debug_shell_command.rs` call `session_runner::run`; the file still holds `Session::start`/`run` (9.6e) and its tests inline (9.6f). The architecture test now reads the file and passes (dry run of 9.4c). `LEGACY_MODULES` keeps `pty`. R30, R38; P9-T53, T86, T89, T90. Windows check mandatory |
| 9.6e | The launcher and `run_pty_session` | 6 | Test-first: `terminal_probe.rs::probe_terminal(output, timeout, read_timeout)` (4 tests; the probe loop leaves `start`), `pty_session.rs` (`run_pty_session`, `env_nonempty`, `attempt`; the panic hook FIRST; 3 tests: the outer panic tier), `session_launcher.rs` (`launch_session(command, &mut modes, module)`: `pty.rs:401-576` line for line in HEAD order with the screen built after the probe (R12), filling `SessionPortsParts` and calling `SessionUseCases::new`, R28; `generate_session_id()` with its test). `Session::start` and `run` are deleted; the CLI commands call `run_pty_session`. R25, R26, R28, R30; P9-T78-T83, T85. Windows check mandatory; `debug_shell_survives_closed_terminal` x3 |
| 9.6f | The driver tests move | 6, 7 | Tests move with their names: the four driver tests (`late_probe_reply_is_not_typed`, `panic_turns_the_session_into_a_pass_through`, `panic_while_handling_keys_still_forwards_them`, `conpty_is_closed_off_the_main_loop`) and the 13 loop tests with the scenarios 11 (`probe`), 12 (`passthrough`) and 14 (`batching`) to `session_runner/tests.rs` (their fixtures build the runner from the harness parts); scenarios 1-10 and 13 to `application/session_use_cases/tests.rs` (only the `Rig` driver lines change); the legacy `session()` fixture and helpers are deleted; the four `settings_catalog.rs` comments naming `pty.rs` (lines 345, 355, 375, 441) and the `architecture_tests.rs:376` comment example are repointed (R39). Every A check is OK. P9-T96 |
| 9.7 | Close | 7 | R39: plan status and sections 2, 4, 5, 7, 8 and 9 ("Phase 9 notes", the deviation list above), glossary rows (table above), `README.md` (status, the four scripts), `handoff.md`, this file's Status; `cargo test --locked --release -- --ignored all_bundle_modules`; `cargo check --locked --no-default-features --all-targets`; the full greps; the orchestrator asks the user for the manual smoke (O14) |

### Per-step guards and test-lock expectations

"Guards" are on top of the gate and the three testlock runs. `W` = the windows check is mandatory (it runs inside the gate anyway). `S` = `pty-diff.py` and `session-diff.py` old versus new binary, both empty: every step from 9.1e on carries it except 9.1c, 9.1d and 9.2d (code without a caller), 9.6b (platform files moved) and 9.6c (scenarios `keytester`, `failures`, `files` only). `D` = `cargo test --locked debug_shell_survives_closed_terminal` alone, three times: steps that touch `Session::start`, the probe, the reader, the screen, the module or `argv`. `X` = 20 serial runs of the pty and characterisation tests.
"CHANGED" lists the first time a name changes against `phase9-start`; a name changed again later is not repeated; PATH-ONLY lines are informational.

| Id | Guards | NEW | CHANGED (first time) |
|---|---|---|---|
| 9.0a | baseline equality of both scripts, twice; `fitness-mod.sh terminal_session` 55 passed | 0 | none |
| 9.0b | helper tests; fixtures read; pass on the old code | 6 | none |
| 9.0c | X; 4 deterministic runs of the fixtures | 14 | none |
| 9.0d | X; then the tag `phase9-guards` | 14 | none |
| 9.1a | the 38 tracker tests; both halves of the two splits | 6 | `suggestion_colors`, `pwsh_reported_prediction_colour`; PATH-ONLY: the 7 splitter tests |
| 9.1b | tracker and pty tests | 3 | `extract_ghost_after_cursor_vs_typed_grey`, `bash_redraw_of_the_prompt_alone_keeps_the_line`, `at_prompt_only_on_the_command_line`, `soft_wrap_redrawn_with_crlf_is_still_the_line`, `prompt_ending_in_a_newline_is_at_the_prompt`, `prompt_drawn_without_markers_is_at_the_prompt` |
| 9.1c | none beyond the new tests (no caller) | 13 | none |
| 9.1d | none beyond the new tests (no caller) | 32 | none |
| 9.1e | W; S; X; the throwaway differential test; the tracker and `lifecycle` fixtures | 0 | the ten tracker-state tests (`markers_stripped_from_output`, `lock_and_foreign_session_ids`, `bare_newcmd_cannot_fake_a_prompt_while_locked`, `command_cycle_produces_history`, `no_history_marker_drops_the_command`, `submitted_line_drops_shell_output_before_preexec`, `transient_prompt_before_preexec_keeps_the_submitted_line`, `nested_shell_prompt_keeps_outer_command_running`, `pwsh_list_view_rows_not_in_buffer`, `windows_deferral_fifo`), `enter_records_the_submitted_line`, `pwsh_list_view_owns_the_rows_below` |
| 9.2a | S; tracker tests and fixtures | 5 | PATH-ONLY: the four `output_filter_*` tests |
| 9.2b | S; the extract tests and fixtures | 5 | none |
| 9.2c | S; the 10 renderer tests unchanged in value (`should_draw_the_overlay_bytes_it_drew_at_the_phase_8_start` holds the ten byte-hash scenarios), all pty tests | 1 | none (module path unchanged) |
| 9.2d | none beyond the new tests (no caller) | 19 | none |
| 9.2e | S; D; X; W; every fixture | 0 | `window_name_reaches_the_terminal_but_is_not_printed`, `extract_wrapped_long_command`, `extract_prompt_scrolled_by_output` |
| 9.2f | S; X; every fixture; A: no `crate::tracker`; F1 | 9 | `extract_zsh_prompt_and_command`, `extract_ignores_rprompt`, `extract_ps2_multiline`, `extract_excludes_completion_listing_below`, `extract_keeps_blank_skipped_before_cursor`, `pwsh_continuation_and_prompt_text_stripped` (the `Rig` constructor); PATH-ONLY: the other marker tests |
| 9.6a | S; D; W; the engine-server tests | 0 | `pty_reader_waits_for_credits`; PATH-ONLY: `probe_replies` and the `engine_server` tests naming `Msg::..` |
| 9.6b | W | 0 | PATH-ONLY: `signal_death_exit_code` |
| 9.3a | S; X; every fixture | 11 | PATH-ONLY: `insertion_bytes_reconcile`, `expected_buffer_after_insertion`, `unechoed_keys_are_consumed_by_their_echo`, `ghost_keys_and_words_keys` |
| 9.3b | S; D; W; X | 5 | `child_argv`; the five decoder tests (`decode_table`, `decode_multi_key_chunk`, `esc_not_last_and_double_esc`, `paste_across_chunks`, `keymap_defaults_and_overrides`): PATH-ONLY, or CHANGED where a body names the bare type |
| 9.3c | S; X; fixtures `lock`, `held`, `typing` | 14 | `insertions_build_on_keys_not_yet_echoed` |
| 9.3d | S; X; fixtures `llm`, `ghost` | 5 | `llm_ghost_requested_for_commands_without_spec`, `should_show_the_llm_ghost_text_of_the_current_request_behind_the_local_one` |
| 9.3e | S; W; X; fixtures `resize`, `lifecycle`, `reload`, `batching` | 13 | the five context tests, `panic_turns_the_session_into_a_pass_through` |
| 9.3f | S; X; fixtures `typing`, `bell`, `held`, `ghost` | 14 | none |
| 9.3g | S; D; X; every fixture | 4 | the `Session` tests that name a field the aggregate took (the compiler lists them; each is in "Test moves") |
| 9.3h | S; X; every fixture | 15 | none new |
| 9.3i | S; X; B8 empty of aggregate fields | 5 | `Session` tests still naming a field or probe state (`late_probe_reply_is_not_typed`, `panic_while_handling_keys_still_forwards_them`) |
| 9.4a | S; D; W; the four older diff scripts; X | 1 | `should_report_the_context_into_the_run_files_of_the_data_directory_it_was_built_with` |
| 9.4b | S; D; W; X; `fitness-mod.sh --worktree` | 10 | `identical_redraw_writes_nothing`, `no_frame_inside_an_open_dcs`, `enter_clears_the_menu_before_the_line_runs`, `inline_tui_below_the_prompt_is_not_the_line`, `conpty_is_closed_off_the_main_loop` (Windows only), `pwsh_list_view_owns_the_rows_below` and `panic_turns_the_session_into_a_pass_through` where not changed before |
| 9.4c | S; D; W; X; the rule-3 dry run | 10 | none expected (fixture only) |
| 9.6c | D; W; the four older diff scripts; `session-diff.py` scenarios `keytester`, `failures`, `files` | 1 | `debug_shell_survives_closed_terminal`; PATH-ONLY: `key_tester_line`; unchanged: `debug_shell_rc_dir_and_quoting` |
| 9.5a | S; D; X; every fixture | 5 | none |
| 9.5b | S; X; the context tests | 13 | none new (the five are already CHANGED) |
| 9.5c | S; X; the panic tests | 14 | `panic_while_handling_keys_still_forwards_them` if not changed before |
| 9.5d | S; X; every fixture | 7 | none |
| 9.5e | S; X; every fixture | 9 | any of the 16 moved tests not changed before |
| 9.5f | S; W; X | 10 | none |
| 9.5g | S; X, and the three real-clock tests alone 20 times | 9 | `split_prompt_redraw_keeps_the_menu`, `keys_after_an_unmodelled_key_wait_for_its_echo`, `slow_echo_keeps_the_insertion_in_the_basis` |
| 9.5h | S; D; W; X | 12 | none |
| 9.6d | S; D; W; X; the architecture test reads the file | 0 | none (PATH-ONLY for every test left in the file) |
| 9.6e | S; D; W; X | 7 | none |
| 9.6f | S; W; X; every A check | 0 | PATH-ONLY: the moved tests |
| 9.7 | the full run-end list; every grep | 0 | none |

Totals at the end, against `phase9-start`: MISSING 0, DUPLICATE 0, NEW 321, CHANGED = the justified list of "Test moves" (38 tracker + 36 pty + cli + the module test; no assertion changed). The Linux pass count is 1411 (1091 + 321 - 1 Windows-only name).

### Acceptance greps (9.7; the script's labels; run `scripts/hexagonal-refactor/phase9-greps.sh phase9-start` and read every REVIEW)

| Label | Check | Expected | Required from |
|---|---|---|---|
| A1, A2 | `src/pty.rs` and `src/tracker.rs` deleted; `main.rs` declares neither | OK | 9.6d (tracker alone from 9.2f) |
| A3 | no `crate::pty`, `crate::tracker`, `pty::`, `tracker::` path (a preceding `:` or `_` excluded, so `infrastructure::pty::..` and `portable_pty::` pass; fix at 9.0a) | OK | 9.6d (`crate::tracker` from 9.2f) |
| A4 | no comment names `pty.rs` or `tracker.rs` as a file | OK | 9.6f |
| A5, A6 | no `types::Msg`, `types::Model`, `Msg`; `types.rs` holds neither nor an import only they used (fix at 9.0a: drop the `\bModel\b` alternative, `headless_model::Model` is legitimate) | OK | 9.6a (Model from 9.2f) |
| B1, B2 | no legacy flat module, no `shell_integration`, `diagnostics`, `bootstrap` in `terminal_session` production | OK | 9.2a (B1 lists the two `crate::pty::run` calls of 9.6c until 9.6d) |
| B3 | no `specs` in production (the test-only edge is allowed) | OK | every step |
| B4 | other modules' application/infrastructure: only `CompletionRequest` | REVIEW | every step |
| B5 | no use case of another module | OK | every step |
| B6 | no `pub use`/`pub(crate) use` in `terminal_session`, `bootstrap`, `cli.rs` | OK | every step |
| B7 | nothing wider than `pub(crate)` | OK | every step |
| B8 | domain fields: only `*Parts` structs `pub(crate)` | REVIEW | 9.3i |
| B9 | `anyhow`, `clap` only in `infrastructure/cli` and `infrastructure/pty` | OK | every step |
| B10 | `alacritty_terminal`, `portable_pty`, `crossterm`, `libc`, `signal_hook`, `windows_sys` never in domain/application | OK | every step |
| B11, B12 | no `serde`, `json!`, print, logger in domain/application | OK | every step |
| B13 | no I/O, clock, thread or channel in domain/application | OK | every step |
| B14, B15 | threads and channels only in `infrastructure/pty`; `Rc<dyn ..>` holders are use cases, services and the runner | REVIEW | 9.6e |
| B16 | more than one `pub(crate)` type per file: only `*Parts`, a trait with its double and call enum, a private helper | REVIEW | every step |
| B17 | a bodyless `#[cfg(test)]` item whose line holds an unpaired bracket (write `\x5b` in byte literals) | REVIEW | every step |
| C1 | the sorted cfg multiset of `pty.rs`, `tracker.rs`, `cli.rs`, `bootstrap`, `terminal_session` against `phase9-start`: a move shows nothing; REMOVED `if cfg!(test) {` is the writer port (9.4b); ADDED `cfg(not(windows))` / `cfg(windows)` for the two 9.5h tests | REVIEW | every step |
| D1 | exactly one production call of `maybe_panic`, in `KeystrokeHandler::handle`'s per-key loop | REVIEW | 9.5c |
| D2 | exactly one `#[ignore]` in the tree | REVIEW | every step |
| D3 | the D9 string names `terminal_session::infrastructure::cli::debug_shell_command::tests::debug_shell_survives_closed_terminal` | REVIEW | 9.6c |
| E1 | `terminal_session` in `REGISTERED_MODULES` | OK | 9.0a |
| F1 | `grep -rnE '\b(KeystrokeHandler\|CompletionRequester\|OverlayPainter\|SessionFilePoller\|ShellOutputFeeder\|SessionUseCases)\b' src \| grep -v '^src/terminal_session/' \| grep -v '^src/pty.rs:'` | empty | every step from its creator (the legacy `src/pty.rs` holds a `ShellOutputFeeder` from 9.2f and `SessionUseCases` from 9.5a until its `git mv` at 9.6d, so its lines are excluded; after 9.6d the last filter matches nothing) |
| F2 | `grep -rnE '\b(OutFilter\|StdinKeyDecoder\|Extracted\|TAG_PROMPT\|Splitter)\b\|\bPiece::' src` | empty | 9.2f (`StdinKeyDecoder` from 9.3b); the `\b` before `Piece::` is required, or every `OutputPiece::Bytes` of the new code matches |
| F3 | `grep -rn 'write_stdout' src`: only `stdout_terminal_writer.rs`; `grep -rn 'std::io::stdout(' src/terminal_session`: only that file, `key_tester_command.rs`, `debug_shell_command.rs` and `session_launcher.rs` (the `is_terminal()` check that opens `Session::start`, `pty.rs:402`, moves there line for line) | as stated | 9.6e |
| F4 | `grep -rn 'SessionUseCases::new' src`: only `session_launcher.rs` and `test_fixtures.rs` (the wiring the harness shares) | as stated | 9.6e |
| F5 | `grep -rn 'catch_unwind' src/terminal_session \| grep -v '/tests.rs'`: only `session_runner.rs` (`guard`) and `pty_session.rs` (`attempt`) in production code; test modules (`alacritty_terminal_screen/tests.rs` for the panic test of 9.2d, `session_runner/tests.rs`, `pty_session` tests) may call it, and inline `#[cfg(test)]` modules are read by eye | as stated | 9.6e |
| Z | `git ls-files src/terminal_session \| sort` equals the file list of "Module map" (read by eye; the table names every file) | equal | 9.7 |


## Open questions (for the plan owner; the default applies if nobody answers)

1. **O1 Application services (R20).** `KeystrokeHandler`, `CompletionRequester`, `OverlayPainter`, `SessionFilePoller` and `ShellOutputFeeder` are services shared by several events, not use cases, so "a use case never calls another use case" holds. Acceptable? *Default: yes (the skill's naming table lists `Service`; rule 3 is cross-module only; F1 pins that nothing outside `terminal_session` names them).*
2. **O2 Size of `TerminalSession` (R16).** About 575 production lines and 60 methods after 9.3i. Split now (a keystroke-specific object for `route_key`, `insert`, `forward_key`) or later? *Default: one aggregate now (they read eight of its fields); a Phase 11 watch-list item.*
3. **O3 A `PreExecTicket` token for the two-call `PreExec` (R5).** It would make a stray `complete_pre_exec` unrepresentable. *Default: no (one caller, one more type).*
4. **O4 Dedupe `rgb_to_256` with `dropdown`'s `xterm256` (R40).** Proven equal for all 2^24 inputs, but it edits two locked tests' call sites. *Default: Phase 11; Phase 9 moves the function as it is.*
5. **O5 `Rc<dyn ..>` for the session's ports.** The session runs on one thread (the reader, writer, input, resize and engine threads only send `SessionMessage`s or bytes through channels). *Default: `Rc`; switch to `Arc<dyn .. + Send + Sync>` if a step finds a thread boundary.*
6. **O6 A section 2 module-edge rule (R3).** About 60 lines and four tests of checker code (`ALLOWED_EDGES`); production edges were verified by script. *Default: not in Phase 9; Phase 11 ("register all modules, no exceptions"); grep B3 pins the one edge that matters here.*
7. **O7 A `TerminalModes` port (R22)** versus restoring the modes after `EndSessionUseCase` (an unobservable reorder). *Default: the port, because the constraint list names write order.*
8. **O8 `Term` through `HandleShellExitUseCase` (R24)** versus the plan table's `EndSession`. *Default: `HandleShellExit` records 129 and stops the loop; `EndSession` runs once after the loop for every exit.*
9. **O9 The screen is built after the probe (R12, deviation 19).** Only in-memory work moves; the `\x1b[H\x1b[2J` write stays before `kitty_graphics::detect()`. *Default: yes.*
10. **O10 `FrameInput.now` (R15).** One commit (9.2c); skipping it changes nothing else. *Default: add; fallback if it proves noisy: keep the renderer reading `Instant::now()` and drop the spinner test.*
11. **O11 The debug shell's post script as a bootstrap closure (R33)** versus a `PostIntegrationScript` port. *Default: the closure (it keeps `anyhow` out of a port; precedent `print_root_help`).*
12. **O12 The two diff scripts in the gate (R35).** `pty-diff.py` ~12 s, `session-diff.py` ~85 s and tmux. *Default: no; they run at the steps of the guard section and at every run end.*
13. **O13 `LEGACY_MODULES` keeps `pty` and `tracker` (R38).** *Default: keep (harmless, `ponytail:` comment); the Phase 11 sweep drops every dead name.*
14. **O14 The manual smoke** (plan section 8 items 1-9 on zsh, bash and fish, Phase 8's P8-S1/S2, Phase 9's P9-S1..S5) is asked once after 9.7, and nothing is pushed or opened as a PR (R43). *Default: once, because agents cannot drive a terminal and the golden capture does not reach the session.*
15. **O15 Where the 26 marker tests live (R32).** *Default: `alacritty_terminal_screen/tests.rs`, one `Rig`, because they are pipeline tests over the real adapter; testlock keys by name, so the tracker-state-only ones could move later.*
16. **O16 The spinner scenario's 250 ms sleep (R34).** The only wall-clock wait of the guards (a floor, so it cannot flake). *Default: accept.*
17. **O17 `TerminalSessionParts.id` is a required `SessionId` (R27)** where HEAD held an `Option` that existed only because `SessionId::new` is fallible. *Default: required; the 28 tests that ran with `None` use `SID` against an in-memory repository that holds nothing.*
18. **O18 `tick_spinner(now, judged_at)` keeps two instants (R19).** *Default: two (exact at no cost); collapse if the reviewer prefers.*

## Launch

From the repo root on `refactor/hexagonal-dropdown-terminal-session` (the branch the request names; no new branch), with the gate green at `5efb4b2` (K3):
1. Re-check the DEFERRED items (K1, K8) and the tree (K4, K16): `git status` must be clean for 9.0a; the research notes under `target/hexagonal-refactor/p9/` stay in place (implementers read and lift from them).
2. Copy this file to `docs/hexagonal-refactor/phase-9-terminal-session.md`, write `docs/hexagonal-refactor/phase-9-terminal-session.args.json` (the shape of `phase-8-dropdown.args.json`: `phase`, `startRev`, `reviewNotes`, and `steps[{ id, title, run, prompt }]` with one prompt per step written from the tables above: the step's row, the traps and decisions it cites, the signatures it builds, the tests it moves and creates, its guards and
   expected testlock line, and the checklist of R45), and commit both as `docs: hand Phase 9 over with its research` with `git commit -F`. The tag `phase9-start` stays at `858b7b5` (the testlock baseline of every step; the docs commit changes no code).
3. Run the workflow once per run, each time with the run's steps only, `branch` = `refactor/hexagonal-dropdown-terminal-session` and `startRev` = the HEAD the run starts from: run 1 = `9.0a 9.0b 9.0c 9.0d`; run 2 = `9.1a 9.1b 9.1c 9.1d 9.1e 9.2a 9.2b 9.2c 9.2d 9.2e 9.2f`; run 3 = `9.6a 9.6b 9.3a 9.3b 9.3c 9.3d 9.3e 9.3f 9.3g 9.3h 9.3i`;
   run 4 = `9.4a 9.4b 9.4c 9.6c`; run 5 = `9.5a 9.5b 9.5c 9.5d 9.5e 9.5f 9.5g 9.5h`; run 6 = `9.6d 9.6e 9.6f 9.7`. Between runs the orchestrator runs the run-end list (Steps), sets `phase9-guards` after run 1, and writes a binding "SINCE" preamble into each step prompt of the next run (what the earlier runs left: the layout as built, signatures as built, the
   gate/testlock/diff numbers, the accepted deviations) and a "SCOPE OF THIS RUN" preamble for the reviewers (judge only this run's steps; do not demand later runs' work).
4. `reviewNotes` (the reviewers' contract): Phase 9 builds `terminal_session` out of `src/pty.rs`, `src/tracker.rs`, `types::{Msg, Model}` and the pty parts of `src/cli.rs`; behaviour must not change (stdout and stderr bytes and their order, PTY writes and their order, timing, panic semantics, exit codes, files); every existing `#[test]` keeps its name and assertions
   (restructurings are listed in "Test moves"); the deviations 1-30 are accepted; P9-N1..N6 are behaviours that are noticed and deliberately kept; the architecture rules and the greps of "Acceptance greps" apply; the interim composites (legacy `Tracker`, `pub(crate)` aggregate fields, `Session` delegates) are allowed only in the steps that name them.
5. Nothing is pushed and no PR is opened (R43). After 9.7 the orchestrator asks the user for the manual smoke (O14) and reports the numbers: tests (1091 -> 1411), clippy 3 on the budget of 4, golden identical, testlock MISSING 0 / DUPLICATE 0 with the CHANGED list justified, the diff scripts empty, and the 30 deviations.

```js
Workflow({
  scriptPath: "docs/hexagonal-refactor/phase-workflow.js",
  args: /* { phase: "Phase 9 (terminal_session)", startRev: <HEAD the run starts from>, branch: "refactor/hexagonal-dropdown-terminal-session",
              reviewNotes: <item 4>, steps: <the run's steps, each prompt prefixed with its SINCE preamble> } */
})
```

